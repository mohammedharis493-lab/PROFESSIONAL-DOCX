use crate::ledger::amount_to_minor_units;
use calamine::{open_workbook_auto_from_rs, Data, Reader, SheetType};
use sha2::{Digest, Sha256};
use std::io::Cursor;

pub const MAX_TRIAL_BALANCE_IMPORT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_TRIAL_BALANCE_ROWS: usize = 100_000;

#[derive(Debug, Clone)]
pub struct TrialBalanceColumnMapping {
    pub account_name_column: u32,
    pub account_code_column: Option<u32>,
    pub opening_balance_column: Option<u32>,
    pub closing_balance_column: u32,
}

#[derive(Debug, Clone)]
pub struct ParsedTrialBalanceAccount {
    pub source_row_number: u64,
    pub source_row_hash: Vec<u8>,
    pub account_code_text: Option<String>,
    pub account_name_text: String,
    pub opening_minor: i64,
    pub closing_minor: i64,
}

pub fn parse_trial_balance_workbook(
    bytes: &[u8],
    sheet_name: &str,
    header_row_number: u32,
    amount_scale: u32,
    mapping: &TrialBalanceColumnMapping,
) -> Result<Vec<ParsedTrialBalanceAccount>, String> {
    if bytes.is_empty() {
        return Err("Controlled trial balance evidence is empty.".to_string());
    }
    if bytes.len() > MAX_TRIAL_BALANCE_IMPORT_BYTES {
        return Err(format!(
            "Controlled trial balance evidence exceeds the {} MB import limit.",
            MAX_TRIAL_BALANCE_IMPORT_BYTES / (1024 * 1024)
        ));
    }
    if header_row_number == 0 {
        return Err("Trial balance header row number must be at least 1.".to_string());
    }
    if amount_scale > 6 {
        return Err("Trial balance amount scale must be between 0 and 6.".to_string());
    }

    let sheet_name = sheet_name.trim();
    if sheet_name.is_empty() {
        return Err("Trial balance worksheet name is required.".to_string());
    }

    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = open_workbook_auto_from_rs(cursor)
        .map_err(|error| format!("Unable to read controlled trial balance workbook: {error}"))?;

    let worksheet_exists = workbook
        .sheets_metadata()
        .iter()
        .any(|sheet| sheet.name == sheet_name && sheet.typ == SheetType::WorkSheet);
    if !worksheet_exists {
        return Err(format!(
            "Worksheet '{sheet_name}' does not exist or is not a worksheet."
        ));
    }

    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|error| format!("Unable to read worksheet '{sheet_name}': {error}"))?;
    let Some((start_row, start_column)) = range.start() else {
        return Err(format!("Worksheet '{sheet_name}' is empty."));
    };
    let Some((end_row, end_column)) = range.end() else {
        return Err(format!("Worksheet '{sheet_name}' is empty."));
    };

    let header_row = header_row_number - 1;
    if header_row < start_row || header_row > end_row {
        return Err(format!(
            "Header row {header_row_number} is outside the used worksheet range."
        ));
    }

    for column in mapped_columns(mapping) {
        if column < start_column || column > end_column {
            return Err(format!(
                "Mapped column {} is outside the used worksheet range.",
                column + 1
            ));
        }
    }

    let mut accounts = Vec::new();
    let first_data_row = header_row.saturating_add(1);
    if first_data_row > end_row {
        return Ok(accounts);
    }

    for row in first_data_row..=end_row {
        if accounts.len() >= MAX_TRIAL_BALANCE_ROWS {
            return Err(format!(
                "Trial balance import exceeds the {MAX_TRIAL_BALANCE_ROWS} account limit."
            ));
        }

        let account_name_text = cell_text(&range, row, mapping.account_name_column);
        let account_code_text = mapping
            .account_code_column
            .and_then(|column| cell_text(&range, row, column));
        let opening_value = mapping
            .opening_balance_column
            .and_then(|column| range.get_value((row, column)));
        let closing_value = range.get_value((row, mapping.closing_balance_column));

        let all_mapped_empty = account_name_text.is_none()
            && account_code_text.is_none()
            && opening_value.is_none_or(|value| matches!(value, Data::Empty))
            && closing_value.is_none_or(|value| matches!(value, Data::Empty));
        if all_mapped_empty {
            continue;
        }

        let account_name_text = account_name_text.ok_or_else(|| {
            format!(
                "Trial balance row {} has mapped content but no account name.",
                u64::from(row) + 1
            )
        })?;
        let opening_minor = optional_amount_to_minor(opening_value, amount_scale).map_err(|error| {
            format!(
                "Trial balance row {} opening balance is invalid: {error}",
                u64::from(row) + 1
            )
        })?;
        let closing_minor = optional_amount_to_minor(closing_value, amount_scale).map_err(|error| {
            format!(
                "Trial balance row {} closing balance is invalid: {error}",
                u64::from(row) + 1
            )
        })?;

        accounts.push(ParsedTrialBalanceAccount {
            source_row_number: u64::from(row) + 1,
            source_row_hash: hash_source_row(&range, row, start_column, end_column),
            account_code_text,
            account_name_text,
            opening_minor,
            closing_minor,
        });
    }

    Ok(accounts)
}

fn mapped_columns(mapping: &TrialBalanceColumnMapping) -> Vec<u32> {
    let mut columns = vec![mapping.account_name_column, mapping.closing_balance_column];
    for column in [mapping.account_code_column, mapping.opening_balance_column]
        .into_iter()
        .flatten()
    {
        if !columns.contains(&column) {
            columns.push(column);
        }
    }
    columns
}

fn cell_text(range: &calamine::Range<Data>, row: u32, column: u32) -> Option<String> {
    range
        .get_value((row, column))
        .filter(|value| !matches!(value, Data::Empty))
        .map(ToString::to_string)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn optional_amount_to_minor(value: Option<&Data>, scale: u32) -> Result<i64, String> {
    match value {
        None | Some(Data::Empty) => Ok(0),
        Some(value) => amount_to_minor_units(value, scale),
    }
}

fn hash_source_row(
    range: &calamine::Range<Data>,
    row: u32,
    start_column: u32,
    end_column: u32,
) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"PROFESSIONAL-DOCX-TRIAL-BALANCE-ROW-V1");
    hasher.update(row.to_be_bytes());
    hasher.update(start_column.to_be_bytes());
    hasher.update(end_column.to_be_bytes());

    for column in start_column..=end_column {
        let value = range
            .get_value((row, column))
            .filter(|value| !matches!(value, Data::Empty))
            .map(ToString::to_string)
            .unwrap_or_default();
        hasher.update(column.to_be_bytes());
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }

    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_unique_trial_balance_columns() {
        let mapping = TrialBalanceColumnMapping {
            account_name_column: 1,
            account_code_column: Some(0),
            opening_balance_column: Some(2),
            closing_balance_column: 3,
        };
        assert_eq!(mapped_columns(&mapping), vec![1, 3, 0, 2]);
    }

    #[test]
    fn blank_trial_balance_amounts_are_zero() {
        assert_eq!(optional_amount_to_minor(None, 2).unwrap(), 0);
        assert_eq!(optional_amount_to_minor(Some(&Data::Empty), 2).unwrap(), 0);
        assert_eq!(
            optional_amount_to_minor(Some(&Data::String("1,234.50".into())), 2).unwrap(),
            123_450
        );
    }
}
