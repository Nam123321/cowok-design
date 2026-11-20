// @story 06.3
// @implements AC-01, AC-03
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CHART_MAX_SERIES: usize = 12;
pub const CHART_MAX_POINTS_PER_SERIES: usize = 200;
pub const CHART_MAX_SVG_PRIMITIVES: usize = 2_000;
pub const CHART_MAX_LABEL_CHARS: usize = 120;
pub const CHART_MAX_UNIT_CHARS: usize = 24;
pub const CHART_MAX_ARIA_SUMMARY_CHARS: usize = 240;
pub const CHART_MAX_ARIA_LABEL_CHARS: usize = 320;
pub const CHART_MAX_ABS_VALUE: f64 = 1_000_000.0;
pub const CHART_PROPORTIONAL_SUM_TOLERANCE: f64 = 0.05;
pub const CHART_MAX_JSON_BYTES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind2D {
    Pie,
    Donut,
    #[serde(rename = "100_stacked")]
    OneHundredStacked,
    Bar,
    Line,
    Area,
    Funnel,
}

impl ChartKind2D {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pie => "pie",
            Self::Donut => "donut",
            Self::OneHundredStacked => "100_stacked",
            Self::Bar => "bar",
            Self::Line => "line",
            Self::Area => "area",
            Self::Funnel => "funnel",
        }
    }

    pub fn is_proportional(&self) -> bool {
        matches!(
            self,
            Self::Pie | Self::Donut | Self::OneHundredStacked | Self::Funnel
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChartColorToken {
    HumanAccent,
    AgentAccent,
    StateSuccess,
    StateWarning,
    StateDanger,
    StateInfo,
    TextSecondary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartDataPoint {
    pub label: String,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_token: Option<ChartColorToken>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartSeries {
    pub name: String,
    pub points: Vec<ChartDataPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartSpecV12 {
    pub kind: ChartKind2D,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aria_summary: Option<String>,
    pub series: Vec<ChartSeries>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedChartSpec {
    pub normalized_spec: ChartSpecV12,
    pub canonical_charts_json: String,
    pub estimated_svg_primitives: usize,
    pub aria_label: String,
}

#[derive(Debug, Error, PartialEq)]
pub enum ChartValidationError {
    #[error(
        "E_CHART_SUM_NOT_100: Proportional series {series_index} sum {sum:.4} must be within 100.0 +/- 0.05"
    )]
    SumNot100 { series_index: usize, sum: f64 },
    #[error(
        "E_CHART_NEGATIVE_PROPORTIONAL: Negative value {value} at series[{series_index}].points[{point_index}] is forbidden on proportional/funnel charts"
    )]
    NegativeProportional {
        series_index: usize,
        point_index: usize,
        value: f64,
    },
    #[error("E_CHART_DOS_LIMIT: {0}")]
    DosLimit(String),
    #[error("VALIDATION_ERROR: {0}")]
    SchemaInvalid(String),
}

impl ChartValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SumNot100 { .. } => "E_CHART_SUM_NOT_100",
            Self::NegativeProportional { .. } => "E_CHART_NEGATIVE_PROPORTIONAL",
            Self::DosLimit(_) => "E_CHART_DOS_LIMIT",
            Self::SchemaInvalid(_) => "VALIDATION_ERROR",
        }
    }
}

/// Conservative SVG primitive count matching `ChartSpecV12Schema.superRefine`.
pub fn estimate_svg_primitives(spec: &ChartSpecV12) -> usize {
    let mut total = 8usize;
    for s in &spec.series {
        total = total
            .saturating_add(s.points.len().saturating_mul(2))
            .saturating_add(2);
    }
    total
}

fn build_chart_aria_label(spec: &ChartSpecV12) -> String {
    let base = if let Some(ref summary) = spec.aria_summary {
        let trimmed = summary.trim();
        if !trimmed.is_empty() {
            trimmed.to_string()
        } else {
            format!("{} ({})", spec.title.trim(), spec.kind.as_str())
        }
    } else {
        let first_points = spec
            .series
            .first()
            .map(|s| {
                s.points
                    .iter()
                    .take(5)
                    .map(|p| format!("{}: {}", p.label.trim(), p.value))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        if first_points.is_empty() {
            format!("{} ({})", spec.title.trim(), spec.kind.as_str())
        } else {
            format!(
                "{} ({}) — {}",
                spec.title.trim(),
                spec.kind.as_str(),
                first_points
            )
        }
    };
    base.chars().take(CHART_MAX_ARIA_LABEL_CHARS).collect()
}

fn contains_unquoted_non_finite_number(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        let rest = &raw[i..];
        if rest.starts_with("NaN") || rest.starts_with("Infinity") || rest.starts_with("-Infinity")
        {
            return true;
        }
        i += 1;
    }
    false
}

/// Parses and validates a raw `charts_json` string against `ChartSpecV12` (`#[serde(deny_unknown_fields)]`).
pub fn validate_chart_json_str(raw: &str) -> Result<ValidatedChartSpec, ChartValidationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ChartValidationError::SchemaInvalid(
            "charts_json cannot be empty".to_string(),
        ));
    }
    if trimmed.len() > CHART_MAX_JSON_BYTES {
        return Err(ChartValidationError::DosLimit(format!(
            "charts_json byte length {} exceeds {}",
            trimmed.len(),
            CHART_MAX_JSON_BYTES
        )));
    }
    if trimmed.to_ascii_lowercase().contains("<script") {
        return Err(ChartValidationError::SchemaInvalid(
            "Forbidden <script construct inside charts_json".to_string(),
        ));
    }
    if contains_unquoted_non_finite_number(trimmed) {
        return Err(ChartValidationError::DosLimit(
            "NaN or Infinity numeric literals are forbidden".to_string(),
        ));
    }

    let spec: ChartSpecV12 = serde_json::from_str(trimmed).map_err(|err| {
        let msg = err.to_string();
        if msg.contains("number out of range") || msg.contains("overflow") {
            ChartValidationError::DosLimit(format!("Numeric coordinate overflow: {msg}"))
        } else {
            ChartValidationError::SchemaInvalid(format!("Invalid ChartSpecV12 JSON: {msg}"))
        }
    })?;

    validate_chart_spec_v12(&spec)
}

/// Deterministically validates a `ChartSpecV12` struct for proportional/cartesian math and DoS caps.
pub fn validate_chart_spec_v12(
    spec: &ChartSpecV12,
) -> Result<ValidatedChartSpec, ChartValidationError> {
    let title_trimmed = spec.title.trim();
    if title_trimmed.is_empty() {
        return Err(ChartValidationError::SchemaInvalid(
            "Chart title cannot be empty".to_string(),
        ));
    }
    if spec.title.chars().count() > CHART_MAX_LABEL_CHARS {
        return Err(ChartValidationError::DosLimit(format!(
            "Chart title exceeds {CHART_MAX_LABEL_CHARS} characters"
        )));
    }
    if let Some(ref unit) = spec.unit {
        if unit.chars().count() > CHART_MAX_UNIT_CHARS {
            return Err(ChartValidationError::DosLimit(format!(
                "Chart unit exceeds {CHART_MAX_UNIT_CHARS} characters"
            )));
        }
    }
    if let Some(ref summary) = spec.aria_summary {
        if summary.chars().count() > CHART_MAX_ARIA_SUMMARY_CHARS {
            return Err(ChartValidationError::DosLimit(format!(
                "Chart ariaSummary exceeds {CHART_MAX_ARIA_SUMMARY_CHARS} characters"
            )));
        }
    }
    if spec.series.is_empty() {
        return Err(ChartValidationError::SchemaInvalid(
            "Chart must include at least 1 series".to_string(),
        ));
    }
    if spec.series.len() > CHART_MAX_SERIES {
        return Err(ChartValidationError::DosLimit(format!(
            "Series count {} exceeds {CHART_MAX_SERIES}",
            spec.series.len()
        )));
    }

    let is_prop = spec.kind.is_proportional();
    for (s_idx, series) in spec.series.iter().enumerate() {
        if series.name.trim().is_empty() {
            return Err(ChartValidationError::SchemaInvalid(format!(
                "series[{s_idx}].name cannot be empty"
            )));
        }
        if series.name.chars().count() > CHART_MAX_LABEL_CHARS {
            return Err(ChartValidationError::DosLimit(format!(
                "series[{s_idx}].name exceeds {CHART_MAX_LABEL_CHARS} characters"
            )));
        }
        if series.points.is_empty() {
            return Err(ChartValidationError::SchemaInvalid(format!(
                "series[{s_idx}].points cannot be empty"
            )));
        }
        if series.points.len() > CHART_MAX_POINTS_PER_SERIES {
            return Err(ChartValidationError::DosLimit(format!(
                "series[{s_idx}].points length {} exceeds {CHART_MAX_POINTS_PER_SERIES}",
                series.points.len()
            )));
        }
        let mut sum = 0.0f64;
        for (p_idx, pt) in series.points.iter().enumerate() {
            if pt.label.trim().is_empty() {
                return Err(ChartValidationError::SchemaInvalid(format!(
                    "series[{s_idx}].points[{p_idx}].label cannot be empty"
                )));
            }
            if pt.label.chars().count() > CHART_MAX_LABEL_CHARS {
                return Err(ChartValidationError::DosLimit(format!(
                    "series[{s_idx}].points[{p_idx}].label exceeds {CHART_MAX_LABEL_CHARS} characters"
                )));
            }
            if !pt.value.is_finite() || pt.value.abs() > CHART_MAX_ABS_VALUE {
                return Err(ChartValidationError::DosLimit(format!(
                    "series[{s_idx}].points[{p_idx}].value ({}) violates finite [-1e6, 1e6] cap",
                    pt.value
                )));
            }
            if is_prop && pt.value < 0.0 {
                return Err(ChartValidationError::NegativeProportional {
                    series_index: s_idx,
                    point_index: p_idx,
                    value: pt.value,
                });
            }
            sum += pt.value;
        }
        if is_prop && (sum - 100.0).abs() > CHART_PROPORTIONAL_SUM_TOLERANCE {
            return Err(ChartValidationError::SumNot100 {
                series_index: s_idx,
                sum,
            });
        }
    }

    let estimated_svg_primitives = estimate_svg_primitives(spec);
    if estimated_svg_primitives > CHART_MAX_SVG_PRIMITIVES {
        return Err(ChartValidationError::DosLimit(format!(
            "Estimated SVG primitives ({estimated_svg_primitives}) exceeds {CHART_MAX_SVG_PRIMITIVES}"
        )));
    }

    let canonical_charts_json = serde_json::to_string(spec)
        .map_err(|e| ChartValidationError::SchemaInvalid(format!("Serialize failed: {e}")))?;
    if canonical_charts_json.len() > CHART_MAX_JSON_BYTES {
        return Err(ChartValidationError::DosLimit(format!(
            "Serialized charts_json byte length {} exceeds {CHART_MAX_JSON_BYTES}",
            canonical_charts_json.len()
        )));
    }
    if canonical_charts_json
        .to_ascii_lowercase()
        .contains("<script")
    {
        return Err(ChartValidationError::SchemaInvalid(
            "Forbidden <script construct inside charts_json".to_string(),
        ));
    }

    Ok(ValidatedChartSpec {
        normalized_spec: spec.clone(),
        canonical_charts_json,
        estimated_svg_primitives,
        aria_label: build_chart_aria_label(spec),
    })
}
