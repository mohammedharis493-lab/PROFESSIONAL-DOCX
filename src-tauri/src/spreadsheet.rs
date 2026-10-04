use crate::{launcher, persistence::ResolvedFileSource};
use calamine::{open_workbook_auto, Data, Dimensions, Reader, SheetType, SheetVisible, Sheets};
use serde::Serialize;
use std::{fs, io::BufReader, path::Path};

pub const MAX_WORKBOOK_PREVIEW_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_WORKBOOK_ROWS: u32 = 60;
pub const DEFAULT_WORKBOOK_COLUMNS: u32 = 20;
pub const MAX_WORKBOOK_ROWS: u32 = 200;
pub const MAX_WORKBOOK_COLUMNS: u32 = 50;
const MAX_SHEET_METADATA_ITEMS: usize = 500;
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
        metadata_truncated,
        total_size_bytes: metadata.len(),
        extension,
    })
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
