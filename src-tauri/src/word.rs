use crate::{launcher, persistence::ResolvedFileSource};
use quick_xml::{
    events::{BytesStart, Event},
    Reader,
};
use serde::Serialize;
use std::{fs, io::Read, path::Path};
use zip::ZipArchive;

pub const MAX_WORD_PREVIEW_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DOCUMENT_XML_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PREVIEW_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_BLOCKS: usize = 5_000;
const MAX_TABLE_ROWS: usize = 500;
const MAX_TABLE_COLUMNS: usize = 100;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordPreview {
    pub blocks: Vec<WordBlock>,
    pub truncated: bool,
    pub total_size_bytes: u64,
    pub block_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WordBlock {
    Paragraph {
        text: String,
        style: Option<String>,
    },
    Table {
        rows: Vec<Vec<String>>,
        truncated: bool,
    },
}

#[derive(Default)]
struct ParseState {
    blocks: Vec<WordBlock>,
    current_paragraph: Option<ParagraphBuilder>,
    current_table: Option<TableBuilder>,
    text_bytes: usize,
    in_text_node: bool,
    truncated: bool,
}

#[derive(Default)]
struct ParagraphBuilder {
    text: String,
    style: Option<String>,
}

#[derive(Default)]
struct TableBuilder {
    rows: Vec<Vec<String>>,
    current_row: Option<Vec<String>>,
    current_cell: Option<String>,
    truncated: bool,
}

pub fn preview_word_source(source: &ResolvedFileSource) -> Result<WordPreview, String> {
    let path = launcher::validated_existing_path(source).map_err(|error| error.to_string())?;
    preview_validated_docx(&path)
}

fn preview_validated_docx(path: &Path) -> Result<WordPreview, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    if !extension.eq_ignore_ascii_case("docx") {
        return Err(format!(
            "In-app Word preview currently supports DOCX files only, not .{}.",
            if extension.is_empty() {
                "<none>"
            } else {
                extension
            }
        ));
    }

    let metadata =
        fs::metadata(path).map_err(|error| format!("Unable to inspect Word document: {error}"))?;

    if metadata.len() > MAX_WORD_PREVIEW_BYTES {
        return Err(format!(
            "Word document is too large for in-app preview (limit: {} MB). Open the original instead.",
            MAX_WORD_PREVIEW_BYTES / (1024 * 1024)
        ));
    }

    let file =
        fs::File::open(path).map_err(|error| format!("Unable to open Word document: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Unable to read DOCX package: {error}"))?;

    let mut document = archive
        .by_name("word/document.xml")
        .map_err(|_| "DOCX package does not contain word/document.xml.".to_string())?;

    if document.size() > MAX_DOCUMENT_XML_BYTES {
        return Err(format!(
            "DOCX document XML exceeds the in-app preview safety limit ({} MB). Open the original instead.",
            MAX_DOCUMENT_XML_BYTES / (1024 * 1024)
        ));
    }

    let mut xml = String::with_capacity(document.size() as usize);
    document
        .read_to_string(&mut xml)
        .map_err(|error| format!("Unable to read DOCX document XML: {error}"))?;

    let (blocks, truncated) = parse_document_xml(&xml)?;
    let block_count = blocks.len();

    Ok(WordPreview {
        blocks,
        truncated,
        total_size_bytes: metadata.len(),
        block_count,
    })
}

fn parse_document_xml(xml: &str) -> Result<(Vec<WordBlock>, bool), String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut buffer = Vec::new();
    let mut state = ParseState::default();
    let mut in_text_node = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) => {
                let qualified_name = start.name();
                let name = local_name(qualified_name.as_ref());

                match name {
                    b"tbl" => {
                        if state.current_table.is_none() {
                            state.current_table = Some(TableBuilder::default());
                        }
                    }
                    b"tr" => {
                        if let Some(table) = state.current_table.as_mut() {
                            table.current_row = Some(Vec::new());
                        }
                    }
                    b"tc" => {
                        if let Some(table) = state.current_table.as_mut() {
                            table.current_cell = Some(String::new());
                        }
                    }
                    b"p" => {
                        state.current_paragraph = Some(ParagraphBuilder::default());
                    }
                    b"pStyle" => {
                        if let Some(paragraph) = state.current_paragraph.as_mut() {
                            paragraph.style = attribute_value(&start, b"val")?;
                        }
                    }
                    b"t" => state.in_text_node = true,
                    b"tab" => append_text(&mut state, "\t"),
                    b"br" | b"cr" => append_text(&mut state, "\n"),
                    _ => {}
                }
            }
            Ok(Event::Empty(empty)) => {
                let qualified_name = empty.name();
                let name = local_name(qualified_name.as_ref());
                match name {
                    b"pStyle" => {
                        if let Some(paragraph) = state.current_paragraph.as_mut() {
                            paragraph.style = attribute_value(&empty, b"val")?;
                        }
                    }
                    b"tab" => append_text(&mut state, "\t"),
                    b"br" | b"cr" => append_text(&mut state, "\n"),
                    _ => {}
                }
            }
            Ok(Event::Text(text)) => {
                if state.in_text_node {
                    let decoded = text
                        .decode()
                        .map_err(|error| format!("Unable to decode DOCX text: {error}"))?;
                    append_text(&mut state, decoded.as_ref());
                }
            }
            Ok(Event::End(end)) => {
                let qualified_name = end.name();
                let name = local_name(qualified_name.as_ref());

                match name {
                    b"t" => state.in_text_node = false,
                    b"p" => finish_paragraph(&mut state),
                    b"tc" => finish_cell(&mut state),
                    b"tr" => finish_row(&mut state),
                    b"tbl" => finish_table(&mut state),
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("Unable to parse DOCX document XML: {error}")),
        }

        if state.blocks.len() >= MAX_BLOCKS || state.text_bytes >= MAX_PREVIEW_TEXT_BYTES {
            state.truncated = true;
            break;
        }

        buffer.clear();
    }

    if state.current_table.is_some() {
        finish_table(&mut state);
    } else if state.current_paragraph.is_some() {
        finish_paragraph(&mut state);
    }

    Ok((state.blocks, state.truncated))
}

fn append_text(state: &mut ParseState, text: &str) {
    if state.text_bytes >= MAX_PREVIEW_TEXT_BYTES {
        state.truncated = true;
        return;
    }

    let remaining = MAX_PREVIEW_TEXT_BYTES - state.text_bytes;
    let safe_len = nearest_char_boundary(text, remaining);
    let fragment = &text[..safe_len];

    if let Some(paragraph) = state.current_paragraph.as_mut() {
        paragraph.text.push_str(fragment);
        state.text_bytes += fragment.len();
    }

    if safe_len < text.len() {
        state.truncated = true;
    }
}

fn finish_paragraph(state: &mut ParseState) {
    let Some(paragraph) = state.current_paragraph.take() else {
        return;
    };

    let text = paragraph.text.trim_matches(['\r', '\n']).to_string();

    if let Some(table) = state.current_table.as_mut() {
        if let Some(cell) = table.current_cell.as_mut() {
            if !text.is_empty() {
                if !cell.is_empty() {
                    cell.push('\n');
                }
                cell.push_str(&text);
            }
        }
        return;
    }

    if text.is_empty() || state.blocks.len() >= MAX_BLOCKS {
        if state.blocks.len() >= MAX_BLOCKS {
            state.truncated = true;
        }
        return;
    }

    state.blocks.push(WordBlock::Paragraph {
        text,
        style: paragraph.style,
    });
}

fn finish_cell(state: &mut ParseState) {
    let Some(table) = state.current_table.as_mut() else {
        return;
    };
    let Some(cell) = table.current_cell.take() else {
        return;
    };

    if let Some(row) = table.current_row.as_mut() {
        if row.len() < MAX_TABLE_COLUMNS {
            row.push(cell);
        } else {
            table.truncated = true;
            state.truncated = true;
        }
    }
}

fn finish_row(state: &mut ParseState) {
    let Some(table) = state.current_table.as_mut() else {
        return;
    };
    let Some(row) = table.current_row.take() else {
        return;
    };

    if table.rows.len() < MAX_TABLE_ROWS {
        table.rows.push(row);
    } else {
        table.truncated = true;
        state.truncated = true;
    }
}

fn finish_table(state: &mut ParseState) {
    let Some(mut table) = state.current_table.take() else {
        return;
    };

    if table.current_cell.is_some() {
        state.current_table = Some(table);
        finish_cell(state);
        table = state.current_table.take().unwrap_or_default();
    }

    if table.current_row.is_some() {
        state.current_table = Some(table);
        finish_row(state);
        table = state.current_table.take().unwrap_or_default();
    }

    if table.rows.is_empty() {
        return;
    }

    if state.blocks.len() >= MAX_BLOCKS {
        state.truncated = true;
        return;
    }

    state.blocks.push(WordBlock::Table {
        rows: table.rows,
        truncated: table.truncated,
    });
}

fn attribute_value(
    start: &BytesStart<'_>,
    wanted_local_name: &[u8],
) -> Result<Option<String>, String> {
    for attribute in start.attributes() {
        let attribute =
            attribute.map_err(|error| format!("Unable to parse DOCX attribute: {error}"))?;

        if local_name(attribute.key.as_ref()) == wanted_local_name {
            return Ok(Some(
                String::from_utf8_lossy(attribute.value.as_ref()).into_owned(),
            ));
        }
    }

    Ok(None)
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn nearest_char_boundary(text: &str, max_bytes: usize) -> usize {
    if text.len() <= max_bytes {
        return text.len();
    }

    let mut boundary = max_bytes;
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headings_paragraphs_and_tables_in_document_order() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:body>
                <w:p>
                  <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
                  <w:r><w:t>Audit Summary</w:t></w:r>
                </w:p>
                <w:p><w:r><w:t>Control </w:t></w:r><w:r><w:t>testing</w:t></w:r></w:p>
                <w:tbl>
                  <w:tr>
                    <w:tc><w:p><w:r><w:t>Ref</w:t></w:r></w:p></w:tc>
                    <w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc>
                  </w:tr>
                  <w:tr>
                    <w:tc><w:p><w:r><w:t>A1</w:t></w:r></w:p></w:tc>
                    <w:tc><w:p><w:r><w:t>Complete</w:t></w:r></w:p></w:tc>
                  </w:tr>
                </w:tbl>
              </w:body>
            </w:document>"#;

        let (blocks, truncated) = parse_document_xml(xml).expect("DOCX XML should parse");

        assert!(!truncated);
        assert_eq!(blocks.len(), 3);

        match &blocks[0] {
            WordBlock::Paragraph { text, style } => {
                assert_eq!(text, "Audit Summary");
                assert_eq!(style.as_deref(), Some("Heading1"));
            }
            _ => panic!("first block should be a paragraph"),
        }

        match &blocks[1] {
            WordBlock::Paragraph { text, .. } => assert_eq!(text, "Control testing"),
            _ => panic!("second block should be a paragraph"),
        }

        match &blocks[2] {
            WordBlock::Table { rows, truncated } => {
                assert!(!truncated);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0], vec!["Ref", "Status"]);
                assert_eq!(rows[1], vec!["A1", "Complete"]);
            }
            _ => panic!("third block should be a table"),
        }
    }

    #[test]
    fn preserves_tabs_and_line_breaks_inside_paragraphs() {
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p>
          <w:r><w:t>Alpha</w:t><w:tab/><w:t>Beta</w:t><w:br/><w:t>Gamma</w:t></w:r>
        </w:p></w:body></w:document>"#;

        let (blocks, _) = parse_document_xml(xml).expect("DOCX XML should parse");

        match &blocks[0] {
            WordBlock::Paragraph { text, .. } => assert_eq!(text, "Alpha\tBeta\nGamma"),
            _ => panic!("block should be a paragraph"),
        }
    }

    #[test]
    fn joins_multiple_paragraphs_inside_one_table_cell() {
        let xml = r#"<w:document xmlns:w="x"><w:body><w:tbl><w:tr><w:tc>
          <w:p><w:r><w:t>First</w:t></w:r></w:p>
          <w:p><w:r><w:t>Second</w:t></w:r></w:p>
        </w:tc></w:tr></w:tbl></w:body></w:document>"#;

        let (blocks, _) = parse_document_xml(xml).expect("DOCX XML should parse");

        match &blocks[0] {
            WordBlock::Table { rows, .. } => assert_eq!(rows[0][0], "First\nSecond"),
            _ => panic!("block should be a table"),
        }
    }

    #[test]
    fn resolves_prefixed_and_unprefixed_local_names() {
        assert_eq!(local_name(b"w:p"), b"p");
        assert_eq!(local_name(b"p"), b"p");
    }

    #[test]
    fn character_boundary_limit_never_splits_utf8() {
        let text = "AéZ";
        assert_eq!(nearest_char_boundary(text, 2), 1);
        assert_eq!(&text[..nearest_char_boundary(text, 3)], "Aé");
    }
}
