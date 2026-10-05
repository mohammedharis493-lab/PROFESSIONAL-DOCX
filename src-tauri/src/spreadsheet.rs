use crate::{launcher, persistence::ResolvedFileSource};
use calamine::{open_workbook_auto, Data, Dimensions, Reader, SheetType, SheetVisible, Sheets};
use quick_xml::{
    escape::{resolve_xml_entity, unescape},
    events::{BytesStart, Event},
    Reader as XmlReader,
};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    io::{BufReader, Read},
    path::Path,
};
use zip::ZipArchive;

pub const MAX_WORKBOOK_PREVIEW_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_WORKBOOK_ROWS: u32 = 60;
pub const DEFAULT_WORKBOOK_COLUMNS: u32 = 20;
pub const MAX_WORKBOOK_ROWS: u32 = 200;
pub const MAX_WORKBOOK_COLUMNS: u32 = 50;
const MAX_SHEET_METADATA_ITEMS: usize = 500;
const MAX_OOXML_METADATA_XML_BYTES: u64 = 8 * 1024 * 1024;
const MAX_COMMENT_TEXT_CHARS: usize = 4_096;
const MAX_WORKBOOK_SEARCH_HITS: usize = 200;
const MAX_WORKBOOK_SEARCH_CELLS: u64 = 500_000;

const SUPPORTED_EXCEL_EXTENSIONS: &[&str] = &["xlsx", "xlsm", "xls", "xlsb"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookSheetInfo {
    pub name: String,
    pub visibility: String,
    pub sheet_type: String,
    pub previewable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookCell {
    pub row: u32,
    pub column: u32,
    pub address: String,
    pub value: String,
    pub value_kind: String,
    pub formula: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookRangeInfo {
    pub start_row: u32,
    pub start_column: u32,
    pub end_row: u32,
    pub end_column: u32,
    pub address: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookHyperlinkInfo {
    pub range: WorkbookRangeInfo,
    pub target: Option<String>,
    pub location: Option<String>,
    pub displayed_text: Option<String>,
    pub tooltip: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookCommentInfo {
    pub row: u32,
    pub column: u32,
    pub address: String,
    pub author: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookSearchHit {
    pub sheet_name: String,
    pub row: u32,
    pub column: u32,
    pub address: String,
    pub value: String,
    pub formula: Option<String>,
    pub matched_field: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookSearchResult {
    pub hits: Vec<WorkbookSearchHit>,
    pub truncated: bool,
    pub scanned_cells: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookPreview {
    pub sheets: Vec<WorkbookSheetInfo>,
    pub selected_sheet: String,
    pub used_start_row: u32,
    pub used_start_column: u32,
    pub used_end_row: u32,
    pub used_end_column: u32,
    pub row_offset: u32,
    pub column_offset: u32,
    pub row_count: u32,
    pub column_count: u32,
    pub cells: Vec<WorkbookCell>,
    pub merged_ranges: Vec<WorkbookRangeInfo>,
    pub hyperlinks: Vec<WorkbookHyperlinkInfo>,
    pub comments: Vec<WorkbookCommentInfo>,
    pub hidden_rows: Vec<u32>,
    pub hidden_columns: Vec<u32>,
    pub ooxml_metadata_available: bool,
    pub metadata_truncated: bool,
    pub total_size_bytes: u64,
    pub extension: String,
}

pub fn search_workbook_source(
    source: &ResolvedFileSource,
    query: &str,
) -> Result<WorkbookSearchResult, String> {
    let path = launcher::validated_existing_path(source).map_err(|error| error.to_string())?;
    search_validated_workbook(&path, query)
}

fn search_validated_workbook(path: &Path, query: &str) -> Result<WorkbookSearchResult, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(WorkbookSearchResult {
            hits: Vec::new(),
            truncated: false,
            scanned_cells: 0,
        });
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if !SUPPORTED_EXCEL_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!(
            "In-document workbook search is not supported for .{} files.",
            if extension.is_empty() {
                "<none>"
            } else {
                &extension
            }
        ));
    }

    let metadata =
        fs::metadata(path).map_err(|error| format!("Unable to inspect workbook: {error}"))?;

    if metadata.len() > MAX_WORKBOOK_PREVIEW_BYTES {
        return Err(format!(
            "Workbook is too large for in-app search (limit: {} MB). Open the original instead.",
            MAX_WORKBOOK_PREVIEW_BYTES / (1024 * 1024)
        ));
    }

    let mut workbook =
        open_workbook_auto(path).map_err(|error| format!("Unable to read workbook: {error}"))?;

    let sheet_names = workbook
        .sheets_metadata()
        .iter()
        .filter(|sheet| sheet.typ == SheetType::WorkSheet)
        .map(|sheet| sheet.name.clone())
        .collect::<Vec<_>>();

    let normalized_query = query.to_lowercase();
    let mut hits = Vec::new();
    let mut scanned_cells = 0_u64;
    let mut truncated = false;

    'sheets: for sheet_name in sheet_names {
        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|error| format!("Unable to read worksheet '{sheet_name}': {error}"))?;
        let formulas = workbook
            .worksheet_formula(&sheet_name)
            .map_err(|error| format!("Unable to read formulas from '{sheet_name}': {error}"))?;

        let Some((start_row, start_column)) = range.start() else {
            continue;
        };
        let Some((end_row, end_column)) = range.end() else {
            continue;
        };

        for row in start_row..=end_row {
            for column in start_column..=end_column {
                if scanned_cells >= MAX_WORKBOOK_SEARCH_CELLS {
                    truncated = true;
                    break 'sheets;
                }
                scanned_cells += 1;

                let value_text = range
                    .get_value((row, column))
                    .filter(|value| !matches!(value, Data::Empty))
                    .map(ToString::to_string)
                    .unwrap_or_default();

                let formula_text = formulas
                    .get_value((row, column))
                    .filter(|formula| !formula.is_empty())
                    .cloned();

                let matched_field = if contains_normalized(&value_text, &normalized_query) {
                    Some("VALUE")
                } else if formula_text
                    .as_deref()
                    .is_some_and(|formula| contains_normalized(formula, &normalized_query))
                {
                    Some("FORMULA")
                } else {
                    None
                };

                let Some(matched_field) = matched_field else {
                    continue;
                };

                hits.push(WorkbookSearchHit {
                    sheet_name: sheet_name.clone(),
                    row,
                    column,
                    address: cell_address(row, column),
                    value: value_text,
                    formula: formula_text,
                    matched_field: matched_field.to_string(),
                });

                if hits.len() >= MAX_WORKBOOK_SEARCH_HITS {
                    truncated = true;
                    break 'sheets;
                }
            }
        }
    }

    Ok(WorkbookSearchResult {
        hits,
        truncated,
        scanned_cells,
    })
}

fn contains_normalized(value: &str, normalized_query: &str) -> bool {
    value.to_lowercase().contains(normalized_query)
}

pub fn preview_workbook_source(
    source: &ResolvedFileSource,
    sheet_name: Option<&str>,
    row_offset: Option<u32>,
    column_offset: Option<u32>,
    row_limit: Option<u32>,
    column_limit: Option<u32>,
) -> Result<WorkbookPreview, String> {
    let path = launcher::validated_existing_path(source).map_err(|error| error.to_string())?;
    preview_validated_workbook(
        &path,
        sheet_name,
        row_offset,
        column_offset,
        row_limit,
        column_limit,
    )
}

fn preview_validated_workbook(
    path: &Path,
    sheet_name: Option<&str>,
    row_offset: Option<u32>,
    column_offset: Option<u32>,
    row_limit: Option<u32>,
    column_limit: Option<u32>,
) -> Result<WorkbookPreview, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if !SUPPORTED_EXCEL_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!(
            "In-app Excel preview is not supported for .{} files.",
            if extension.is_empty() {
                "<none>"
            } else {
                &extension
            }
        ));
    }

    let metadata =
        fs::metadata(path).map_err(|error| format!("Unable to inspect workbook: {error}"))?;

    if metadata.len() > MAX_WORKBOOK_PREVIEW_BYTES {
        return Err(format!(
            "Workbook is too large for in-app preview (limit: {} MB). Open the original instead.",
            MAX_WORKBOOK_PREVIEW_BYTES / (1024 * 1024)
        ));
    }

    let mut workbook =
        open_workbook_auto(path).map_err(|error| format!("Unable to read workbook: {error}"))?;

    let sheets: Vec<WorkbookSheetInfo> = workbook
        .sheets_metadata()
        .iter()
        .map(|sheet| WorkbookSheetInfo {
            name: sheet.name.clone(),
            visibility: sheet_visibility_label(sheet.visible).to_string(),
            sheet_type: sheet_type_label(sheet.typ).to_string(),
            previewable: sheet.typ == SheetType::WorkSheet,
        })
        .collect();

    let selected_sheet = choose_sheet(&sheets, sheet_name)?;
    let range = workbook
        .worksheet_range(&selected_sheet)
        .map_err(|error| format!("Unable to read worksheet '{selected_sheet}': {error}"))?;
    let formulas = workbook
        .worksheet_formula(&selected_sheet)
        .map_err(|error| format!("Unable to read formulas from '{selected_sheet}': {error}"))?;

    let (used_start_row, used_start_column) = range.start().unwrap_or((0, 0));
    let (used_end_row, used_end_column) = range.end().unwrap_or((0, 0));

    let bounded_row_limit = row_limit
        .unwrap_or(DEFAULT_WORKBOOK_ROWS)
        .clamp(1, MAX_WORKBOOK_ROWS);
    let bounded_column_limit = column_limit
        .unwrap_or(DEFAULT_WORKBOOK_COLUMNS)
        .clamp(1, MAX_WORKBOOK_COLUMNS);

    let effective_row_offset = row_offset.unwrap_or(used_start_row);
    let effective_column_offset = column_offset.unwrap_or(used_start_column);

    let actual_row_count = if range.is_empty() || effective_row_offset > used_end_row {
        0
    } else {
        bounded_row_limit.min(
            used_end_row
                .saturating_sub(effective_row_offset)
                .saturating_add(1),
        )
    };

    let actual_column_count = if range.is_empty() || effective_column_offset > used_end_column {
        0
    } else {
        bounded_column_limit.min(
            used_end_column
                .saturating_sub(effective_column_offset)
                .saturating_add(1),
        )
    };

    let row_end_exclusive = effective_row_offset.saturating_add(actual_row_count);
    let column_end_exclusive = effective_column_offset.saturating_add(actual_column_count);

    let mut cells = Vec::new();

    for row in effective_row_offset..row_end_exclusive {
        for column in effective_column_offset..column_end_exclusive {
            let value = range.get_value((row, column));
            let formula = formulas
                .get_value((row, column))
                .filter(|formula| !formula.is_empty())
                .cloned();

            if value.is_none() && formula.is_none() {
                continue;
            }

            let value = value.unwrap_or(&Data::Empty);
            if matches!(value, Data::Empty) && formula.is_none() {
                continue;
            }

            cells.push(WorkbookCell {
                row,
                column,
                address: cell_address(row, column),
                value: value.to_string(),
                value_kind: data_kind(value).to_string(),
                formula,
            });
        }
    }

    let mut metadata_truncated = false;

    let mut merged_ranges = merged_ranges_for_sheet(&mut workbook, &selected_sheet)?
        .into_iter()
        .map(range_info)
        .collect::<Vec<_>>();

    if merged_ranges.len() > MAX_SHEET_METADATA_ITEMS {
        merged_ranges.truncate(MAX_SHEET_METADATA_ITEMS);
        metadata_truncated = true;
    }

    let mut hyperlinks = hyperlinks_for_sheet(&mut workbook, &selected_sheet)?
        .into_iter()
        .map(|link| WorkbookHyperlinkInfo {
            range: range_info(link.range),
            target: link.target,
            location: link.location,
            displayed_text: link.displayed_text,
            tooltip: link.tooltip,
        })
        .collect::<Vec<_>>();

    if hyperlinks.len() > MAX_SHEET_METADATA_ITEMS {
        hyperlinks.truncate(MAX_SHEET_METADATA_ITEMS);
        metadata_truncated = true;
    }

    let (ooxml_metadata_available, ooxml_metadata) =
        if matches!(extension.as_str(), "xlsx" | "xlsm") {
            match ooxml_sheet_metadata(path, &selected_sheet) {
                Ok(metadata) => (true, metadata),
                Err(_) => (false, OoxmlSheetMetadata::default()),
            }
        } else {
            (false, OoxmlSheetMetadata::default())
        };
    metadata_truncated |= ooxml_metadata.truncated;

    Ok(WorkbookPreview {
        sheets,
        selected_sheet,
        used_start_row,
        used_start_column,
        used_end_row,
        used_end_column,
        row_offset: effective_row_offset,
        column_offset: effective_column_offset,
        row_count: actual_row_count,
        column_count: actual_column_count,
        cells,
        merged_ranges,
        hyperlinks,
        comments: ooxml_metadata.comments,
        hidden_rows: ooxml_metadata.hidden_rows,
        hidden_columns: ooxml_metadata.hidden_columns,
        ooxml_metadata_available,
        metadata_truncated,
        total_size_bytes: metadata.len(),
        extension,
    })
}

#[derive(Default)]
struct OoxmlSheetMetadata {
    hidden_rows: Vec<u32>,
    hidden_columns: Vec<u32>,
    comments: Vec<WorkbookCommentInfo>,
    truncated: bool,
}

fn ooxml_sheet_metadata(path: &Path, sheet_name: &str) -> Result<OoxmlSheetMetadata, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("Unable to open OOXML workbook metadata: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Unable to read OOXML package: {error}"))?;

    let workbook_xml = read_ooxml_xml(&mut archive, "xl/workbook.xml")?
        .ok_or_else(|| "OOXML package does not contain xl/workbook.xml.".to_string())?;
    let relationship_id = sheet_relationship_id(&workbook_xml, sheet_name)?
        .ok_or_else(|| format!("Worksheet '{sheet_name}' has no OOXML relationship."))?;

    let workbook_rels = read_ooxml_xml(&mut archive, "xl/_rels/workbook.xml.rels")?
        .ok_or_else(|| "OOXML package does not contain workbook relationships.".to_string())?;
    let sheet_target =
        relationship_target_by_id(&workbook_rels, &relationship_id, Some("/worksheet"))?
            .ok_or_else(|| format!("Worksheet '{sheet_name}' target could not be resolved."))?;
    let sheet_part = normalize_package_target("xl/workbook.xml", &sheet_target)
        .ok_or_else(|| "Worksheet relationship target is invalid.".to_string())?;

    let sheet_xml = read_ooxml_xml(&mut archive, &sheet_part)?
        .ok_or_else(|| format!("OOXML worksheet part '{sheet_part}' is missing."))?;
    let (hidden_rows, hidden_columns, mut truncated) = parse_hidden_sheet_dimensions(&sheet_xml)?;

    let comments = if let Some(rels_part) = relationships_part_for(&sheet_part) {
        if let Some(sheet_rels) = read_ooxml_xml(&mut archive, &rels_part)? {
            if let Some(comment_target) = relationship_target_by_type(&sheet_rels, "/comments")? {
                if let Some(comment_part) = normalize_package_target(&sheet_part, &comment_target) {
                    if let Some(comment_xml) = read_ooxml_xml(&mut archive, &comment_part)? {
                        let (comments, comments_truncated) = parse_comments_xml(&comment_xml)?;
                        truncated |= comments_truncated;
                        comments
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    Ok(OoxmlSheetMetadata {
        hidden_rows,
        hidden_columns,
        comments,
        truncated,
    })
}

fn read_ooxml_xml(
    archive: &mut ZipArchive<fs::File>,
    part_name: &str,
) -> Result<Option<String>, String> {
    let mut part = match archive.by_name(part_name) {
        Ok(part) => part,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Unable to open OOXML package part '{part_name}': {error}"
            ))
        }
    };

    if part.size() > MAX_OOXML_METADATA_XML_BYTES {
        return Err(format!(
            "OOXML metadata part '{part_name}' exceeds the {} MB safety limit.",
            MAX_OOXML_METADATA_XML_BYTES / (1024 * 1024)
        ));
    }

    let mut xml = String::with_capacity(part.size() as usize);
    part.read_to_string(&mut xml)
        .map_err(|error| format!("Unable to read OOXML package part '{part_name}': {error}"))?;
    Ok(Some(xml))
}

fn sheet_relationship_id(xml: &str, wanted_sheet_name: &str) -> Result<Option<String>, String> {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) | Ok(Event::Empty(start))
                if local_xml_name(start.name().as_ref()) == b"sheet" =>
            {
                let name = xml_attribute_value(&start, b"name")?;
                if name.as_deref() == Some(wanted_sheet_name) {
                    return xml_attribute_value(&start, b"id");
                }
            }
            Ok(Event::Eof) => return Ok(None),
            Ok(_) => {}
            Err(error) => return Err(format!("Unable to parse OOXML workbook metadata: {error}")),
        }
        buffer.clear();
    }
}

fn relationship_target_by_id(
    xml: &str,
    wanted_id: &str,
    required_type_suffix: Option<&str>,
) -> Result<Option<String>, String> {
    relationship_target(xml, Some(wanted_id), required_type_suffix)
}

fn relationship_target_by_type(
    xml: &str,
    required_type_suffix: &str,
) -> Result<Option<String>, String> {
    relationship_target(xml, None, Some(required_type_suffix))
}

fn relationship_target(
    xml: &str,
    wanted_id: Option<&str>,
    required_type_suffix: Option<&str>,
) -> Result<Option<String>, String> {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) | Ok(Event::Empty(start))
                if local_xml_name(start.name().as_ref()) == b"Relationship" =>
            {
                let id = xml_attribute_value(&start, b"Id")?;
                let target_mode = xml_attribute_value(&start, b"TargetMode")?;
                let relationship_type = xml_attribute_value(&start, b"Type")?;

                let id_matches = wanted_id.is_none_or(|wanted| id.as_deref() == Some(wanted));
                let is_internal = !target_mode
                    .as_deref()
                    .is_some_and(|mode| mode.eq_ignore_ascii_case("External"));
                let type_matches = required_type_suffix.is_none_or(|suffix| {
                    relationship_type
                        .as_deref()
                        .is_some_and(|value| value.ends_with(suffix))
                });

                if id_matches && is_internal && type_matches {
                    return xml_attribute_value(&start, b"Target");
                }
            }
            Ok(Event::Eof) => return Ok(None),
            Ok(_) => {}
            Err(error) => {
                return Err(format!(
                    "Unable to parse OOXML relationship metadata: {error}"
                ))
            }
        }
        buffer.clear();
    }
}

fn parse_hidden_sheet_dimensions(xml: &str) -> Result<(Vec<u32>, Vec<u32>, bool), String> {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut hidden_rows = BTreeSet::new();
    let mut hidden_columns = BTreeSet::new();
    let mut truncated = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) | Ok(Event::Empty(start)) => {
                match local_xml_name(start.name().as_ref()) {
                    b"row" if xml_attribute_is_true(&start, b"hidden")? => {
                        if let Some(row) = xml_attribute_value(&start, b"r")?
                            .and_then(|value| value.parse::<u32>().ok())
                            .and_then(|value| value.checked_sub(1))
                        {
                            if hidden_rows.len() < MAX_SHEET_METADATA_ITEMS {
                                hidden_rows.insert(row);
                            } else {
                                truncated = true;
                            }
                        }
                    }
                    b"col" if xml_attribute_is_true(&start, b"hidden")? => {
                        let min = xml_attribute_value(&start, b"min")?
                            .and_then(|value| value.parse::<u32>().ok());
                        let max = xml_attribute_value(&start, b"max")?
                            .and_then(|value| value.parse::<u32>().ok());

                        if let (Some(min), Some(max)) = (min, max) {
                            for column in min..=max {
                                let Some(column) = column.checked_sub(1) else {
                                    continue;
                                };
                                if hidden_columns.len() < MAX_SHEET_METADATA_ITEMS {
                                    hidden_columns.insert(column);
                                } else {
                                    truncated = true;
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(format!(
                    "Unable to parse OOXML worksheet visibility metadata: {error}"
                ))
            }
        }
        buffer.clear();
    }

    Ok((
        hidden_rows.into_iter().collect(),
        hidden_columns.into_iter().collect(),
        truncated,
    ))
}

fn parse_comments_xml(xml: &str) -> Result<(Vec<WorkbookCommentInfo>, bool), String> {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut authors = Vec::new();
    let mut current_author = None::<String>;
    let mut current_comment = None::<(String, Option<usize>, String)>;
    let mut in_author = false;
    let mut in_comment_text = false;
    let mut comments = Vec::new();
    let mut truncated = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) => match local_xml_name(start.name().as_ref()) {
                b"author" => {
                    in_author = true;
                    current_author = Some(String::new());
                }
                b"comment" => {
                    let address = xml_attribute_value(&start, b"ref")?.unwrap_or_default();
                    let author_id = xml_attribute_value(&start, b"authorId")?
                        .and_then(|value| value.parse::<usize>().ok());
                    current_comment = Some((address, author_id, String::new()));
                }
                b"t" if current_comment.is_some() => in_comment_text = true,
                _ => {}
            },
            Ok(Event::Empty(start)) if local_xml_name(start.name().as_ref()) == b"comment" => {
                if comments.len() >= MAX_SHEET_METADATA_ITEMS {
                    truncated = true;
                } else if let Some(address) = xml_attribute_value(&start, b"ref")? {
                    if let Some((row, column)) = parse_cell_address(&address) {
                        let author_id = xml_attribute_value(&start, b"authorId")?
                            .and_then(|value| value.parse::<usize>().ok());
                        comments.push(WorkbookCommentInfo {
                            row,
                            column,
                            address,
                            author: author_id.and_then(|index| authors.get(index).cloned()),
                            text: String::new(),
                        });
                    }
                }
            }
            Ok(Event::Text(text)) => {
                let decoded = text
                    .decode()
                    .map_err(|error| format!("Unable to decode OOXML text: {error}"))?;
                let decoded = unescape(decoded.as_ref())
                    .map_err(|error| format!("Unable to unescape OOXML text: {error}"))?;
                append_comment_fragment(
                    &mut current_author,
                    &mut current_comment,
                    in_author,
                    in_comment_text,
                    decoded.as_ref(),
                    &mut truncated,
                );
            }
            Ok(Event::GeneralRef(reference)) => {
                let resolved = if let Some(character) =
                    reference.resolve_char_ref().map_err(|error| {
                        format!("Unable to resolve OOXML character reference: {error}")
                    })? {
                    character.to_string()
                } else {
                    let entity_name = String::from_utf8_lossy(reference.as_ref());
                    resolve_xml_entity(entity_name.as_ref())
                        .ok_or_else(|| {
                            format!("Unsupported OOXML entity reference '&{entity_name};'.")
                        })?
                        .to_string()
                };
                append_comment_fragment(
                    &mut current_author,
                    &mut current_comment,
                    in_author,
                    in_comment_text,
                    &resolved,
                    &mut truncated,
                );
            }
            Ok(Event::End(end)) => match local_xml_name(end.name().as_ref()) {
                b"author" => {
                    in_author = false;
                    if let Some(author) = current_author.take() {
                        authors.push(author);
                    }
                }
                b"t" => in_comment_text = false,
                b"comment" => {
                    if let Some((address, author_id, text)) = current_comment.take() {
                        if comments.len() >= MAX_SHEET_METADATA_ITEMS {
                            truncated = true;
                        } else if let Some((row, column)) = parse_cell_address(&address) {
                            comments.push(WorkbookCommentInfo {
                                row,
                                column,
                                address,
                                author: author_id.and_then(|index| authors.get(index).cloned()),
                                text,
                            });
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("Unable to parse OOXML comments metadata: {error}")),
        }
        buffer.clear();
    }

    Ok((comments, truncated))
}

fn append_comment_fragment(
    current_author: &mut Option<String>,
    current_comment: &mut Option<(String, Option<usize>, String)>,
    in_author: bool,
    in_comment_text: bool,
    fragment: &str,
    truncated: &mut bool,
) {
    if in_author {
        if let Some(author) = current_author.as_mut() {
            author.push_str(fragment);
        }
        return;
    }

    if !in_comment_text {
        return;
    }

    let Some((_, _, comment_text)) = current_comment.as_mut() else {
        return;
    };

    let remaining = MAX_COMMENT_TEXT_CHARS.saturating_sub(comment_text.chars().count());
    if remaining == 0 {
        *truncated = true;
        return;
    }

    let bounded: String = fragment.chars().take(remaining).collect();
    comment_text.push_str(&bounded);
    if bounded.chars().count() < fragment.chars().count() {
        *truncated = true;
    }
}

fn xml_attribute_value(
    start: &BytesStart<'_>,
    wanted_local_name: &[u8],
) -> Result<Option<String>, String> {
    for attribute in start.attributes() {
        let attribute =
            attribute.map_err(|error| format!("Unable to parse OOXML attribute: {error}"))?;

        if local_xml_name(attribute.key.as_ref()) == wanted_local_name {
            let raw = String::from_utf8_lossy(attribute.value.as_ref());
            let value = unescape(raw.as_ref())
                .map_err(|error| format!("Unable to unescape OOXML attribute: {error}"))?;
            return Ok(Some(value.into_owned()));
        }
    }

    Ok(None)
}

fn xml_attribute_is_true(start: &BytesStart<'_>, wanted_local_name: &[u8]) -> Result<bool, String> {
    Ok(xml_attribute_value(start, wanted_local_name)?
        .as_deref()
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true")))
}

fn local_xml_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn normalize_package_target(base_part: &str, target: &str) -> Option<String> {
    if target.contains("://") || target.contains(char::from(92_u8)) {
        return None;
    }

    let mut segments = Vec::new();
    if !target.starts_with('/') {
        let base_parent = base_part
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        segments.extend(
            base_parent
                .split('/')
                .filter(|segment| !segment.is_empty())
                .map(str::to_string),
        );
    }

    for segment in target.trim_start_matches('/').split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other.to_string()),
        }
    }

    (!segments.is_empty()).then(|| segments.join("/"))
}

fn relationships_part_for(part: &str) -> Option<String> {
    let (directory, file_name) = part.rsplit_once('/')?;
    Some(format!("{directory}/_rels/{file_name}.rels"))
}

fn parse_cell_address(address: &str) -> Option<(u32, u32)> {
    let cleaned = address.replace('$', "");
    let mut column = 0_u32;
    let mut column_chars = 0_u32;
    let mut row_start = 0_usize;

    for (index, character) in cleaned.char_indices() {
        if character.is_ascii_alphabetic() {
            let upper = character.to_ascii_uppercase();
            column = column
                .checked_mul(26)?
                .checked_add(u32::from(upper as u8 - b'A') + 1)?;
            column_chars += 1;
            row_start = index + character.len_utf8();
        } else {
            break;
        }
    }

    if column_chars == 0 || row_start >= cleaned.len() {
        return None;
    }

    let row = cleaned[row_start..].parse::<u32>().ok()?.checked_sub(1)?;
    Some((row, column.checked_sub(1)?))
}

fn choose_sheet(sheets: &[WorkbookSheetInfo], requested: Option<&str>) -> Result<String, String> {
    if sheets.is_empty() {
        return Err("Workbook contains no worksheets.".to_string());
    }

    if let Some(requested) = requested {
        let sheet = sheets
            .iter()
            .find(|sheet| sheet.name == requested)
            .ok_or_else(|| "Requested worksheet does not exist.".to_string())?;

        if !sheet.previewable {
            return Err("Requested sheet type cannot be previewed as worksheet cells.".to_string());
        }

        return Ok(sheet.name.clone());
    }

    sheets
        .iter()
        .find(|sheet| sheet.previewable && sheet.visibility == "VISIBLE")
        .or_else(|| sheets.iter().find(|sheet| sheet.previewable))
        .map(|sheet| sheet.name.clone())
        .ok_or_else(|| "Workbook contains no previewable worksheets.".to_string())
}

fn merged_ranges_for_sheet(
    workbook: &mut Sheets<BufReader<fs::File>>,
    sheet_name: &str,
) -> Result<Vec<Dimensions>, String> {
    match workbook {
        Sheets::Xlsx(book) => book
            .merge_cells_by_sheet_name(sheet_name)
            .map_err(|error| format!("Unable to read merged ranges: {error}")),
        Sheets::Xls(book) => book
            .merge_cells_by_sheet_name(sheet_name)
            .map_err(|error| format!("Unable to read merged ranges: {error}")),
        Sheets::Xlsb(_) | Sheets::Ods(_) => Ok(Vec::new()),
    }
}

fn hyperlinks_for_sheet(
    workbook: &mut Sheets<BufReader<fs::File>>,
    sheet_name: &str,
) -> Result<Vec<calamine::Hyperlink>, String> {
    match workbook {
        Sheets::Xlsx(book) => book
            .hyperlinks_by_sheet_name(sheet_name)
            .map_err(|error| format!("Unable to read worksheet hyperlinks: {error}")),
        Sheets::Xls(_) | Sheets::Xlsb(_) | Sheets::Ods(_) => Ok(Vec::new()),
    }
}

fn sheet_visibility_label(visibility: SheetVisible) -> &'static str {
    match visibility {
        SheetVisible::Visible => "VISIBLE",
        SheetVisible::Hidden => "HIDDEN",
        SheetVisible::VeryHidden => "VERY_HIDDEN",
    }
}

fn sheet_type_label(sheet_type: SheetType) -> &'static str {
    match sheet_type {
        SheetType::WorkSheet => "WORKSHEET",
        SheetType::DialogSheet => "DIALOG_SHEET",
        SheetType::MacroSheet => "MACRO_SHEET",
        SheetType::ChartSheet => "CHART_SHEET",
        SheetType::Vba => "VBA",
    }
}

fn data_kind(value: &Data) -> &'static str {
    match value {
        Data::Empty => "EMPTY",
        Data::String(_) => "STRING",
        Data::Float(_) => "FLOAT",
        Data::Int(_) => "INTEGER",
        Data::Bool(_) => "BOOLEAN",
        Data::DateTime(_) | Data::DateTimeIso(_) => "DATETIME",
        Data::DurationIso(_) => "DURATION",
        Data::Error(_) => "ERROR",
    }
}

fn range_info(range: Dimensions) -> WorkbookRangeInfo {
    WorkbookRangeInfo {
        start_row: range.start.0,
        start_column: range.start.1,
        end_row: range.end.0,
        end_column: range.end.1,
        address: format!(
            "{}:{}",
            cell_address(range.start.0, range.start.1),
            cell_address(range.end.0, range.end.1)
        ),
    }
}

fn cell_address(row: u32, column: u32) -> String {
    format!("{}{}", column_label(column), row.saturating_add(1))
}

fn column_label(column: u32) -> String {
    let mut value = u64::from(column) + 1;
    let mut chars = Vec::new();

    while value > 0 {
        let remainder = ((value - 1) % 26) as u8;
        chars.push((b'A' + remainder) as char);
        value = (value - 1) / 26;
    }

    chars.into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uuid::Uuid;

    struct WorkbookFixture {
        root: PathBuf,
    }

    impl WorkbookFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "professional-docx-workbook-preview-{}",
                Uuid::new_v4()
            ));
            fs::create_dir_all(&root).expect("workbook preview test root should be created");
            Self { root }
        }

        fn source(&self, relative_path: &str) -> ResolvedFileSource {
            ResolvedFileSource {
                storage_root_path: self.root.clone(),
                relative_path: PathBuf::from(relative_path),
            }
        }
    }

    impl Drop for WorkbookFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn parses_ooxml_hidden_dimensions_and_comments() {
        use std::io::Write;
        use zip::{write::SimpleFileOptions, ZipWriter};

        let fixture = WorkbookFixture::new();
        let path = fixture.root.join("metadata.xlsx");
        let file = fs::File::create(&path).expect("OOXML fixture should be created");
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        let parts = [
            (
                "xl/workbook.xml",
                r#"<workbook xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="A &amp; B" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                r#"<worksheet><cols><col min="2" max="3" hidden="1"/></cols><sheetData><row r="4" hidden="true"/></sheetData></worksheet>"#,
            ),
            (
                "xl/worksheets/_rels/sheet1.xml.rels",
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="../comments1.xml"/></Relationships>"#,
            ),
            (
                "xl/comments1.xml",
                r#"<comments><authors><author>Reviewer &amp; Partner</author></authors><commentList><comment ref="C4" authorId="0"><text><t>Check &amp; agree</t></text></comment></commentList></comments>"#,
            ),
        ];

        for (name, content) in parts {
            writer
                .start_file(name, options)
                .expect("OOXML part should start");
            writer
                .write_all(content.as_bytes())
                .expect("OOXML part should be written");
        }
        writer.finish().expect("OOXML fixture should finish");

        let metadata = ooxml_sheet_metadata(&path, "A & B").expect("OOXML metadata should parse");

        assert_eq!(metadata.hidden_rows, vec![3]);
        assert_eq!(metadata.hidden_columns, vec![1, 2]);
        assert_eq!(metadata.comments.len(), 1);
        assert_eq!(metadata.comments[0].address, "C4");
        assert_eq!(metadata.comments[0].row, 3);
        assert_eq!(metadata.comments[0].column, 2);
        assert_eq!(
            metadata.comments[0].author.as_deref(),
            Some("Reviewer & Partner")
        );
        assert_eq!(metadata.comments[0].text, "Check & agree");
        assert!(!metadata.truncated);
    }

    #[test]
    fn normalizes_ooxml_relationship_targets_without_escape() {
        assert_eq!(
            normalize_package_target("xl/worksheets/sheet1.xml", "../comments1.xml"),
            Some("xl/comments1.xml".to_string())
        );
        assert_eq!(
            normalize_package_target("xl/workbook.xml", "worksheets/sheet1.xml"),
            Some("xl/worksheets/sheet1.xml".to_string())
        );
        assert!(normalize_package_target("xl/workbook.xml", "../../escape.xml").is_none());
        assert!(normalize_package_target("xl/workbook.xml", "https://example.com/x").is_none());
    }

    #[test]
    fn parses_excel_cell_addresses_for_comment_metadata() {
        assert_eq!(parse_cell_address("A1"), Some((0, 0)));
        assert_eq!(parse_cell_address("$AB$10"), Some((9, 27)));
        assert_eq!(parse_cell_address("ZZ100"), Some((99, 701)));
        assert_eq!(parse_cell_address("not-a-cell"), None);
    }

    #[test]
    fn matches_workbook_search_case_insensitively() {
        assert!(contains_normalized("Revenue Recognition", "revenue"));
        assert!(contains_normalized("=SUM(A1:A3)", "sum("));
        assert!(!contains_normalized("Cash", "inventory"));
    }

    #[test]
    fn converts_zero_based_columns_to_excel_labels() {
        assert_eq!(column_label(0), "A");
        assert_eq!(column_label(25), "Z");
        assert_eq!(column_label(26), "AA");
        assert_eq!(column_label(701), "ZZ");
        assert_eq!(column_label(702), "AAA");
    }

    #[test]
    fn builds_a1_cell_addresses() {
        assert_eq!(cell_address(0, 0), "A1");
        assert_eq!(cell_address(9, 27), "AB10");
    }

    #[test]
    fn chooses_first_visible_previewable_sheet_by_default() {
        let sheets = vec![
            WorkbookSheetInfo {
                name: "Hidden".into(),
                visibility: "HIDDEN".into(),
                sheet_type: "WORKSHEET".into(),
                previewable: true,
            },
            WorkbookSheetInfo {
                name: "Dashboard".into(),
                visibility: "VISIBLE".into(),
                sheet_type: "CHART_SHEET".into(),
                previewable: false,
            },
            WorkbookSheetInfo {
                name: "Data".into(),
                visibility: "VISIBLE".into(),
                sheet_type: "WORKSHEET".into(),
                previewable: true,
            },
        ];

        assert_eq!(choose_sheet(&sheets, None).unwrap(), "Data");
    }

    #[test]
    fn rejects_non_previewable_requested_sheet() {
        let sheets = vec![WorkbookSheetInfo {
            name: "Chart".into(),
            visibility: "VISIBLE".into(),
            sheet_type: "CHART_SHEET".into(),
            previewable: false,
        }];

        assert!(choose_sheet(&sheets, Some("Chart")).is_err());
    }

    #[test]
    fn rejects_unsupported_excel_extension_before_parsing() {
        let fixture = WorkbookFixture::new();
        fs::write(fixture.root.join("notes.csv"), b"a,b\n1,2\n")
            .expect("CSV fixture should be written");

        let error =
            preview_workbook_source(&fixture.source("notes.csv"), None, None, None, None, None)
                .expect_err("CSV should not be parsed by Excel viewer");

        assert!(error.contains("not supported"));
    }

    #[test]
    fn rejects_workbook_over_preview_size_limit_before_parsing() {
        let fixture = WorkbookFixture::new();
        let path = fixture.root.join("huge.xlsx");
        let file = fs::File::create(path).expect("large workbook fixture should be created");
        file.set_len(MAX_WORKBOOK_PREVIEW_BYTES + 1)
            .expect("large workbook fixture should be sized");

        let error =
            preview_workbook_source(&fixture.source("huge.xlsx"), None, None, None, None, None)
                .expect_err("oversize workbook should fail before parsing");

        assert!(error.contains("too large"));
    }
}
