// @story 06.5
// @implements AC-03, AC-05 (.csv/.tsv/.pdf -> ChartSpecV12 Normalizer with Largest-Remainder Rounding & Raw Unit Preservation)
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::charts_2d::{
    validate_chart_spec_v12, ChartColorToken, ChartDataPoint, ChartKind2D, ChartSeries,
    ChartSpecV12, ChartValidationError, ValidatedChartSpec, CHART_MAX_ABS_VALUE,
    CHART_MAX_LABEL_CHARS, CHART_MAX_POINTS_PER_SERIES, CHART_MAX_SERIES, CHART_MAX_UNIT_CHARS,
    CHART_PROPORTIONAL_SUM_TOLERANCE,
};

pub const MAX_SERIES: usize = CHART_MAX_SERIES;
pub const MAX_POINTS: usize = CHART_MAX_POINTS_PER_SERIES;
pub const MAX_PDF_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_PDF_PAGES: usize = 40;
pub const MAX_CSV_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_DECOMPRESSED_PDF_STREAM_BYTES: usize = 4 * 1024 * 1024;

const COLOR_CYCLE: &[ChartColorToken] = &[
    ChartColorToken::HumanAccent,
    ChartColorToken::AgentAccent,
    ChartColorToken::StateSuccess,
    ChartColorToken::StateInfo,
    ChartColorToken::StateWarning,
    ChartColorToken::TextSecondary,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartSourceFormat {
    Csv,
    Tsv,
    Pdf,
}

impl ChartSourceFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Pdf => "pdf",
        }
    }

    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "csv" => Some(Self::Csv),
            "tsv" => Some(Self::Tsv),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NormalizedChartKind {
    Bar,
    Line,
    Pie,
    Donut,
    Funnel,
    Kpi,
}

impl NormalizedChartKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Line => "line",
            Self::Pie => "pie",
            Self::Donut => "donut",
            Self::Funnel => "funnel",
            Self::Kpi => "kpi",
        }
    }

    pub fn to_chart_kind_2d(self) -> ChartKind2D {
        match self {
            Self::Bar | Self::Kpi => ChartKind2D::Bar,
            Self::Line => ChartKind2D::Line,
            Self::Pie => ChartKind2D::Pie,
            Self::Donut => ChartKind2D::Donut,
            Self::Funnel => ChartKind2D::Funnel,
        }
    }

    pub fn is_proportional(self) -> bool {
        matches!(self, Self::Pie | Self::Donut | Self::Funnel)
    }
}

#[derive(Debug, Clone)]
pub struct ChartNormalizeOptions {
    pub title: Option<String>,
    pub chart_kind: NormalizedChartKind,
    pub max_series: usize,
    pub max_points: usize,
}

impl Default for ChartNormalizeOptions {
    fn default() -> Self {
        Self {
            title: None,
            chart_kind: NormalizedChartKind::Bar,
            max_series: MAX_SERIES,
            max_points: MAX_POINTS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedPointRaw {
    pub series_name: String,
    pub category: String,
    pub raw_display: String,
    pub raw_value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub normalized_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedChartOutput {
    pub spec: ChartSpecV12,
    pub canonical_charts_json: String,
    pub aria_label: String,
    pub estimated_svg_primitives: usize,
    pub requested_kind: NormalizedChartKind,
    pub source_format: ChartSourceFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proportional_sum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_unit: Option<String>,
    pub row_count: usize,
    pub series_count: usize,
    pub raw_points: Vec<NormalizedPointRaw>,
}

#[derive(Debug, Error, PartialEq)]
pub enum ChartNormalizerError {
    #[error("E_CHART_SOURCE_EMPTY: {0}")]
    EmptySource(String),
    #[error("E_CHART_NON_FINITE: Non-finite numeric cell '{raw}' at row {row}, col {col}")]
    NonFiniteValue { row: usize, col: usize, raw: String },
    #[error(
        "E_CHART_NEGATIVE_PROPORTIONAL: Negative value {value} at row {row}, col {col} is forbidden for proportional charts"
    )]
    NegativeProportional { row: usize, col: usize, value: f64 },
    #[error(
        "E_CHART_SUM_NON_POSITIVE: Series '{series_name}' sum ({sum}) must be strictly > 0 for proportional charts"
    )]
    NonPositiveProportionalSum { series_name: String, sum: f64 },
    #[error("E_CHART_MAX_SERIES_EXCEEDED: Series count {actual} exceeds limit {max}")]
    SeriesLimitExceeded { actual: usize, max: usize },
    #[error("E_CHART_MAX_POINTS_EXCEEDED: Point count {actual} exceeds limit {max}")]
    PointsLimitExceeded { actual: usize, max: usize },
    #[error("E_PDF_TOO_LARGE: PDF byte length {actual} exceeds {max} bytes")]
    PdfTooLarge { actual: usize, max: usize },
    #[error("E_PDF_PAGE_LIMIT_EXCEEDED: PDF page count {actual} exceeds {max} pages")]
    PdfPageLimitExceeded { actual: usize, max: usize },
    #[error(
        "SCANNED_PDF_NO_TEXT_TABLES: Scanned or image-only PDF detected (0 extractable text tables). Please paste CSV/TSV rows or export text tables from your spreadsheet."
    )]
    ScannedPdfNoTextTables,
    #[error("E_CHART_TABLE_INVALID: {0}")]
    InvalidTableFormat(String),
    #[error("{0}")]
    Validation(#[from] ChartValidationError),
}

impl ChartNormalizerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::EmptySource(_) => "E_CHART_SOURCE_EMPTY",
            Self::NonFiniteValue { .. } => "E_CHART_NON_FINITE",
            Self::NegativeProportional { .. } => "E_CHART_NEGATIVE_PROPORTIONAL",
            Self::NonPositiveProportionalSum { .. } => "E_CHART_SUM_NON_POSITIVE",
            Self::SeriesLimitExceeded { .. } => "E_CHART_MAX_SERIES_EXCEEDED",
            Self::PointsLimitExceeded { .. } => "E_CHART_MAX_POINTS_EXCEEDED",
            Self::PdfTooLarge { .. } => "E_PDF_TOO_LARGE",
            Self::PdfPageLimitExceeded { .. } => "E_PDF_PAGE_LIMIT_EXCEEDED",
            Self::ScannedPdfNoTextTables => "SCANNED_PDF_NO_TEXT_TABLES",
            Self::InvalidTableFormat(_) => "E_CHART_TABLE_INVALID",
            Self::Validation(v) => v.code(),
        }
    }

    pub fn non_tech_guidance(&self) -> Option<&'static str> {
        match self {
            Self::ScannedPdfNoTextTables => Some(
                "This PDF appears to be an image scan without selectable text tables. Copy rows from Excel/Google Sheets and paste as CSV or TSV instead.",
            ),
            Self::EmptySource(_) => Some(
                "Upload a CSV, TSV, or text-based PDF containing at least one header row and one data row.",
            ),
            Self::NegativeProportional { .. } => Some(
                "Pie, Donut, and Funnel charts require zero or positive values. Switch to a Bar or Line chart to display negative deltas.",
            ),
            Self::NonPositiveProportionalSum { .. } => Some(
                "Proportional charts require a positive total greater than zero. Check that your value column contains positive numbers.",
            ),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedNumericCell {
    pub value: f64,
    pub raw_display: String,
    pub unit: Option<String>,
}

/// Parses US (`$1,234.56`) and EU/VN (`1.234,56 €`, `42,5M VND`) numeric cells while preserving raw unit strings.
pub fn parse_localized_number_with_unit(raw: &str) -> Result<ParsedNumericCell, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty numeric cell".to_string());
    }
    // Strip leading single-quote added by CAS CSV formula neutralization (e.g. `'-12.5` or `'+50`)
    let unquoted = trimmed
        .strip_prefix('\'')
        .filter(|rest| rest.starts_with(['-', '+', '=', '@']))
        .unwrap_or(trimmed);

    let lower = unquoted.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "nan"
            | "+nan"
            | "-nan"
            | "inf"
            | "+inf"
            | "-inf"
            | "infinity"
            | "+infinity"
            | "-infinity"
            | "#n/a"
            | "#value!"
            | "#div/0!"
            | "#num!"
            | "null"
            | "undefined"
    ) {
        return Err(format!("non-finite token '{trimmed}'"));
    }

    let raw_display = unquoted.to_string();
    let chars: Vec<char> = unquoted.chars().collect();
    let mut start = 0usize;
    let mut sign = 1.0f64;

    // Leading sign or currency prefix (e.g. `-$1,234.56` or `$-1,234.56` or `(1,234.56)`)
    let accounting_negative = chars.first() == Some(&'(') && chars.last() == Some(&')');
    let effective_end = if accounting_negative {
        start = 1;
        sign = -1.0;
        chars.len().saturating_sub(1)
    } else {
        chars.len()
    };

    let mut prefix_unit = String::new();
    let mut seen_sign = accounting_negative;
    while start < effective_end {
        let ch = chars[start];
        if ch.is_ascii_digit() {
            break;
        }
        if ch == '-' {
            if seen_sign {
                return Err(format!("multiple signs in '{trimmed}'"));
            }
            seen_sign = true;
            sign = -sign;
            start += 1;
            continue;
        }
        if ch == '+' {
            if seen_sign {
                return Err(format!("multiple signs in '{trimmed}'"));
            }
            seen_sign = true;
            start += 1;
            continue;
        }
        if ch.is_whitespace() {
            start += 1;
            continue;
        }
        prefix_unit.push(ch);
        start += 1;
    }

    let mut num_end = start;
    while num_end < effective_end {
        let ch = chars[num_end];
        if ch.is_ascii_digit() || ch == '.' || ch == ',' || ch == '_' || ch == ' ' {
            num_end += 1;
        } else {
            break;
        }
    }

    let num_slice: String = chars[start..num_end]
        .iter()
        .copied()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .collect();
    if num_slice.is_empty() || !num_slice.chars().any(|c| c.is_ascii_digit()) {
        return Err(format!("missing digits in '{trimmed}'"));
    }

    // Parse optional scientific notation exponent (`1e5`, `2.5E-3`, `3e+6`) directly into the number
    // instead of splitting `e5` into a fake unit suffix.
    let mut exponent_part = String::new();
    if num_end < effective_end && matches!(chars[num_end], 'e' | 'E') {
        let mut exp_cursor = num_end + 1;
        let mut exp_sign = String::new();
        if exp_cursor < effective_end && matches!(chars[exp_cursor], '+' | '-') {
            exp_sign.push(chars[exp_cursor]);
            exp_cursor += 1;
        }
        let digits_start = exp_cursor;
        while exp_cursor < effective_end && chars[exp_cursor].is_ascii_digit() {
            exp_cursor += 1;
        }
        if exp_cursor > digits_start {
            let exp_digits: String = chars[digits_start..exp_cursor].iter().collect();
            exponent_part = format!("e{exp_sign}{exp_digits}");
            num_end = exp_cursor;
        }
    }

    let suffix_unit: String = chars[num_end..effective_end]
        .iter()
        .collect::<String>()
        .trim()
        .to_string();

    let canonical_mantissa = normalize_locale_digits(&num_slice)?;
    let canonical_num = if exponent_part.is_empty() {
        canonical_mantissa
    } else {
        format!("{canonical_mantissa}{exponent_part}")
    };
    let parsed = canonical_num
        .parse::<f64>()
        .map_err(|_| format!("invalid number '{trimmed}'"))?;
    if !parsed.is_finite() {
        return Err(format!("non-finite parsed value '{trimmed}'"));
    }

    let combined_unit = match (prefix_unit.trim(), suffix_unit.as_str()) {
        ("", "") => None,
        (p, "") => Some(p.to_string()),
        ("", s) => Some(s.to_string()),
        (p, s) => Some(format!("{p} {s}")),
    }
    .map(|u| u.chars().take(CHART_MAX_UNIT_CHARS).collect::<String>());

    Ok(ParsedNumericCell {
        value: sign * parsed,
        raw_display,
        unit: combined_unit,
    })
}

fn is_grouped_thousands(s: &str, sep: char) -> bool {
    let parts: Vec<&str> = s.split(sep).collect();
    if parts.len() < 2 {
        return false;
    }
    let first_ok =
        !parts[0].is_empty() && parts[0].len() <= 3 && parts[0].chars().all(|c| c.is_ascii_digit());
    first_ok
        && parts[1..]
            .iter()
            .all(|p| p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))
}

fn normalize_locale_digits(num_str: &str) -> Result<String, String> {
    let dot_count = num_str.matches('.').count();
    let comma_count = num_str.matches(',').count();

    if dot_count > 0 && comma_count > 0 {
        let last_dot = num_str.rfind('.').unwrap_or(0);
        let last_comma = num_str.rfind(',').unwrap_or(0);
        if last_comma > last_dot {
            // EU/VN format: `1.234,56`
            if comma_count != 1 {
                return Err(format!("invalid EU/VN decimal format '{num_str}'"));
            }
            let int_part = &num_str[..last_comma];
            let frac_part = &num_str[last_comma + 1..];
            if !is_grouped_thousands(int_part, '.')
                || frac_part.is_empty()
                || !frac_part.chars().all(|c| c.is_ascii_digit())
            {
                return Err(format!("invalid EU/VN thousands grouping '{num_str}'"));
            }
            return Ok(format!("{}.{}", int_part.replace('.', ""), frac_part));
        }
        // US format: `1,234.56`
        if dot_count != 1 {
            return Err(format!("invalid US decimal format '{num_str}'"));
        }
        let int_part = &num_str[..last_dot];
        let frac_part = &num_str[last_dot + 1..];
        if !is_grouped_thousands(int_part, ',')
            || frac_part.is_empty()
            || !frac_part.chars().all(|c| c.is_ascii_digit())
        {
            return Err(format!("invalid US thousands grouping '{num_str}'"));
        }
        return Ok(format!("{}.{}", int_part.replace(',', ""), frac_part));
    }

    if comma_count > 0 {
        if comma_count > 1 {
            if is_grouped_thousands(num_str, ',') {
                return Ok(num_str.replace(',', ""));
            }
            return Err(format!("invalid thousands grouping in '{num_str}'"));
        }
        if is_grouped_thousands(num_str, ',') {
            return Ok(num_str.replace(',', ""));
        }
        return Ok(num_str.replace(',', "."));
    }

    if dot_count > 1 {
        if is_grouped_thousands(num_str, '.') {
            return Ok(num_str.replace('.', ""));
        }
        return Err(format!("ambiguous decimal separators in '{num_str}'"));
    }

    Ok(num_str.to_string())
}

/// Applies deterministic Hamilton / largest-remainder rounding so percentages sum to exactly `100.00` (within `±0.05`).
pub fn largest_remainder_percentages(values: &[f64]) -> Result<Vec<f64>, ChartNormalizerError> {
    if values.is_empty() {
        return Err(ChartNormalizerError::EmptySource(
            "No values available for proportional rounding".to_string(),
        ));
    }
    for (idx, &v) in values.iter().enumerate() {
        if !v.is_finite() {
            return Err(ChartNormalizerError::NonFiniteValue {
                row: idx + 1,
                col: 1,
                raw: format!("{v}"),
            });
        }
        if v < 0.0 {
            return Err(ChartNormalizerError::NegativeProportional {
                row: idx + 1,
                col: 1,
                value: v,
            });
        }
    }
    let sum: f64 = values.iter().sum();
    if !sum.is_finite() || sum <= 0.0 {
        return Err(ChartNormalizerError::NonPositiveProportionalSum {
            series_name: "Series".to_string(),
            sum,
        });
    }

    const TOTAL_BASIS_POINTS: i64 = 10_000;
    let mut floors: Vec<i64> = Vec::with_capacity(values.len());
    let mut remainders: Vec<(usize, f64)> = Vec::with_capacity(values.len());
    let mut floor_sum: i64 = 0;

    for (idx, &v) in values.iter().enumerate() {
        let exact = (v / sum) * (TOTAL_BASIS_POINTS as f64);
        let floored = exact.floor() as i64;
        let rem = exact - (floored as f64);
        floors.push(floored);
        remainders.push((idx, rem));
        floor_sum += floored;
    }

    let deficit = (TOTAL_BASIS_POINTS - floor_sum).max(0) as usize;
    remainders.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    for &(idx, _) in remainders.iter().take(deficit) {
        floors[idx] += 1;
    }

    Ok(floors.into_iter().map(|bp| (bp as f64) / 100.0).collect())
}

/// Parses RFC-4180 CSV or TSV text into rectangular table rows.
pub fn parse_delimited_table(raw_text: &str, delimiter: char) -> Vec<Vec<String>> {
    let cleaned = raw_text.trim_start_matches('\u{feff}');
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();
    let mut in_quotes = false;
    let mut chars = cleaned.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current_cell.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                current_cell.push(ch);
            }
            continue;
        }

        if ch == '"' && current_cell.trim().is_empty() {
            in_quotes = true;
            current_cell.clear();
        } else if ch == delimiter {
            current_row.push(current_cell.trim().to_string());
            current_cell.clear();
        } else if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            current_row.push(current_cell.trim().to_string());
            current_cell.clear();
            if current_row.iter().any(|c| !c.is_empty()) {
                rows.push(std::mem::take(&mut current_row));
            } else {
                current_row.clear();
            }
        } else if ch == '\n' {
            current_row.push(current_cell.trim().to_string());
            current_cell.clear();
            if current_row.iter().any(|c| !c.is_empty()) {
                rows.push(std::mem::take(&mut current_row));
            } else {
                current_row.clear();
            }
        } else {
            current_cell.push(ch);
        }
    }

    if !current_cell.is_empty() || !current_row.is_empty() {
        current_row.push(current_cell.trim().to_string());
        if current_row.iter().any(|c| !c.is_empty()) {
            rows.push(current_row);
        }
    }
    rows
}

fn count_pdf_pages(raw: &[u8]) -> usize {
    let text = String::from_utf8_lossy(raw);
    let mut count = 0usize;
    let bytes = text.as_bytes();
    let needle = b"/Type";
    let mut pos = 0usize;

    while let Some(rel) = find_subslice(&bytes[pos..], needle) {
        let mut cursor = pos + rel + needle.len();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes[cursor..].starts_with(b"/Page") {
            let next_byte = bytes.get(cursor + 5).copied();
            let is_pages_plural = matches!(next_byte, Some(b's' | b'S' | b'L' | b'l'));
            if !is_pages_plural {
                count += 1;
            }
        }
        pos = cursor.max(pos + rel + needle.len());
    }
    count.max(usize::from(text.contains("%PDF-")))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn unescape_pdf_literal_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('(') => out.push('('),
                Some(')') => out.push(')'),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => break,
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn extract_pdf_text_lines(bytes: &[u8]) -> Vec<String> {
    let lossy = String::from_utf8_lossy(bytes);
    let mut lines: Vec<String> = Vec::new();
    let mut cursor = 0usize;

    while let Some(bt_rel) = lossy[cursor..].find("BT") {
        let bt_start = cursor + bt_rel + 2;
        let Some(et_rel) = lossy[bt_start..].find("ET") else {
            break;
        };
        let block = &lossy[bt_start..bt_start + et_rel];
        let mut current_line = String::new();
        let bchars: Vec<char> = block.chars().collect();
        let mut i = 0usize;
        while i < bchars.len() {
            if bchars[i] == '(' {
                i += 1;
                let mut depth = 1usize;
                let mut lit = String::new();
                while i < bchars.len() && depth > 0 {
                    let c = bchars[i];
                    if c == '\\' && i + 1 < bchars.len() {
                        lit.push(c);
                        lit.push(bchars[i + 1]);
                        i += 2;
                        continue;
                    }
                    if c == '(' {
                        depth += 1;
                        lit.push(c);
                    } else if c == ')' {
                        depth -= 1;
                        if depth > 0 {
                            lit.push(c);
                        }
                    } else {
                        lit.push(c);
                    }
                    i += 1;
                }
                let decoded = unescape_pdf_literal_string(&lit);
                if !decoded.trim().is_empty() {
                    if !current_line.is_empty() {
                        current_line.push('\t');
                    }
                    current_line.push_str(decoded.trim());
                }
            } else if i + 1 < bchars.len()
                && bchars[i] == 'T'
                && matches!(bchars[i + 1], '*' | 'd' | 'D')
            {
                if !current_line.trim().is_empty() {
                    lines.push(std::mem::take(&mut current_line));
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        if !current_line.trim().is_empty() {
            lines.push(current_line);
        }
        cursor = bt_start + et_rel + 2;
    }

    lines
}

fn split_pdf_table_line(line: &str) -> Vec<String> {
    if line.contains('\t') {
        return line
            .split('\t')
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
    }
    if line.contains('|') {
        return line
            .split('|')
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
    }
    if line.contains(',') && !line.contains("  ") {
        let csv_rows = parse_delimited_table(line, ',');
        if let Some(first) = csv_rows.into_iter().next() {
            if first.len() >= 2 {
                return first;
            }
        }
    }
    // Split on 2+ spaces for column-aligned PDF tables
    let mut cells = Vec::new();
    let mut buf = String::new();
    let mut spaces = 0usize;
    for ch in line.chars() {
        if ch == ' ' {
            spaces += 1;
            if spaces == 1 {
                buf.push(' ');
            } else if spaces == 2 && !buf.trim().is_empty() {
                cells.push(buf.trim().to_string());
                buf.clear();
            }
        } else {
            spaces = 0;
            buf.push(ch);
        }
    }
    if !buf.trim().is_empty() {
        cells.push(buf.trim().to_string());
    }
    cells
}

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buf: u32,
    bit_count: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_buf: 0,
            bit_count: 0,
        }
    }

    fn read_bit(&mut self) -> Result<u32, String> {
        if self.bit_count == 0 {
            let byte = *self
                .data
                .get(self.byte_pos)
                .ok_or_else(|| "unexpected EOF in DEFLATE bitstream".to_string())?;
            self.byte_pos += 1;
            self.bit_buf = u32::from(byte);
            self.bit_count = 8;
        }
        let bit = self.bit_buf & 1;
        self.bit_buf >>= 1;
        self.bit_count -= 1;
        Ok(bit)
    }

    fn read_bits(&mut self, n: u8) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        while self.bit_count < n {
            let byte = *self
                .data
                .get(self.byte_pos)
                .ok_or_else(|| "unexpected EOF in DEFLATE bitstream".to_string())?;
            self.byte_pos += 1;
            self.bit_buf |= u32::from(byte) << self.bit_count;
            self.bit_count += 8;
        }
        let mask = (1u32 << n) - 1;
        let val = self.bit_buf & mask;
        self.bit_buf >>= n;
        self.bit_count -= n;
        Ok(val)
    }

    fn align_to_byte(&mut self) {
        let discard = self.bit_count % 8;
        self.bit_buf >>= discard;
        self.bit_count -= discard;
        let unread_full = usize::from(self.bit_count / 8);
        self.byte_pos = self.byte_pos.saturating_sub(unread_full);
        self.bit_buf = 0;
        self.bit_count = 0;
    }

    fn read_u16_le(&mut self) -> Result<u16, String> {
        if self.byte_pos + 2 > self.data.len() {
            return Err("unexpected EOF reading stored DEFLATE length".to_string());
        }
        let val = u16::from_le_bytes([self.data[self.byte_pos], self.data[self.byte_pos + 1]]);
        self.byte_pos += 2;
        Ok(val)
    }

    fn read_exact_slice(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .byte_pos
            .checked_add(len)
            .ok_or_else(|| "DEFLATE slice offset overflow".to_string())?;
        if end > self.data.len() {
            return Err("unexpected EOF reading stored DEFLATE payload".to_string());
        }
        let slice = &self.data[self.byte_pos..end];
        self.byte_pos = end;
        Ok(slice)
    }
}

struct HuffmanTree {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl HuffmanTree {
    fn from_lengths(lengths: &[u8]) -> Result<Self, String> {
        let mut counts = [0u16; 16];
        for &len in lengths {
            if len > 15 {
                return Err("DEFLATE Huffman code length > 15".to_string());
            }
            if len > 0 {
                let slot = &mut counts[usize::from(len)];
                *slot = slot
                    .checked_add(1)
                    .ok_or_else(|| "DEFLATE Huffman count overflow".to_string())?;
            }
        }
        let mut left: i32 = 1;
        for &cnt in counts.iter().skip(1) {
            left <<= 1;
            left -= i32::from(cnt);
            if left < 0 {
                return Err("over-subscribed DEFLATE Huffman tree".to_string());
            }
        }
        let mut offsets = [0u16; 16];
        for bits in 1..15 {
            offsets[bits + 1] = offsets[bits] + counts[bits];
        }
        let total: usize = counts[1..=15].iter().map(|&c| usize::from(c)).sum();
        let mut symbols = vec![0u16; total];
        for (sym, &len) in lengths.iter().enumerate() {
            if len > 0 {
                let b = usize::from(len);
                let idx = usize::from(offsets[b]);
                symbols[idx] = sym as u16;
                offsets[b] += 1;
            }
        }
        Ok(Self { counts, symbols })
    }

    fn decode(&self, reader: &mut BitReader<'_>) -> Result<u16, String> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for bits in 1..=15 {
            code |= reader.read_bit()? as i32;
            let count = i32::from(self.counts[bits]);
            if code - count < first {
                let sym_idx = (index + (code - first)) as usize;
                return self
                    .symbols
                    .get(sym_idx)
                    .copied()
                    .ok_or_else(|| "DEFLATE Huffman symbol out of bounds".to_string());
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err("invalid DEFLATE Huffman code".to_string())
    }
}

fn build_fixed_huffman_trees() -> Result<(HuffmanTree, HuffmanTree), String> {
    let mut lit_lens = [0u8; 288];
    lit_lens[..144].fill(8);
    lit_lens[144..256].fill(9);
    lit_lens[256..280].fill(7);
    lit_lens[280..288].fill(8);
    let dist_lens = [5u8; 32];
    Ok((
        HuffmanTree::from_lengths(&lit_lens)?,
        HuffmanTree::from_lengths(&dist_lens)?,
    ))
}

fn read_dynamic_huffman_trees(
    reader: &mut BitReader<'_>,
) -> Result<(HuffmanTree, HuffmanTree), String> {
    const CL_ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let hlit = (reader.read_bits(5)? as usize) + 257;
    let hdist = (reader.read_bits(5)? as usize) + 1;
    let hclen = (reader.read_bits(4)? as usize) + 4;
    if hlit > 286 || hdist > 32 {
        return Err("invalid dynamic DEFLATE table counts".to_string());
    }
    let mut cl_lens = [0u8; 19];
    for &idx in CL_ORDER.iter().take(hclen) {
        cl_lens[idx] = reader.read_bits(3)? as u8;
    }
    let cl_tree = HuffmanTree::from_lengths(&cl_lens)?;
    let total = hlit + hdist;
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        let sym = cl_tree.decode(reader)?;
        match sym {
            0..=15 => lengths.push(sym as u8),
            16 => {
                let prev = *lengths
                    .last()
                    .ok_or_else(|| "DEFLATE repeat without previous code length".to_string())?;
                let repeat = (reader.read_bits(2)? as usize) + 3;
                if lengths.len() + repeat > total {
                    return Err("DEFLATE code length repeat overflow".to_string());
                }
                lengths.resize(lengths.len() + repeat, prev);
            }
            17 => {
                let repeat = (reader.read_bits(3)? as usize) + 3;
                if lengths.len() + repeat > total {
                    return Err("DEFLATE zero repeat overflow".to_string());
                }
                lengths.resize(lengths.len() + repeat, 0);
            }
            18 => {
                let repeat = (reader.read_bits(7)? as usize) + 11;
                if lengths.len() + repeat > total {
                    return Err("DEFLATE long zero repeat overflow".to_string());
                }
                lengths.resize(lengths.len() + repeat, 0);
            }
            _ => return Err("invalid DEFLATE code length symbol".to_string()),
        }
    }
    Ok((
        HuffmanTree::from_lengths(&lengths[..hlit])?,
        HuffmanTree::from_lengths(&lengths[hlit..])?,
    ))
}

fn decode_huffman_block(
    reader: &mut BitReader<'_>,
    lit_tree: &HuffmanTree,
    dist_tree: &HuffmanTree,
    out: &mut Vec<u8>,
    max_out: usize,
) -> Result<(), String> {
    const LEN_BASE: [usize; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LEN_EXTRA: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DIST_BASE: [usize; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];

    loop {
        let sym = lit_tree.decode(reader)?;
        if sym < 256 {
            if out.len() >= max_out {
                return Err(format!(
                    "DEFLATE_LIMIT_EXCEEDED: output exceeded {max_out} bytes"
                ));
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else if sym <= 285 {
            let len_idx = usize::from(sym - 257);
            let length = LEN_BASE[len_idx] + (reader.read_bits(LEN_EXTRA[len_idx])? as usize);
            let dist_sym = dist_tree.decode(reader)?;
            if dist_sym > 29 {
                return Err("invalid DEFLATE distance symbol".to_string());
            }
            let dist_idx = usize::from(dist_sym);
            let dist = DIST_BASE[dist_idx] + (reader.read_bits(DIST_EXTRA[dist_idx])? as usize);
            if dist == 0 || dist > out.len() {
                return Err("DEFLATE back-reference distance out of bounds".to_string());
            }
            if out.len().saturating_add(length) > max_out {
                return Err(format!(
                    "DEFLATE_LIMIT_EXCEEDED: output exceeded {max_out} bytes"
                ));
            }
            for _ in 0..length {
                let b = out[out.len() - dist];
                out.push(b);
            }
        } else {
            return Err("invalid DEFLATE literal/length symbol".to_string());
        }
    }
}

fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + u32::from(byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

/// Pure-Rust RFC 1951 bounded raw DEFLATE decompressor (zip-bomb safe via `max_out`).
pub fn inflate_raw_deflate_bounded(
    data: &[u8],
    max_out: usize,
) -> Result<(Vec<u8>, usize), String> {
    let mut reader = BitReader::new(data);
    let mut out = Vec::new();
    loop {
        let bfinal = reader.read_bit()?;
        let btype = reader.read_bits(2)?;
        match btype {
            0 => {
                reader.align_to_byte();
                let len = reader.read_u16_le()?;
                let nlen = reader.read_u16_le()?;
                if len != !nlen {
                    return Err("corrupt stored DEFLATE block length".to_string());
                }
                let count = usize::from(len);
                if out.len().saturating_add(count) > max_out {
                    return Err(format!(
                        "DEFLATE_LIMIT_EXCEEDED: output exceeded {max_out} bytes"
                    ));
                }
                out.extend_from_slice(reader.read_exact_slice(count)?);
            }
            1 => {
                let (lit_tree, dist_tree) = build_fixed_huffman_trees()?;
                decode_huffman_block(&mut reader, &lit_tree, &dist_tree, &mut out, max_out)?;
            }
            2 => {
                let (lit_tree, dist_tree) = read_dynamic_huffman_trees(&mut reader)?;
                decode_huffman_block(&mut reader, &lit_tree, &dist_tree, &mut out, max_out)?;
            }
            _ => return Err("invalid DEFLATE block type 3".to_string()),
        }
        if bfinal == 1 {
            break;
        }
    }
    reader.align_to_byte();
    Ok((out, reader.byte_pos))
}

/// Pure-Rust RFC 1950/1951 bounded zlib (`/FlateDecode`) decompressor with strict output cap.
pub fn inflate_zlib_bounded(data: &[u8], max_out: usize) -> Result<Vec<u8>, String> {
    if data.len() < 2 {
        return Err("zlib stream shorter than 2-byte header".to_string());
    }
    let cmf = data[0];
    let flg = data[1];
    if (cmf & 0x0f) != 8 || (cmf >> 4) > 7 {
        return Err("unsupported zlib compression method or window size".to_string());
    }
    if ((u16::from(cmf) << 8) | u16::from(flg)) % 31 != 0 {
        return Err("invalid zlib header checksum".to_string());
    }
    if (flg & 0x20) != 0 {
        return Err("preset zlib dictionaries are unsupported".to_string());
    }
    let (out, consumed) = inflate_raw_deflate_bounded(&data[2..], max_out)?;
    let trailer_pos = 2 + consumed;
    if trailer_pos + 4 <= data.len() {
        let expected = u32::from_be_bytes([
            data[trailer_pos],
            data[trailer_pos + 1],
            data[trailer_pos + 2],
            data[trailer_pos + 3],
        ]);
        let actual = adler32(&out);
        if expected != actual {
            return Err("zlib Adler-32 checksum mismatch".to_string());
        }
    }
    Ok(out)
}

fn extract_pdf_streams_text_lines(bytes: &[u8]) -> Result<Vec<String>, ChartNormalizerError> {
    let mut lines: Vec<String> = Vec::new();
    let mut remaining_budget = MAX_DECOMPRESSED_PDF_STREAM_BYTES;
    let mut cursor = 0usize;
    let stream_kw = b"stream";
    let endstream_kw = b"endstream";

    while let Some(rel) = find_subslice(&bytes[cursor..], stream_kw) {
        let kw_pos = cursor + rel;
        let prev_byte = kw_pos.checked_sub(1).and_then(|i| bytes.get(i)).copied();
        if prev_byte.is_some_and(|b| b.is_ascii_alphanumeric()) {
            cursor = kw_pos + stream_kw.len();
            continue;
        }
        let after_kw = kw_pos + stream_kw.len();
        let data_start = if bytes[after_kw..].starts_with(b"\r\n") {
            after_kw + 2
        } else if bytes[after_kw..].starts_with(b"\n") || bytes[after_kw..].starts_with(b"\r") {
            after_kw + 1
        } else {
            cursor = after_kw;
            continue;
        };

        let Some(end_rel) = find_subslice(&bytes[data_start..], endstream_kw) else {
            break;
        };
        let mut data_end = data_start + end_rel;
        if data_end >= data_start + 2 && &bytes[data_end - 2..data_end] == b"\r\n" {
            data_end -= 2;
        } else if data_end > data_start && matches!(bytes[data_end - 1], b'\n' | b'\r') {
            data_end -= 1;
        }

        let dict_start = kw_pos.saturating_sub(512);
        let dict_header = String::from_utf8_lossy(&bytes[dict_start..kw_pos]);
        let is_image = dict_header.contains("/Subtype") && dict_header.contains("/Image");
        let is_flate = dict_header.contains("/FlateDecode");

        if !is_image && data_end > data_start {
            let payload = &bytes[data_start..data_end];
            if is_flate {
                match inflate_zlib_bounded(payload, remaining_budget) {
                    Ok(decompressed) => {
                        remaining_budget = remaining_budget.saturating_sub(decompressed.len());
                        lines.extend(extract_pdf_text_lines(&decompressed));
                    }
                    Err(err) if err.contains("DEFLATE_LIMIT_EXCEEDED") => {
                        return Err(ChartNormalizerError::PdfTooLarge {
                            actual: MAX_DECOMPRESSED_PDF_STREAM_BYTES + 1,
                            max: MAX_DECOMPRESSED_PDF_STREAM_BYTES,
                        });
                    }
                    Err(_) => {
                        // Ignore non-text / non-zlib auxiliary streams (e.g. font/ICC/metadata streams)
                    }
                }
            } else {
                lines.extend(extract_pdf_text_lines(payload));
            }
        }

        cursor = data_start + end_rel + endstream_kw.len();
    }

    if lines.is_empty() {
        lines = extract_pdf_text_lines(bytes);
    }
    Ok(lines)
}

/// Extracts tabular rows from a text-based PDF (including `/FlateDecode` compressed streams),
/// enforcing `MAX_PDF_BYTES`, `MAX_DECOMPRESSED_PDF_STREAM_BYTES`, and `MAX_PDF_PAGES`,
/// and rejecting image-only/scanned PDFs with `SCANNED_PDF_NO_TEXT_TABLES`.
pub fn extract_pdf_table_rows(bytes: &[u8]) -> Result<Vec<Vec<String>>, ChartNormalizerError> {
    if bytes.is_empty() {
        return Err(ChartNormalizerError::EmptySource(
            "PDF payload is empty".to_string(),
        ));
    }
    if bytes.len() > MAX_PDF_BYTES {
        return Err(ChartNormalizerError::PdfTooLarge {
            actual: bytes.len(),
            max: MAX_PDF_BYTES,
        });
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(ChartNormalizerError::InvalidTableFormat(
            "Invalid PDF header (missing %PDF-)".to_string(),
        ));
    }
    let page_count = count_pdf_pages(bytes);
    if page_count > MAX_PDF_PAGES {
        return Err(ChartNormalizerError::PdfPageLimitExceeded {
            actual: page_count,
            max: MAX_PDF_PAGES,
        });
    }

    let extracted_lines = extract_pdf_streams_text_lines(bytes)?;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    for line in extracted_lines {
        for subline in line.split('\n') {
            let trimmed = subline.trim();
            if trimmed.is_empty() {
                continue;
            }
            let cells = split_pdf_table_line(trimmed);
            if cells.len() >= 2 {
                table_rows.push(cells);
            }
        }
    }

    if table_rows.len() < 2 {
        return Err(ChartNormalizerError::ScannedPdfNoTextTables);
    }

    // Verify at least one data row has a numeric column (otherwise it's prose or image-only OCR noise)
    let has_numeric_row = table_rows[1..].iter().any(|row| {
        row.iter()
            .skip(1)
            .any(|cell| parse_localized_number_with_unit(cell).is_ok())
    });
    if !has_numeric_row {
        return Err(ChartNormalizerError::ScannedPdfNoTextTables);
    }

    Ok(table_rows)
}

fn format_preserved_label(category: &str, raw_display: &str, force_raw_suffix: bool) -> String {
    let cat = category.trim();
    let raw = raw_display.trim();
    let composed = if force_raw_suffix
        || raw
            .chars()
            .any(|c| !c.is_ascii_digit() && c != '.' && c != '-')
    {
        format!("{cat} ({raw})")
    } else {
        cat.to_string()
    };
    composed.chars().take(CHART_MAX_LABEL_CHARS).collect()
}

fn select_cartesian_scale(max_abs: f64) -> (f64, Option<String>) {
    if !max_abs.is_finite() || max_abs <= CHART_MAX_ABS_VALUE {
        return (1.0, None);
    }
    const NAMED_TIERS: &[(f64, &str)] = &[
        (1_000.0, "K"),
        (1_000_000.0, "M"),
        (1_000_000_000.0, "B"),
        (1_000_000_000_000.0, "T"),
    ];
    // Pick the engineering power-of-1000 tier (`K`/`M`/`B`/`T`) that keeps scaled values human-readable
    // (`< 1_000` when possible) and strictly within `CHART_MAX_ABS_VALUE`.
    for &(scale, label) in NAMED_TIERS {
        let scaled = max_abs / scale;
        if scaled < 1_000.0 {
            return (scale, Some(label.to_string()));
        }
    }
    if max_abs / 1_000_000_000_000.0 <= CHART_MAX_ABS_VALUE {
        return (1_000_000_000_000.0, Some("T".to_string()));
    }
    let mut scale = 1e15f64;
    let mut exp: u32 = 15;
    while max_abs / scale > CHART_MAX_ABS_VALUE && scale.is_finite() && exp < 306 {
        scale *= 1_000.0;
        exp += 3;
    }
    (scale, Some(format!("1e{exp}")))
}

/// Normalizes raw `.csv`, `.tsv`, or `.pdf` bytes into a validated `ChartSpecV12` with
/// largest-remainder proportional rounding (`100.00 ± 0.05`) and preserved raw value/unit labels.
pub fn normalize_chart_source(
    bytes: &[u8],
    format: ChartSourceFormat,
    options: &ChartNormalizeOptions,
) -> Result<NormalizedChartOutput, ChartNormalizerError> {
    let effective_max_series = options.max_series.clamp(1, MAX_SERIES);
    let effective_max_points = options.max_points.clamp(1, MAX_POINTS);

    let rows = match format {
        ChartSourceFormat::Csv | ChartSourceFormat::Tsv => {
            if bytes.is_empty() {
                return Err(ChartNormalizerError::EmptySource(
                    "CSV/TSV input cannot be empty".to_string(),
                ));
            }
            if bytes.len() > MAX_CSV_BYTES {
                return Err(ChartNormalizerError::InvalidTableFormat(format!(
                    "CSV/TSV byte size {} exceeds {}",
                    bytes.len(),
                    MAX_CSV_BYTES
                )));
            }
            let text = std::str::from_utf8(bytes).map_err(|_| {
                ChartNormalizerError::InvalidTableFormat(
                    "CSV/TSV source must be valid UTF-8".into(),
                )
            })?;
            let delim = if format == ChartSourceFormat::Tsv {
                '\t'
            } else {
                ','
            };
            parse_delimited_table(text, delim)
        }
        ChartSourceFormat::Pdf => extract_pdf_table_rows(bytes)?,
    };

    if rows.len() < 2 {
        return Err(ChartNormalizerError::EmptySource(
            "Table must contain a header row and at least one data row".to_string(),
        ));
    }

    let headers = &rows[0];
    if headers.len() < 2 {
        return Err(ChartNormalizerError::InvalidTableFormat(
            "Table must contain at least 2 columns (category label + numeric series)".to_string(),
        ));
    }

    let series_count = headers.len() - 1;
    if series_count > effective_max_series {
        return Err(ChartNormalizerError::SeriesLimitExceeded {
            actual: series_count,
            max: effective_max_series,
        });
    }

    let data_rows = &rows[1..];
    if data_rows.is_empty() {
        return Err(ChartNormalizerError::EmptySource(
            "Table contains header only and 0 data rows".to_string(),
        ));
    }
    if data_rows.len() > effective_max_points {
        return Err(ChartNormalizerError::PointsLimitExceeded {
            actual: data_rows.len(),
            max: effective_max_points,
        });
    }

    let is_proportional = options.chart_kind.is_proportional();
    let mut parsed_series: Vec<(String, Vec<(String, ParsedNumericCell)>)> =
        Vec::with_capacity(series_count);
    let mut detected_unit: Option<String> = None;

    for s_idx in 0..series_count {
        let col_idx = s_idx + 1;
        let series_name_raw = headers[col_idx].trim();
        let series_name: String = if series_name_raw.is_empty() {
            format!("Series {}", s_idx + 1)
        } else {
            series_name_raw
                .chars()
                .take(CHART_MAX_LABEL_CHARS)
                .collect()
        };

        let mut parsed_cells: Vec<(String, ParsedNumericCell)> =
            Vec::with_capacity(data_rows.len());
        for (r_idx, row) in data_rows.iter().enumerate() {
            let category = row
                .first()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("Row {}", r_idx + 1));
            let raw_cell = row.get(col_idx).map(String::as_str).unwrap_or("");
            let parsed = parse_localized_number_with_unit(raw_cell).map_err(|_| {
                ChartNormalizerError::NonFiniteValue {
                    row: r_idx + 2,
                    col: col_idx + 1,
                    raw: raw_cell.to_string(),
                }
            })?;
            if is_proportional && parsed.value < 0.0 {
                return Err(ChartNormalizerError::NegativeProportional {
                    row: r_idx + 2,
                    col: col_idx + 1,
                    value: parsed.value,
                });
            }
            if detected_unit.is_none() {
                detected_unit = parsed.unit.clone();
            }
            parsed_cells.push((category, parsed));
        }

        parsed_series.push((series_name, parsed_cells));
    }

    let (cartesian_scale, cartesian_scale_suffix) = if is_proportional {
        (1.0f64, None)
    } else {
        let global_max_abs = parsed_series
            .iter()
            .flat_map(|(_, cells)| cells.iter().map(|(_, p)| p.value.abs()))
            .fold(0.0f64, f64::max);
        select_cartesian_scale(global_max_abs)
    };
    if let Some(ref suffix) = cartesian_scale_suffix {
        detected_unit = Some(
            match detected_unit.take() {
                Some(existing) if !existing.is_empty() => format!("{suffix} {existing}"),
                _ => suffix.clone(),
            }
            .chars()
            .take(CHART_MAX_UNIT_CHARS)
            .collect(),
        );
    }

    let mut series_list: Vec<ChartSeries> = Vec::with_capacity(series_count);
    let mut raw_points: Vec<NormalizedPointRaw> = Vec::new();
    let mut first_proportional_sum: Option<f64> = None;

    for (s_idx, (series_name, parsed_cells)) in parsed_series.into_iter().enumerate() {
        let points: Vec<ChartDataPoint> = if is_proportional {
            let raw_vals: Vec<f64> = parsed_cells.iter().map(|(_, p)| p.value).collect();
            let raw_sum: f64 = raw_vals.iter().sum();
            if !raw_sum.is_finite() || raw_sum <= 0.0 {
                return Err(ChartNormalizerError::NonPositiveProportionalSum {
                    series_name,
                    sum: raw_sum,
                });
            }
            let pcts = largest_remainder_percentages(&raw_vals)?;
            let pct_sum: f64 = pcts.iter().sum();
            if (pct_sum - 100.0).abs() > CHART_PROPORTIONAL_SUM_TOLERANCE {
                return Err(ChartValidationError::SumNot100 {
                    series_index: s_idx,
                    sum: pct_sum,
                }
                .into());
            }
            if first_proportional_sum.is_none() {
                first_proportional_sum = Some((pct_sum * 100.0).round() / 100.0);
            }
            parsed_cells
                .iter()
                .zip(pcts.iter())
                .enumerate()
                .map(|(p_idx, ((cat, parsed), &pct))| {
                    raw_points.push(NormalizedPointRaw {
                        series_name: series_name.clone(),
                        category: cat.clone(),
                        raw_display: parsed.raw_display.clone(),
                        raw_value: parsed.value,
                        unit: parsed.unit.clone(),
                        normalized_value: pct,
                    });
                    ChartDataPoint {
                        label: format_preserved_label(cat, &parsed.raw_display, true),
                        value: pct,
                        color_token: Some(COLOR_CYCLE[p_idx % COLOR_CYCLE.len()]),
                    }
                })
                .collect()
        } else {
            parsed_cells
                .iter()
                .enumerate()
                .map(|(p_idx, (cat, parsed))| {
                    let scaled_val = ((parsed.value / cartesian_scale) * 100.0).round() / 100.0;
                    raw_points.push(NormalizedPointRaw {
                        series_name: series_name.clone(),
                        category: cat.clone(),
                        raw_display: parsed.raw_display.clone(),
                        raw_value: parsed.value,
                        unit: parsed.unit.clone(),
                        normalized_value: scaled_val,
                    });
                    ChartDataPoint {
                        label: format_preserved_label(
                            cat,
                            &parsed.raw_display,
                            cartesian_scale > 1.0,
                        ),
                        value: scaled_val,
                        color_token: Some(COLOR_CYCLE[p_idx % COLOR_CYCLE.len()]),
                    }
                })
                .collect()
        };

        series_list.push(ChartSeries {
            name: series_name,
            points,
        });
    }

    let resolved_title: String = options
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            let lead = headers.first().map(String::as_str).unwrap_or("Metrics");
            let first_series = series_list
                .first()
                .map(|s| s.name.as_str())
                .unwrap_or("Values");
            format!("{first_series} by {lead}")
        })
        .chars()
        .take(CHART_MAX_LABEL_CHARS)
        .collect();

    let spec_unit = if is_proportional {
        Some("%".to_string())
    } else {
        detected_unit.clone()
    };

    let spec = ChartSpecV12 {
        kind: options.chart_kind.to_chart_kind_2d(),
        title: resolved_title,
        unit: spec_unit,
        aria_summary: None,
        series: series_list,
    };

    let validated: ValidatedChartSpec = validate_chart_spec_v12(&spec)?;

    Ok(NormalizedChartOutput {
        spec: validated.normalized_spec,
        canonical_charts_json: validated.canonical_charts_json,
        aria_label: validated.aria_label,
        estimated_svg_primitives: validated.estimated_svg_primitives,
        requested_kind: options.chart_kind,
        source_format: format,
        proportional_sum: first_proportional_sum,
        detected_unit,
        row_count: data_rows.len(),
        series_count,
        raw_points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_us_and_eu_vn_locales_with_largest_remainder_sum_100() {
        let csv = "Channel,Revenue\nDirect,\"$1,234.56\"\nPartner,\"1.234,56 €\"\nOrganic,\"42.5M VND\"\n";
        let out = normalize_chart_source(
            csv.as_bytes(),
            ChartSourceFormat::Csv,
            &ChartNormalizeOptions {
                title: Some("Revenue Mix".into()),
                chart_kind: NormalizedChartKind::Pie,
                max_series: 12,
                max_points: 200,
            },
        )
        .expect("normalize csv");

        let sum: f64 = out.spec.series[0].points.iter().map(|p| p.value).sum();
        assert!((sum - 100.0).abs() <= 0.05, "sum={sum}");
        assert!(out.spec.series[0].points[0].label.contains("$1,234.56"));
        assert!(out.spec.series[0].points[1].label.contains("1.234,56 €"));
        assert!(out.spec.series[0].points[2].label.contains("42.5M VND"));
    }

    #[test]
    fn detects_scanned_pdf_and_returns_structured_error() {
        let scanned_pdf = b"%PDF-1.4\n1 0 obj << /Type /Page /Resources << /XObject << /Im0 2 0 R >> >> >> endobj\n2 0 obj << /Subtype /Image >> endobj\n%%EOF";
        let err = normalize_chart_source(
            scanned_pdf,
            ChartSourceFormat::Pdf,
            &ChartNormalizeOptions::default(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "SCANNED_PDF_NO_TEXT_TABLES");
        assert!(err.non_tech_guidance().is_some());
    }

    #[test]
    fn extracts_text_pdf_table_to_chart_spec() {
        let text_pdf = b"%PDF-1.4\n1 0 obj << /Type /Page >> endobj\n2 0 obj << /Length 120 >> stream\nBT\n(Segment) Tj (Share) Tj T*\n(Enterprise) Tj (120) Tj T*\n(SMB) Tj (80) Tj T*\n(SelfServe) Tj (50) Tj T*\nET\nendstream endobj\n%%EOF";
        let out = normalize_chart_source(
            text_pdf,
            ChartSourceFormat::Pdf,
            &ChartNormalizeOptions {
                title: Some("PDF Table".into()),
                chart_kind: NormalizedChartKind::Donut,
                max_series: 12,
                max_points: 200,
            },
        )
        .expect("pdf table");
        let sum: f64 = out.spec.series[0].points.iter().map(|p| p.value).sum();
        assert!((sum - 100.0).abs() <= 0.05);
        assert_eq!(out.row_count, 3);
    }

    #[test]
    fn rejects_nan_negative_proportional_and_dos_caps() {
        let nan_csv = b"Item,Val\nA,NaN\nB,10\n";
        assert_eq!(
            normalize_chart_source(
                nan_csv,
                ChartSourceFormat::Csv,
                &ChartNormalizeOptions::default()
            )
            .unwrap_err()
            .code(),
            "E_CHART_NON_FINITE"
        );

        let neg_csv = b"Item,Val\nA,-5\nB,10\n";
        assert_eq!(
            normalize_chart_source(
                neg_csv,
                ChartSourceFormat::Csv,
                &ChartNormalizeOptions {
                    chart_kind: NormalizedChartKind::Funnel,
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code(),
            "E_CHART_NEGATIVE_PROPORTIONAL"
        );

        let zero_csv = b"Item,Val\nA,0\nB,0\n";
        assert_eq!(
            normalize_chart_source(
                zero_csv,
                ChartSourceFormat::Csv,
                &ChartNormalizeOptions {
                    chart_kind: NormalizedChartKind::Pie,
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code(),
            "E_CHART_SUM_NON_POSITIVE"
        );
    }

    #[test]
    fn decompresses_flatedecode_pdf_stream_and_enforces_zip_bomb_cap() {
        // Real RFC 1950/1951 zlib-compressed `BT (Segment) Tj (Revenue) Tj T* ... ET` payload
        const FLATE_TABLE: &[u8] = &[
            120, 218, 115, 10, 225, 210, 8, 78, 77, 207, 77, 205, 43, 209, 84, 8, 201, 82, 208, 8,
            74, 45, 75, 205, 43, 77, 5, 115, 66, 180, 184, 52, 92, 243, 74, 82, 139, 10, 138, 50,
            139, 33, 66, 26, 134, 70, 6, 112, 185, 96, 95, 39, 136, 160, 5, 146, 88, 106, 78, 90,
            112, 106, 81, 25, 84, 185, 41, 92, 198, 53, 132, 11, 0, 174, 193, 29, 123,
        ];
        let mut pdf = Vec::new();
        pdf.extend_from_slice(
            b"%PDF-1.4\n1 0 obj << /Type /Page >> endobj\n2 0 obj << /Filter /FlateDecode /Length 77 >>\nstream\n",
        );
        pdf.extend_from_slice(FLATE_TABLE);
        pdf.extend_from_slice(b"\nendstream\nendobj\n%%EOF");

        let out = normalize_chart_source(
            &pdf,
            ChartSourceFormat::Pdf,
            &ChartNormalizeOptions {
                title: Some("Compressed PDF".into()),
                chart_kind: NormalizedChartKind::Pie,
                max_series: 12,
                max_points: 200,
            },
        )
        .expect("flatedecode pdf table");
        assert_eq!(out.row_count, 3);
        let sum: f64 = out.spec.series[0].points.iter().map(|p| p.value).sum();
        assert!((sum - 100.0).abs() <= 0.05);

        // 100,000 bytes of 'A' compressed via dynamic Huffman zlib (121 bytes)
        const DYNAMIC_BOMB: &[u8] = &[
            120, 218, 237, 193, 49, 1, 0, 0, 0, 194, 160, 108, 235, 95, 202, 26, 30, 64, 1, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 175, 6, 226, 12, 52, 110,
        ];
        let err = inflate_zlib_bounded(DYNAMIC_BOMB, 4096).unwrap_err();
        assert!(err.contains("DEFLATE_LIMIT_EXCEEDED"), "got {err}");
        let full = inflate_zlib_bounded(DYNAMIC_BOMB, 200_000).expect("dynamic huffman ok");
        assert_eq!(full.len(), 100_000);
        assert!(full.iter().all(|&b| b == b'A'));
    }

    #[test]
    fn scales_cartesian_values_dynamically_and_validates_locale_and_scientific_numbers() {
        let csv = "Quarter,Revenue\nQ1,\"2,500,000 VND\"\nQ2,\"1,500,000 VND\"\n";
        let out = normalize_chart_source(
            csv.as_bytes(),
            ChartSourceFormat::Csv,
            &ChartNormalizeOptions {
                title: Some("VND Revenue".into()),
                chart_kind: NormalizedChartKind::Bar,
                max_series: 12,
                max_points: 200,
            },
        )
        .expect("normalize large VND");
        assert_eq!(out.spec.unit.as_deref(), Some("M VND"));
        assert!((out.spec.series[0].points[0].value - 2.5).abs() < 1e-6);

        let huge_csv = "Year,GMV\n2025,\"5,000,000,000,000 VND\"\n2026,\"8,000,000,000,000 VND\"\n";
        let huge = normalize_chart_source(
            huge_csv.as_bytes(),
            ChartSourceFormat::Csv,
            &ChartNormalizeOptions {
                title: Some("Trillion GMV".into()),
                chart_kind: NormalizedChartKind::Line,
                max_series: 12,
                max_points: 200,
            },
        )
        .expect("normalize >1e12");
        assert_eq!(huge.spec.unit.as_deref(), Some("T VND"));
        assert!((huge.spec.series[0].points[0].value - 5.0).abs() < 1e-6);

        // Multi-sign & malformed thousands grouping rejected; scientific notation parsed cleanly
        assert!(parse_localized_number_with_unit("--5").is_err());
        assert!(parse_localized_number_with_unit("+-5").is_err());
        assert!(parse_localized_number_with_unit("1,23,45").is_err());
        let sci = parse_localized_number_with_unit("2.5e3 USD").expect("scientific notation");
        assert!((sci.value - 2500.0).abs() < 1e-6);
        assert_eq!(sci.unit.as_deref(), Some("USD"));
    }
}
