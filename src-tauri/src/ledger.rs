use calamine::{open_workbook_auto_from_rs, Data, Reader, SheetType};
use sha2::{Digest, Sha256};
use std::io::Cursor;

pub const MAX_LEDGER_IMPORT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_LEDGER_IMPORT_ROWS: usize = 500_000;

#[derive(Debug, Clone)]
pub struct LedgerColumnMapping {
    pub amount_column: u32,
    pub date_column: Option<u32>,
    pub account_column: Option<u32>,
    pub voucher_column: Option<u32>,
    pub narration_column: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ParsedLedgerTransaction {
    pub source_row_number: u64,
    pub source_row_hash: Vec<u8>,
    pub transaction_date_text: Option<String>,
    pub account_text: Option<String>,
    pub voucher_text: Option<String>,
    pub narration_text: Option<String>,
    pub amount_minor: i64,
}

pub fn parse_ledger_workbook(
    bytes: &[u8],
    sheet_name: &str,
    header_row_number: u32,
    amount_scale: u32,
    mapping: &LedgerColumnMapping,
) -> Result<Vec<ParsedLedgerTransaction>, String> {
    if bytes.is_empty() {
        return Err("Controlled ledger evidence is empty.".to_string());
    }
    if bytes.len() > MAX_LEDGER_IMPORT_BYTES {
        return Err(format!(
            "Controlled ledger evidence exceeds the {} MB import limit.",
            MAX_LEDGER_IMPORT_BYTES / (1024 * 1024)
        ));
    }
    if header_row_number == 0 {
        return Err("Ledger header row number must be at least 1.".to_string());
    }
    if amount_scale > 6 {
        return Err("Ledger amount scale must be between 0 and 6.".to_string());
    }

    let sheet_name = sheet_name.trim();
    if sheet_name.is_empty() {
        return Err("Ledger worksheet name is required.".to_string());
    }

    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = open_workbook_auto_from_rs(cursor)
        .map_err(|error| format!("Unable to read controlled ledger workbook: {error}"))?;

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

    let mut transactions = Vec::new();
    let first_data_row = header_row.saturating_add(1);
    if first_data_row > end_row {
        return Ok(transactions);
    }

    for row in first_data_row..=end_row {
        if transactions.len() >= MAX_LEDGER_IMPORT_ROWS {
            return Err(format!(
                "Ledger import exceeds the {MAX_LEDGER_IMPORT_ROWS} transaction limit."
            ));
        }

        let amount_value = range.get_value((row, mapping.amount_column));
        let transaction_date_text = mapped_text(&range, row, mapping.date_column);
        let account_text = mapped_text(&range, row, mapping.account_column);
        let voucher_text = mapped_text(&range, row, mapping.voucher_column);
        let narration_text = mapped_text(&range, row, mapping.narration_column);

        let amount_text = amount_value
            .filter(|value| !matches!(value, Data::Empty))
            .map(ToString::to_string)
            .unwrap_or_default();

        let all_mapped_empty = amount_text.trim().is_empty()
            && transaction_date_text.is_none()
            && account_text.is_none()
            && voucher_text.is_none()
            && narration_text.is_none();
        if all_mapped_empty {
            continue;
        }

        let amount_value = amount_value.ok_or_else(|| {
            format!(
                "Ledger row {} has mapped content but no amount.",
                u64::from(row) + 1
            )
        })?;
        if matches!(amount_value, Data::Empty) {
            return Err(format!(
                "Ledger row {} has mapped content but no amount.",
                u64::from(row) + 1
            ));
        }
        let amount_minor = amount_to_minor_units(amount_value, amount_scale).map_err(|error| {
            format!(
                "Ledger row {} amount is invalid: {error}",
                u64::from(row) + 1
            )
        })?;

        let source_row_hash = hash_source_row(&range, row, start_column, end_column);

        transactions.push(ParsedLedgerTransaction {
            source_row_number: u64::from(row) + 1,
            source_row_hash,
            transaction_date_text,
            account_text,
            voucher_text,
            narration_text,
            amount_minor,
        });
    }

    Ok(transactions)
}

fn hash_source_row(
    range: &calamine::Range<Data>,
    row: u32,
    start_column: u32,
    end_column: u32,
) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"PROFESSIONAL-DOCX-LEDGER-ROW-V1");
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

fn mapped_columns(mapping: &LedgerColumnMapping) -> Vec<u32> {
    let mut columns = vec![mapping.amount_column];
    for column in [
        mapping.date_column,
        mapping.account_column,
        mapping.voucher_column,
        mapping.narration_column,
    ]
    .into_iter()
    .flatten()
    {
        if !columns.contains(&column) {
            columns.push(column);
        }
    }
    columns
}

fn mapped_text(range: &calamine::Range<Data>, row: u32, column: Option<u32>) -> Option<String> {
    let column = column?;
    range
        .get_value((row, column))
        .filter(|value| !matches!(value, Data::Empty))
        .map(ToString::to_string)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn amount_to_minor_units(value: &Data, scale: u32) -> Result<i64, String> {
    let factor = 10_i128
        .checked_pow(scale)
        .ok_or_else(|| "amount scale overflowed".to_string())?;

    let scaled = match value {
        Data::Int(value) => i128::from(*value)
            .checked_mul(factor)
            .ok_or_else(|| "amount exceeds supported range".to_string())?,
        Data::Float(value) => {
            if !value.is_finite() {
                return Err("amount is not finite".to_string());
            }
            let scaled = *value * factor as f64;
            if !scaled.is_finite() {
                return Err("amount exceeds supported range".to_string());
            }
            scaled.round() as i128
        }
        Data::String(value) => parse_decimal_text(value, scale)?,
        _ => parse_decimal_text(&value.to_string(), scale)?,
    };

    i64::try_from(scaled).map_err(|_| "amount exceeds supported range".to_string())
}

fn parse_decimal_text(value: &str, scale: u32) -> Result<i128, String> {
    let mut text = value.trim().replace(',', "");
    if text.is_empty() {
        return Err("amount is blank".to_string());
    }

    let mut negative = false;
    if text.starts_with('(') && text.ends_with(')') {
        negative = true;
        text = text[1..text.len() - 1].trim().to_string();
    }
    if let Some(rest) = text.strip_prefix('-') {
        if negative {
            return Err("amount has conflicting signs".to_string());
        }
        negative = true;
        text = rest.to_string();
    } else if let Some(rest) = text.strip_prefix('+') {
        text = rest.to_string();
    }

    let mut parts = text.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return Err("amount contains more than one decimal point".to_string());
    }
    if integer.is_empty() && fraction.is_empty() {
        return Err("amount is blank".to_string());
    }
    if !integer.chars().all(|character| character.is_ascii_digit())
        || !fraction.chars().all(|character| character.is_ascii_digit())
    {
        return Err("amount must be numeric".to_string());
    }

    let scale_usize = usize::try_from(scale).map_err(|_| "invalid amount scale".to_string())?;
    if fraction.len() > scale_usize
        && fraction[scale_usize..]
            .chars()
            .any(|character| character != '0')
    {
        return Err(format!(
            "amount has more than {scale} non-zero decimal places"
        ));
    }

    let integer_value = if integer.is_empty() {
        0_i128
    } else {
        integer
            .parse::<i128>()
            .map_err(|_| "amount exceeds supported range".to_string())?
    };
    let factor = 10_i128
        .checked_pow(scale)
        .ok_or_else(|| "amount scale overflowed".to_string())?;
    let mut fraction_text = fraction.chars().take(scale_usize).collect::<String>();
    while fraction_text.len() < scale_usize {
        fraction_text.push('0');
    }
    let fraction_value = if fraction_text.is_empty() {
        0_i128
    } else {
        fraction_text
            .parse::<i128>()
            .map_err(|_| "amount exceeds supported range".to_string())?
    };

    let magnitude = integer_value
        .checked_mul(factor)
        .and_then(|value| value.checked_add(fraction_value))
        .ok_or_else(|| "amount exceeds supported range".to_string())?;
    Ok(if negative { -magnitude } else { magnitude })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_accounting_amounts_to_fixed_minor_units() {
        assert_eq!(parse_decimal_text("1,234.50", 2).unwrap(), 123_450);
        assert_eq!(parse_decimal_text("(99.25)", 2).unwrap(), -9_925);
        assert_eq!(parse_decimal_text("+10", 2).unwrap(), 1_000);
        assert_eq!(parse_decimal_text(".75", 2).unwrap(), 75);
        assert_eq!(parse_decimal_text("12.3400", 2).unwrap(), 1_234);
        assert!(parse_decimal_text("12.341", 2).is_err());
        assert!(parse_decimal_text("₹12.00", 2).is_err());
    }

    #[test]
    fn maps_unique_columns_for_range_validation() {
        let mapping = LedgerColumnMapping {
            amount_column: 4,
            date_column: Some(0),
            account_column: Some(1),
            voucher_column: Some(2),
            narration_column: Some(4),
        };
        assert_eq!(mapped_columns(&mapping), vec![4, 0, 1, 2]);
    }
}
