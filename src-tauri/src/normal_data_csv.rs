//! Strict, bounded UTF-8 CSV normalization for verified Normal Data sources.
//! This initial adapter accepts numeric amounts as signed integer MINOR UNITS.
//! It never guesses a currency scale, rounds decimals, or infers period roles.
use crate::normal_data_comparison::{CompareRow, PeriodBasis};
use std::collections::BTreeMap;

const MAX_ROWS: usize = 100_000;
const MAX_COLUMNS: usize = 256;

pub struct CsvLayout<'a> {
    pub key_column: &'a str,
    pub period_column: &'a str,
    pub period_basis: PeriodBasis,
    pub amount_columns: &'a [String],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldState {
    Start,
    Unquoted,
    Quoted,
    AfterQuote,
}

fn csv_records(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let source = std::str::from_utf8(bytes)
        .map_err(|_| "source must be UTF-8 CSV; legacy encodings and workbooks are unsupported")?;
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut chars = source.chars().peekable();
    let mut records = Vec::new();
    let mut current = Vec::new();
    let mut field = String::new();
    let mut state = FieldState::Start;
    let mut saw_content = false;
    while let Some(ch) = chars.next() {
        saw_content = true;
        match (state, ch) {
            (FieldState::Quoted, '"') => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    state = FieldState::AfterQuote;
                }
            }
            (FieldState::Quoted, _) => field.push(ch),
            (FieldState::Start, '"') => state = FieldState::Quoted,
            (_, ',') => {
                current.push(std::mem::take(&mut field));
                state = FieldState::Start;
                if current.len() >= MAX_COLUMNS {
                    return Err("CSV exceeds the 256-column limit".into());
                }
            }
            (_, '\r' | '\n') => {
                if ch == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                current.push(std::mem::take(&mut field));
                if current.len() > MAX_COLUMNS {
                    return Err("CSV exceeds the 256-column limit".into());
                }
                records.push(std::mem::take(&mut current));
                if records.len() > MAX_ROWS + 1 {
                    return Err("CSV exceeds the 100,000-data-row limit".into());
                }
                state = FieldState::Start;
                saw_content = false;
            }
            (FieldState::AfterQuote, _) => {
                return Err("characters after a closing CSV quote are not allowed".into());
            }
            (FieldState::Unquoted, '"') => {
                return Err("quotes inside an unquoted CSV field are not allowed".into());
            }
            (_, other) => {
                field.push(other);
                state = FieldState::Unquoted;
            }
        }
    }
    if state == FieldState::Quoted {
        return Err("unterminated quoted CSV field".into());
    }
    if saw_content || !current.is_empty() {
        current.push(field);
        if current.len() > MAX_COLUMNS {
            return Err("CSV exceeds the 256-column limit".into());
        }
        records.push(current);
    }
    if records.is_empty() {
        return Err("CSV source is empty".into());
    }
    Ok(records)
}

fn valid_month(period: &str) -> bool {
    let b = period.as_bytes();
    b.len() == 7
        && b[4] == b'-'
        && b[..4].iter().chain(&b[5..7]).all(u8::is_ascii_digit)
        && &period[..4] != "0000"
        && matches!(
            &period[5..7],
            "01" | "02" | "03" | "04" | "05" | "06" | "07" | "08" | "09" | "10" | "11" | "12"
        )
}

fn invoice_month(value: &str) -> Result<String, String> {
    let b = value.as_bytes();
    if b.len() != 10
        || b[7] != b'-'
        || !valid_month(&value[..7])
        || !b[8..10].iter().all(u8::is_ascii_digit)
    {
        return Err("invoice date must be an ISO YYYY-MM-DD date".into());
    }
    let year = value[..4]
        .parse::<u32>()
        .map_err(|_| "invalid invoice year")?;
    let month = value[5..7]
        .parse::<usize>()
        .map_err(|_| "invalid invoice month")?;
    let day = value[8..10]
        .parse::<usize>()
        .map_err(|_| "invalid invoice day")?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day == 0 || day > days[month - 1] {
        return Err("invoice date is not a valid calendar day".into());
    }
    Ok(value[..7].to_string())
}

fn header_index(headers: &[String], name: &str) -> Result<usize, String> {
    headers
        .iter()
        .position(|value| value.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("declared CSV column '{name}' does not exist"))
}

pub fn parse_normalized_csv(
    bytes: &[u8],
    layout: &CsvLayout<'_>,
) -> Result<Vec<CompareRow>, String> {
    let mut records = csv_records(bytes)?.into_iter();
    let headers = records.next().ok_or("CSV header is missing")?;
    if headers.is_empty()
        || headers.iter().any(|name| {
            name.is_empty() || name.trim() != name || name.chars().any(char::is_control)
        })
    {
        return Err("CSV header contains blank, padded, or control-character names".into());
    }
    for (i, name) in headers.iter().enumerate() {
        if headers[..i]
            .iter()
            .any(|other| other.eq_ignore_ascii_case(name))
        {
            return Err("CSV header contains duplicate names".into());
        }
    }
    let key_index = header_index(&headers, layout.key_column)?;
    let period_index = header_index(&headers, layout.period_column)?;
    let mut numeric = Vec::new();
    for column in layout.amount_columns {
        let index = header_index(&headers, column)?;
        if index == key_index || index == period_index {
            return Err("numeric, key and period column identities cannot overlap".into());
        }
        numeric.push((column, index));
    }
    let mut normalized = Vec::new();
    for (row_number, record) in records.enumerate() {
        if record.len() != headers.len() {
            return Err(format!(
                "CSV row {} has a different column count",
                row_number + 2
            ));
        }
        let period = record[period_index].as_str();
        let period = match layout.period_basis {
            PeriodBasis::InvoiceMonth => invoice_month(period)?,
            _ => {
                if !valid_month(period) {
                    return Err(format!(
                        "CSV row {} has invalid YYYY-MM period",
                        row_number + 2
                    ));
                }
                period.to_string()
            }
        };
        let mut numeric_values = BTreeMap::new();
        for (name, index) in &numeric {
            let raw = &record[*index];
            if raw.trim() != raw
                || raw.is_empty()
                || !raw
                    .trim_start_matches(|ch| ch == '-' || ch == '+')
                    .chars()
                    .all(|ch| ch.is_ascii_digit())
                || raw
                    .trim_start_matches(|ch| ch == '-' || ch == '+')
                    .is_empty()
            {
                return Err(format!(
                    "CSV row {} column '{}' must be signed integer minor units; no rounding or decimal-scale inference",
                    row_number + 2, name
                ));
            }
            let amount = raw.parse::<i64>().map_err(|_| {
                format!(
                    "CSV row {} column '{}' overflows signed integer minor units",
                    row_number + 2,
                    name
                )
            })?;
            numeric_values.insert((*name).clone(), amount);
        }
        let mut row = CompareRow {
            business_key: record[key_index].clone(),
            filing_period: None,
            invoice_month: None,
            accounting_period: None,
            numeric_values,
        };
        match layout.period_basis {
            PeriodBasis::FilingPeriod => row.filing_period = Some(period),
            PeriodBasis::InvoiceMonth => row.invoice_month = Some(period),
            PeriodBasis::AccountingPeriod => row.accounting_period = Some(period),
        }
        normalized.push(row);
    }
    if normalized.is_empty() {
        return Err("CSV must contain at least one data row".into());
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn layout<'a>(amount_columns: &'a [String], basis: PeriodBasis) -> CsvLayout<'a> {
        CsvLayout {
            key_column: "Key",
            period_column: "Period",
            period_basis: basis,
            amount_columns,
        }
    }
    #[test]
    fn quoted_fields_and_crlf_are_parsed_without_guessing_amount_scale() {
        let amounts = ["Amount".to_string()];
        let rows = parse_normalized_csv(
            b"Key,Period,Amount\r\n\"A,1\",2026-08,100\r\n\"B\"\"2\",2026-09,-42\r\n",
            &layout(&amounts, PeriodBasis::FilingPeriod),
        )
        .expect("valid CSV");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].business_key, "A,1");
        assert_eq!(rows[1].business_key, "B\"2");
        assert_eq!(rows[1].numeric_values["Amount"], -42);
    }
    #[test]
    fn date_basis_uses_invoice_calendar_month_and_validates_day() {
        let amounts = ["Amount".to_string()];
        let rows = parse_normalized_csv(
            b"Key,Period,Amount\nA,2026-07-29,200\n",
            &layout(&amounts, PeriodBasis::InvoiceMonth),
        )
        .expect("date source");
        assert_eq!(rows[0].invoice_month.as_deref(), Some("2026-07"));
        assert!(parse_normalized_csv(
            b"Key,Period,Amount\nA,2026-02-30,200\n",
            &layout(&amounts, PeriodBasis::InvoiceMonth)
        )
        .is_err());
    }
    #[test]
    fn unsupported_decimals_and_ambiguous_headers_are_rejected() {
        let amounts = ["Amount".to_string()];
        let selected = layout(&amounts, PeriodBasis::FilingPeriod);
        assert!(parse_normalized_csv(b"Key,Period,Amount\nA,2026-08,1.23\n", &selected).is_err());
        assert!(
            parse_normalized_csv(b"Key,Period,Amount,amount\nA,2026-08,100,100\n", &selected)
                .is_err()
        );
        assert!(parse_normalized_csv(b"Key,Period,Amount\n\"A,2026-08,100\n", &selected).is_err());
    }
    #[test]
    fn source_rows_have_exact_declared_roles_without_date_fallback() {
        let amounts = ["Amount".to_string()];
        let selected = layout(&amounts, PeriodBasis::FilingPeriod);
        assert!(parse_normalized_csv(b"Key,Period,Amount\nA,2026-07-29,100\n", &selected).is_err());
        let rows =
            parse_normalized_csv(b"Key,Period,Amount\nA,2026-08,100\n", &selected).expect("valid");
        assert_eq!(rows[0].filing_period.as_deref(), Some("2026-08"));
        assert_eq!(rows[0].invoice_month, None);
    }
}
