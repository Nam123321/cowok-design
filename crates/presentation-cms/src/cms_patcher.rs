// @story 06.2
// @implements AC-02, AC-04, AC-05
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use html5ever::tendril::TendrilSink;
use html5ever::{parse_document, ParseOpts};
use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, RewriteStrSettings};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::charts_2d::validate_chart_json_str;
use crate::protocol::MAX_SLIDE_HTML_BYTES;
use crate::templates::compute_sha256_hex;
use crate::ulid::is_valid_slide_uid;

pub const MAX_SLOT_TEXT_CHARS: usize = 8_000;
pub const MAX_FF_ID_LEN: usize = 64;
pub const MAX_CHARTS_JSON_BYTES: usize = 65_536;
pub const CUSTOM_OVERRIDE_ATTR: &str = "data-cowok-custom-override";

const ALLOWED_INLINE_CSS_PROPS: &[&str] = &[
    "color",
    "background-color",
    "font-size",
    "font-weight",
    "font-family",
    "line-height",
    "letter-spacing",
    "text-align",
    "opacity",
    "border-color",
    "border-radius",
    "padding",
    "margin",
    "gap",
    "display",
];

const ALLOWED_DISPLAY_VALUES: &[&str] =
    &["none", "block", "inline", "inline-block", "flex", "grid"];

const ALLOWED_TEXT_ALIGN_VALUES: &[&str] = &["left", "center", "right", "justify", "start", "end"];

const ALLOWED_FONT_WEIGHT_VALUES: &[&str] = &[
    "normal", "bold", "lighter", "bolder", "100", "200", "300", "400", "500", "600", "700", "800",
    "900",
];

const FORBIDDEN_ELEMENTS: &[&str] = &[
    "script",
    "foreignobject",
    "iframe",
    "object",
    "embed",
    "meta",
    "base",
    "noscript",
    "template",
    "form",
    "input",
    "textarea",
    "select",
    "applet",
    "link",
    "math",
];

const ALLOWED_HTML_ELEMENTS: &[&str] = &[
    "html",
    "head",
    "body",
    "title",
    "style",
    "section",
    "article",
    "header",
    "footer",
    "main",
    "aside",
    "nav",
    "div",
    "span",
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "strong",
    "em",
    "b",
    "i",
    "u",
    "small",
    "code",
    "pre",
    "blockquote",
    "figure",
    "figcaption",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "th",
    "td",
    "img",
    "a",
    "br",
    "hr",
    "time",
    "mark",
];

const ALLOWED_SVG_ELEMENTS: &[&str] = &[
    "svg",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "g",
    "defs",
    "lineargradient",
    "radialgradient",
    "stop",
    "desc",
];

const ALLOWED_VOID_ELEMENTS: &[&str] = &["br", "hr", "img"];

const ALLOWED_SVG_ATTRS: &[&str] = &[
    "viewbox",
    "width",
    "height",
    "fill",
    "stroke",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-dasharray",
    "opacity",
    "fill-opacity",
    "stroke-opacity",
    "d",
    "x",
    "y",
    "x1",
    "y1",
    "x2",
    "y2",
    "cx",
    "cy",
    "r",
    "rx",
    "ry",
    "points",
    "transform",
    "offset",
    "stop-color",
    "stop-opacity",
    "preserveaspectratio",
    "xmlns",
];

const ALLOWED_COMMON_ATTRS: &[&str] = &[
    "id", "class", "lang", "dir", "role", "style", "alt", "title", "src", "href", "width",
    "height", "colspan", "rowspan", "scope",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CmsSlotKind {
    Text,
    Heading,
    Metric,
    Bullet,
    Image,
    Color,
    ChartJson,
}

impl CmsSlotKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Heading => "heading",
            Self::Metric => "metric",
            Self::Bullet => "bullet",
            Self::Image => "image",
            Self::Color => "color",
            Self::ChartJson => "chart_json",
        }
    }

    pub fn from_str_checked(raw: &str) -> Result<Self, CmsPatcherError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "heading" => Ok(Self::Heading),
            "metric" => Ok(Self::Metric),
            "bullet" => Ok(Self::Bullet),
            "image" => Ok(Self::Image),
            "color" => Ok(Self::Color),
            "chart_json" => Ok(Self::ChartJson),
            other => Err(CmsPatcherError::InvalidSlotValue(format!(
                "Unsupported slot_kind '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotPatchInput {
    pub ff_id: String,
    pub slot_kind: CmsSlotKind,
    pub text_value: String,
    #[serde(default)]
    pub bullet_items: Option<Vec<String>>,
    #[serde(default)]
    pub charts_json: Option<String>,
    #[serde(default)]
    pub safe_styles_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotPatchOutput {
    pub patched_html: String,
    pub patched_slots_count: usize,
    pub content_sha256: String,
    pub execution_time_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedSlot {
    pub ff_id: String,
    pub slot_label: String,
    pub slot_kind: CmsSlotKind,
    pub text_value: String,
    pub charts_json: String,
    pub safe_styles_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepSanitizeOutput {
    pub sanitized_html: String,
    pub content_sha256: String,
    pub extracted_slots: Vec<ExtractedSlot>,
    pub converged_slots_count: usize,
    pub custom_html_override: bool,
    pub unmapped_slot_ids: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CmsPatcherError {
    #[error("E_PAYLOAD_TOO_LARGE: Slide HTML byte length {actual} exceeds {max}")]
    PayloadTooLarge { actual: usize, max: usize },
    #[error("E_PAYLOAD_TOO_LARGE: Slot text length {actual} exceeds {max} characters")]
    SlotTextTooLong { actual: usize, max: usize },
    #[error("VALIDATION_ERROR: Invalid data-ff-id '{0}'")]
    InvalidFfId(String),
    #[error("VALIDATION_ERROR: Invalid slide_uid '{0}'")]
    InvalidSlideUid(String),
    #[error("E_MXSS_BLOCKED: Blocked dangerous HTML construct '{0}'")]
    MxssBlocked(String),
    #[error("E_MXSS_BLOCKED: Blocked unsafe CSS token '{0}'")]
    CssTokenBlocked(String),
    #[error("VALIDATION_ERROR: Invalid slot payload: {0}")]
    InvalidSlotValue(String),
    #[error("{code}: {message}")]
    ChartValidationFailed { code: &'static str, message: String },
    #[error("INTERNAL_ERROR: HTML rewriter failed: {0}")]
    RewriterFailure(String),
}

/// Validates canonical `[data-ff-id]` syntax (`^[a-zA-Z0-9_-]{1,64}$`) to block CSS selector injection.
pub fn is_valid_ff_id(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_FF_ID_LEN {
        return false;
    }
    value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Strict fail-closed URL allowlist for `img[src]` and `a[href]`.
pub fn is_safe_asset_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 2048 {
        return false;
    }
    if trimmed.bytes().any(|b| b < 0x20 || b == 0x7F) {
        return false;
    }
    let compact_lower: String = trimmed
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if compact_lower.starts_with("javascript:")
        || compact_lower.starts_with("vbscript:")
        || compact_lower.starts_with("data:")
        || compact_lower.starts_with("file:")
        || compact_lower.starts_with("//")
        || compact_lower.contains("..")
    {
        return false;
    }
    compact_lower.starts_with("https://")
        || compact_lower.starts_with("/assets/")
        || compact_lower.starts_with("assets/")
        || compact_lower.starts_with("presentations/")
        || compact_lower.starts_with("slides/")
        || compact_lower.starts_with('#')
}

fn strip_css_comments_fail_closed(input: &str) -> Result<String, CmsPatcherError> {
    if input.contains('\0') {
        return Err(CmsPatcherError::CssTokenBlocked("null-byte".to_string()));
    }
    if input.contains('\\') {
        return Err(CmsPatcherError::CssTokenBlocked(
            "backslash-escape".to_string(),
        ));
    }
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            let mut closed = false;
            while i + 1 < bytes.len() {
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                return Err(CmsPatcherError::CssTokenBlocked(
                    "unclosed-comment".to_string(),
                ));
            }
            out.push(' ');
        } else {
            let ch = input[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    Ok(out)
}

fn scan_forbidden_css_tokens(stripped: &str) -> Result<(), CmsPatcherError> {
    let lower = stripped.to_ascii_lowercase();
    let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();
    if lower.contains('<') || compact.contains("</style") {
        return Err(CmsPatcherError::CssTokenBlocked("</style".to_string()));
    }
    for needle in [
        "@import",
        "@charset",
        "@namespace",
        "@document",
        "@-moz-",
        "expression(",
        "-moz-binding",
        "behavior:",
        "unsafe-inline",
        "url(",
        "image-set(",
        "cross-fade(",
        "image(",
        "src(",
        "javascript:",
        "vbscript:",
        "data:",
    ] {
        if lower.contains(needle) || compact.contains(needle) {
            return Err(CmsPatcherError::CssTokenBlocked(needle.to_string()));
        }
    }
    Ok(())
}

fn selector_targets_protected_root(selector: &str) -> bool {
    let lower = selector.to_ascii_lowercase();
    if lower.contains("live-html-frame")
        || lower.contains(":root")
        || lower.contains('*')
        || lower.contains("[class")
    {
        return true;
    }
    for token in lower.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
        if matches!(token, "html" | "body") {
            return true;
        }
    }
    false
}

fn is_valid_hex_color(v: &str) -> bool {
    let Some(hex) = v.strip_prefix('#') else {
        return false;
    };
    matches!(hex.len(), 3 | 6 | 8) && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_valid_rgb_color(v: &str) -> bool {
    let lower = v.to_ascii_lowercase();
    let Some(inner) = lower.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) else {
        return false;
    };
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.parse::<u8>().is_ok())
}

fn is_valid_css_var(v: &str) -> bool {
    let Some(inner) = v.strip_prefix("var(--").and_then(|s| s.strip_suffix(')')) else {
        return false;
    };
    !inner.is_empty()
        && inner.len() <= 64
        && inner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_valid_css_length_token(tok: &str) -> bool {
    if tok == "0" || tok == "auto" {
        return true;
    }
    let mut num_end = tok.len();
    for suffix in ["px", "rem", "em", "%", "fr"] {
        if let Some(prefix) = tok.strip_suffix(suffix) {
            num_end = prefix.len();
            break;
        }
    }
    if num_end == tok.len() {
        return false;
    }
    let num = &tok[..num_end];
    if num.is_empty() {
        return false;
    }
    num.parse::<f64>().is_ok()
}

fn is_valid_css_value(prop: &str, raw_val: &str) -> bool {
    let val = raw_val.trim();
    if val.is_empty() || val.len() > 256 {
        return false;
    }
    if val.to_ascii_lowercase().contains("!important") {
        return false;
    }
    if is_valid_css_var(val) {
        return true;
    }
    match prop {
        "color" | "background-color" | "border-color" => {
            val == "transparent"
                || val == "currentColor"
                || val == "inherit"
                || is_valid_hex_color(val)
                || is_valid_rgb_color(val)
        }
        "font-size" | "letter-spacing" | "border-radius" | "gap" => {
            val.split_whitespace().count() <= 4
                && val
                    .split_whitespace()
                    .all(|t| is_valid_css_var(t) || is_valid_css_length_token(t))
        }
        "padding" | "margin" => {
            val.split_whitespace().count() <= 4
                && val
                    .split_whitespace()
                    .all(|t| is_valid_css_var(t) || is_valid_css_length_token(t))
        }
        "line-height" => {
            val == "normal"
                || is_valid_css_length_token(val)
                || val.parse::<f64>().is_ok_and(|n| (0.5..=4.0).contains(&n))
        }
        "opacity" => val.parse::<f64>().is_ok_and(|n| (0.0..=1.0).contains(&n)),
        "font-weight" => ALLOWED_FONT_WEIGHT_VALUES.contains(&val.to_ascii_lowercase().as_str()),
        "text-align" => ALLOWED_TEXT_ALIGN_VALUES.contains(&val.to_ascii_lowercase().as_str()),
        "display" => ALLOWED_DISPLAY_VALUES.contains(&val.to_ascii_lowercase().as_str()),
        "font-family" => {
            val.len() <= 128
                && val.bytes().all(|b| {
                    b.is_ascii_alphanumeric()
                        || matches!(b, b' ' | b',' | b'-' | b'_' | b'"' | b'\'')
                })
        }
        _ => false,
    }
}

/// Fail-closed deterministic CSS sanitizer for both `style=""` inline attributes and `<style>` blocks.
pub fn validate_and_scrub_css_tokens(
    css: &str,
    is_inline_style: bool,
) -> Result<String, CmsPatcherError> {
    let stripped = strip_css_comments_fail_closed(css)?;
    scan_forbidden_css_tokens(&stripped)?;

    if is_inline_style {
        let mut normalized = Vec::new();
        for decl in stripped.split(';') {
            let trimmed = decl.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some((raw_prop, raw_val)) = trimmed.split_once(':') else {
                return Err(CmsPatcherError::CssTokenBlocked(format!(
                    "malformed-decl:{trimmed}"
                )));
            };
            let prop = raw_prop.trim().to_ascii_lowercase();
            let val = raw_val.trim();
            if !ALLOWED_INLINE_CSS_PROPS.contains(&prop.as_str()) {
                return Err(CmsPatcherError::CssTokenBlocked(format!(
                    "disallowed-prop:{prop}"
                )));
            }
            if !is_valid_css_value(&prop, val) {
                return Err(CmsPatcherError::CssTokenBlocked(format!(
                    "invalid-value:{prop}:{val}"
                )));
            }
            normalized.push(format!("{prop}: {val}"));
        }
        return Ok(normalized.join("; "));
    }

    // Stylesheet `<style>` block validation: string-aware brace scanner & protected-root !important guard
    let mut selector_stack: Vec<String> = Vec::new();
    let mut body_stack: Vec<String> = Vec::new();
    let mut pending_prelude = String::new();
    let mut quote_char: Option<char> = None;
    for ch in stripped.chars() {
        if let Some(q) = quote_char {
            pending_prelude.push(ch);
            if ch == q {
                quote_char = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote_char = Some(ch);
                pending_prelude.push(ch);
            }
            '{' => {
                if selector_stack.len() >= 2 {
                    return Err(CmsPatcherError::CssTokenBlocked(
                        "excessive-css-nesting".to_string(),
                    ));
                }
                selector_stack.push(std::mem::take(&mut pending_prelude));
                body_stack.push(String::new());
            }
            '}' => {
                let Some(sel) = selector_stack.pop() else {
                    return Err(CmsPatcherError::CssTokenBlocked(
                        "unbalanced-braces".to_string(),
                    ));
                };
                let block = body_stack.pop().unwrap_or_default();
                let combined_block = format!("{block}{pending_prelude}");
                pending_prelude.clear();
                if selector_targets_protected_root(&sel)
                    && combined_block.to_ascii_lowercase().contains("!important")
                {
                    return Err(CmsPatcherError::CssTokenBlocked(
                        "protected-selector-important".to_string(),
                    ));
                }
            }
            ';' => {
                if let Some(cur_body) = body_stack.last_mut() {
                    cur_body.push_str(&pending_prelude);
                    cur_body.push(';');
                }
                pending_prelude.clear();
            }
            _ => {
                pending_prelude.push(ch);
            }
        }
    }
    if quote_char.is_some() || !selector_stack.is_empty() {
        return Err(CmsPatcherError::CssTokenBlocked(
            "unbalanced-braces".to_string(),
        ));
    }
    Ok(stripped)
}

fn parse_safe_styles_map(safe_styles_json: &str) -> Result<String, CmsPatcherError> {
    let trimmed = safe_styles_json.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return Ok(String::new());
    }
    let map: BTreeMap<String, String> = serde_json::from_str(trimmed)
        .map_err(|e| CmsPatcherError::InvalidSlotValue(format!("Invalid safeStylesJson: {e}")))?;
    let mut decls = Vec::with_capacity(map.len());
    for (k, v) in map {
        decls.push(format!("{k}: {v}"));
    }
    validate_and_scrub_css_tokens(&decls.join("; "), true)
}

fn escape_html_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_html_attr(input: &str) -> String {
    escape_html_text(input)
}

/// High-speed (`< 1ms`) streaming HTML5 slot patcher powered by `lol_html`.
/// Scopes selectors strictly inside `section[data-slide-uid="..."]` when `scope_slide_uid` is provided (`one-page` decks).
pub fn patch_slide_slots_lol_html(
    html: &str,
    scope_slide_uid: Option<&str>,
    patches: &[SlotPatchInput],
) -> Result<SlotPatchOutput, CmsPatcherError> {
    let started = Instant::now();
    if html.len() > MAX_SLIDE_HTML_BYTES {
        return Err(CmsPatcherError::PayloadTooLarge {
            actual: html.len(),
            max: MAX_SLIDE_HTML_BYTES,
        });
    }
    if let Some(uid) = scope_slide_uid {
        if !is_valid_slide_uid(uid) {
            return Err(CmsPatcherError::InvalidSlideUid(uid.to_string()));
        }
    }

    struct PreparedPatch {
        ff_id: String,
        kind: CmsSlotKind,
        text_value: String,
        bullet_html: Option<String>,
        image_src: Option<String>,
        charts_json: Option<String>,
        sanitized_style: Option<String>,
    }

    let mut prepared = Vec::with_capacity(patches.len());
    for patch in patches {
        if !is_valid_ff_id(&patch.ff_id) {
            return Err(CmsPatcherError::InvalidFfId(patch.ff_id.clone()));
        }
        let char_count = patch.text_value.chars().count();
        if char_count > MAX_SLOT_TEXT_CHARS {
            return Err(CmsPatcherError::SlotTextTooLong {
                actual: char_count,
                max: MAX_SLOT_TEXT_CHARS,
            });
        }

        let mut sanitized_style = match patch.safe_styles_json.as_deref() {
            Some(raw) if !raw.trim().is_empty() && raw.trim() != "{}" => {
                Some(parse_safe_styles_map(raw)?)
            }
            _ => None,
        };

        let bullet_html = if patch.slot_kind == CmsSlotKind::Bullet {
            let items: Vec<String> = match &patch.bullet_items {
                Some(list) => list.clone(),
                None => patch
                    .text_value
                    .lines()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string)
                    .collect(),
            };
            if items.len() > 24 {
                return Err(CmsPatcherError::InvalidSlotValue(
                    "bulletItems exceeds max 24".to_string(),
                ));
            }
            for item in &items {
                if item.chars().count() > 512 {
                    return Err(CmsPatcherError::SlotTextTooLong {
                        actual: item.chars().count(),
                        max: 512,
                    });
                }
            }
            let rendered = items
                .iter()
                .map(|item| format!("<li>{}</li>", escape_html_text(item)))
                .collect::<String>();
            Some(rendered)
        } else {
            None
        };

        let image_src = if patch.slot_kind == CmsSlotKind::Image {
            let candidate = patch.text_value.trim();
            if !is_safe_asset_url(candidate) {
                return Err(CmsPatcherError::MxssBlocked(format!(
                    "unsafe-image-url:{candidate}"
                )));
            }
            Some(candidate.to_string())
        } else {
            None
        };

        if patch.slot_kind == CmsSlotKind::Color && sanitized_style.is_none() {
            let color_val = patch.text_value.trim();
            let checked = validate_and_scrub_css_tokens(&format!("color: {color_val}"), true)?;
            sanitized_style = Some(checked);
        }

        let charts_json = if let Some(raw_chart) = patch.charts_json.as_deref() {
            let trimmed = raw_chart.trim();
            if !trimmed.is_empty() {
                if trimmed.len() > 65_536 {
                    return Err(CmsPatcherError::ChartValidationFailed {
                        code: "E_CHART_DOS_LIMIT",
                        message: "chartsJson exceeds 65536 bytes".to_string(),
                    });
                }
                if trimmed != "[]" {
                    let validated = validate_chart_json_str(trimmed).map_err(|err| {
                        CmsPatcherError::ChartValidationFailed {
                            code: err.code(),
                            message: err.to_string(),
                        }
                    })?;
                    Some(validated.canonical_charts_json)
                } else {
                    Some("[]".to_string())
                }
            } else {
                None
            }
        } else {
            None
        };

        prepared.push(PreparedPatch {
            ff_id: patch.ff_id.clone(),
            kind: patch.slot_kind,
            text_value: patch.text_value.clone(),
            bullet_html,
            image_src,
            charts_json,
            sanitized_style,
        });
    }

    if prepared.is_empty() {
        let sha = compute_sha256_hex(html.as_bytes());
        return Ok(SlotPatchOutput {
            patched_html: html.to_string(),
            patched_slots_count: 0,
            content_sha256: sha,
            execution_time_micros: started.elapsed().as_micros() as u64,
        });
    }

    let mut patch_map: HashMap<String, PreparedPatch> = HashMap::with_capacity(prepared.len());
    for p in prepared {
        if patch_map.contains_key(&p.ff_id) {
            return Err(CmsPatcherError::InvalidSlotValue(format!(
                "Duplicate ffId '{}' in patch batch",
                p.ff_id
            )));
        }
        patch_map.insert(p.ff_id.clone(), p);
    }

    let selector = match scope_slide_uid {
        Some(uid) => format!(r#"section[data-slide-uid="{uid}"] [data-ff-id]"#),
        None => "[data-ff-id]".to_string(),
    };

    let matched_ids: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
    let matched_ref = Rc::clone(&matched_ids);

    let patched_html = rewrite_str(
        html,
        RewriteStrSettings::new().append_element_content_handler(element!(
            selector.as_str(),
            move |el| {
                if let Some(ff_id) = el.get_attribute("data-ff-id") {
                    if let Some(patch) = patch_map.get(&ff_id) {
                        matched_ref.borrow_mut().insert(ff_id.clone());
                        if let Some(ref style_css) = patch.sanitized_style {
                            if !style_css.is_empty() {
                                let _ = el.set_attribute("style", style_css);
                            }
                        }
                        if let Some(ref chart_val) = patch.charts_json {
                            let _ = el.set_attribute("data-charts-json", chart_val);
                        }
                        match patch.kind {
                            CmsSlotKind::Bullet => {
                                if let Some(ref list_html) = patch.bullet_html {
                                    el.set_inner_content(list_html, ContentType::Html);
                                }
                            }
                            CmsSlotKind::Image => {
                                if let Some(ref src) = patch.image_src {
                                    let _ = el.set_attribute("src", src);
                                }
                            }
                            CmsSlotKind::Color => {
                                if !patch.text_value.trim().starts_with('#')
                                    && !patch.text_value.trim().starts_with("var(")
                                    && !patch.text_value.trim().starts_with("rgb(")
                                    && !patch.text_value.trim().is_empty()
                                {
                                    el.set_inner_content(&patch.text_value, ContentType::Text);
                                }
                            }
                            CmsSlotKind::Text
                            | CmsSlotKind::Heading
                            | CmsSlotKind::Metric
                            | CmsSlotKind::ChartJson => {
                                if !patch.text_value.is_empty()
                                    || patch.kind != CmsSlotKind::ChartJson
                                {
                                    el.set_inner_content(&patch.text_value, ContentType::Text);
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
        )),
    )
    .map_err(|e| CmsPatcherError::RewriterFailure(e.to_string()))?;

    if patched_html.len() > MAX_SLIDE_HTML_BYTES {
        return Err(CmsPatcherError::PayloadTooLarge {
            actual: patched_html.len(),
            max: MAX_SLIDE_HTML_BYTES,
        });
    }

    let content_sha256 = compute_sha256_hex(patched_html.as_bytes());
    let count = matched_ids.borrow().len();
    Ok(SlotPatchOutput {
        patched_html,
        patched_slots_count: count,
        content_sha256,
        execution_time_micros: started.elapsed().as_micros() as u64,
    })
}

fn collect_text_content(handle: &Handle, out: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => {
            out.push_str(&contents.borrow());
        }
        NodeData::Element { .. } | NodeData::Document => {
            for child in handle.children.borrow().iter() {
                collect_text_content(child, out);
            }
        }
        _ => {}
    }
}

fn infer_slot_kind(tag: &str, attrs: &[(String, String)]) -> CmsSlotKind {
    for (k, v) in attrs {
        if k == "data-slot-kind" {
            if let Ok(parsed) = CmsSlotKind::from_str_checked(v) {
                return parsed;
            }
        }
        if k == "data-charts-json" {
            return CmsSlotKind::ChartJson;
        }
    }
    match tag {
        "h1" | "h2" | "h3" | "h4" => CmsSlotKind::Heading,
        "ul" | "ol" => CmsSlotKind::Bullet,
        "img" => CmsSlotKind::Image,
        _ => CmsSlotKind::Text,
    }
}

fn style_string_to_json(style_css: &str) -> String {
    let mut map = BTreeMap::new();
    for decl in style_css.split(';') {
        if let Some((k, v)) = decl.split_once(':') {
            let prop = k.trim().to_ascii_lowercase();
            let val = v.trim().to_string();
            if !prop.is_empty() && !val.is_empty() {
                map.insert(prop, val);
            }
        }
    }
    serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
}

fn is_allowed_data_or_aria_attr(attr_name: &str) -> bool {
    (attr_name.starts_with("data-") || attr_name.starts_with("aria-"))
        && attr_name.len() <= 64
        && attr_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[allow(clippy::too_many_arguments)]
fn sanitize_and_serialize_node(
    handle: &Handle,
    out: &mut String,
    extracted: &mut Vec<ExtractedSlot>,
    seen_ff_ids: &mut HashSet<(String, String)>,
    in_svg: bool,
    override_stamp: Option<bool>,
    scope_slide_uid: Option<&str>,
    active_slide_scope: Option<&str>,
    stamped_root: &mut bool,
) -> Result<(), CmsPatcherError> {
    match &handle.data {
        NodeData::Document => {
            for child in handle.children.borrow().iter() {
                sanitize_and_serialize_node(
                    child,
                    out,
                    extracted,
                    seen_ff_ids,
                    in_svg,
                    override_stamp,
                    scope_slide_uid,
                    active_slide_scope,
                    stamped_root,
                )?;
            }
        }
        NodeData::Doctype { .. } => {
            out.push_str("<!DOCTYPE html>");
        }
        NodeData::Text { contents } => {
            out.push_str(&escape_html_text(&contents.borrow()));
        }
        NodeData::Comment { .. } => {
            // Strip HTML comments to prevent comment-based mXSS re-parsing differentials
        }
        NodeData::Element { name, attrs, .. } => {
            let tag: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
            if FORBIDDEN_ELEMENTS.contains(&tag.as_str()) {
                return Err(CmsPatcherError::MxssBlocked(format!(
                    "forbidden-element:<{tag}>"
                )));
            }
            let is_svg_tag = ALLOWED_SVG_ELEMENTS.contains(&tag.as_str());
            let is_html_tag = ALLOWED_HTML_ELEMENTS.contains(&tag.as_str());
            if !is_svg_tag && !is_html_tag {
                return Err(CmsPatcherError::MxssBlocked(format!(
                    "disallowed-element:<{tag}>"
                )));
            }
            if in_svg && !is_svg_tag {
                return Err(CmsPatcherError::MxssBlocked(format!(
                    "svg-namespace-escape:<{tag}>"
                )));
            }
            if !in_svg && is_svg_tag && !is_html_tag && tag != "svg" {
                return Err(CmsPatcherError::MxssBlocked(format!(
                    "svg-child-outside-svg:<{tag}>"
                )));
            }

            let next_in_svg = in_svg || tag == "svg";

            // Special handling for `<style>`: scrub content strictly and emit raw scrubbed CSS
            if tag == "style" {
                let mut raw_css = String::new();
                collect_text_content(handle, &mut raw_css);
                let safe_css = validate_and_scrub_css_tokens(&raw_css, false)?;
                out.push_str("<style>");
                out.push_str(&safe_css);
                out.push_str("</style>");
                return Ok(());
            }

            let mut safe_attrs: Vec<(String, String)> = Vec::new();
            let mut is_slide_root = false;
            let mut matched_scope_slide_root = false;
            let mut own_slide_uid: Option<String> = None;
            let mut ff_id_opt: Option<String> = None;
            let mut slot_label_opt: Option<String> = None;
            let mut charts_json_val = "[]".to_string();
            let mut safe_styles_json_val = "{}".to_string();

            if tag == "section" {
                for attr in attrs.borrow().iter() {
                    let k: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
                    if k == "data-slide-uid" {
                        let trimmed = attr.value.trim();
                        if !trimmed.is_empty() {
                            own_slide_uid = Some(trimmed.to_string());
                        }
                    }
                }
            }
            let next_slide_scope: Option<&str> = own_slide_uid.as_deref().or(active_slide_scope);

            for attr in attrs.borrow().iter() {
                let attr_name: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
                let attr_val = attr.value.to_string();

                if attr_name.starts_with("on") {
                    return Err(CmsPatcherError::MxssBlocked(format!(
                        "event-handler-attr:{attr_name}"
                    )));
                }
                if attr_name == "srcdoc" || attr_name == "xlink:href" {
                    return Err(CmsPatcherError::MxssBlocked(format!(
                        "forbidden-attr:{attr_name}"
                    )));
                }
                if next_in_svg && attr_name == "href" {
                    return Err(CmsPatcherError::MxssBlocked("svg-href-blocked".to_string()));
                }

                let allowed = ALLOWED_COMMON_ATTRS.contains(&attr_name.as_str())
                    || (next_in_svg && ALLOWED_SVG_ATTRS.contains(&attr_name.as_str()))
                    || is_allowed_data_or_aria_attr(&attr_name);

                if !allowed {
                    return Err(CmsPatcherError::MxssBlocked(format!(
                        "disallowed-attr:{attr_name}"
                    )));
                }

                if attr_name == CUSTOM_OVERRIDE_ATTR {
                    // Managed strictly by sanitizer convergence stamp
                    continue;
                }

                if (attr_name == "src" || attr_name == "href") && !is_safe_asset_url(&attr_val) {
                    return Err(CmsPatcherError::MxssBlocked(format!(
                        "unsafe-url:{attr_name}:{attr_val}"
                    )));
                }

                if attr_name == "style" {
                    let scrubbed = validate_and_scrub_css_tokens(&attr_val, true)?;
                    safe_styles_json_val = style_string_to_json(&scrubbed);
                    if !scrubbed.is_empty() {
                        safe_attrs.push((attr_name, scrubbed));
                    }
                    continue;
                }

                if attr_name == "data-ff-id" {
                    if !is_valid_ff_id(&attr_val) {
                        return Err(CmsPatcherError::InvalidFfId(attr_val));
                    }
                    let scope_key = next_slide_scope.unwrap_or("").to_string();
                    if !seen_ff_ids.insert((scope_key, attr_val.clone())) {
                        return Err(CmsPatcherError::InvalidSlotValue(format!(
                            "Duplicate data-ff-id '{attr_val}'"
                        )));
                    }
                    ff_id_opt = Some(attr_val.clone());
                } else if attr_name == "data-slot-label" {
                    slot_label_opt = Some(attr_val.clone());
                } else if attr_name == "data-charts-json" {
                    if attr_val.len() > MAX_CHARTS_JSON_BYTES
                        || validate_chart_json_str(&attr_val).is_err()
                    {
                        return Err(CmsPatcherError::InvalidSlotValue(
                            "Invalid or oversized data-charts-json attribute".to_string(),
                        ));
                    }
                    charts_json_val = attr_val.clone();
                } else if attr_name == "class" && attr_val.split_whitespace().any(|c| c == "slide")
                {
                    is_slide_root = true;
                } else if attr_name == "data-slide-uid" && tag == "section" {
                    is_slide_root = true;
                    if scope_slide_uid.is_some_and(|u| attr_val.trim() == u) {
                        matched_scope_slide_root = true;
                    }
                }

                safe_attrs.push((attr_name, attr_val));
            }

            if let Some(should_stamp) = override_stamp {
                let can_stamp_here = if scope_slide_uid.is_some() {
                    matched_scope_slide_root
                } else {
                    is_slide_root || tag == "section" || tag == "body"
                };
                if should_stamp && !*stamped_root && can_stamp_here {
                    safe_attrs.push((CUSTOM_OVERRIDE_ATTR.to_string(), "1".to_string()));
                    *stamped_root = true;
                }
            }

            if let Some(ff_id) = ff_id_opt {
                if scope_slide_uid.is_none() || scope_slide_uid == next_slide_scope {
                    let kind = infer_slot_kind(&tag, &safe_attrs);
                    let text_val = if kind == CmsSlotKind::Image {
                        safe_attrs
                            .iter()
                            .find(|(k, _)| k == "src")
                            .map(|(_, v)| v.clone())
                            .unwrap_or_default()
                    } else {
                        let mut txt = String::new();
                        collect_text_content(handle, &mut txt);
                        txt.trim().to_string()
                    };
                    let label = slot_label_opt.unwrap_or_else(|| ff_id.clone());
                    extracted.push(ExtractedSlot {
                        ff_id,
                        slot_label: label,
                        slot_kind: kind,
                        text_value: text_val,
                        charts_json: charts_json_val,
                        safe_styles_json: safe_styles_json_val,
                    });
                }
            }

            out.push('<');
            out.push_str(&tag);
            for (k, v) in &safe_attrs {
                out.push(' ');
                out.push_str(k);
                out.push_str("=\"");
                out.push_str(&escape_html_attr(v));
                out.push('"');
            }
            out.push('>');

            if !ALLOWED_VOID_ELEMENTS.contains(&tag.as_str()) {
                for child in handle.children.borrow().iter() {
                    sanitize_and_serialize_node(
                        child,
                        out,
                        extracted,
                        seen_ff_ids,
                        next_in_svg,
                        override_stamp,
                        scope_slide_uid,
                        next_slide_scope,
                        stamped_root,
                    )?;
                }
                out.push_str("</");
                out.push_str(&tag);
                out.push('>');
            }
        }
        NodeData::ProcessingInstruction { .. } => {
            return Err(CmsPatcherError::MxssBlocked(
                "processing-instruction".to_string(),
            ));
        }
    }
    Ok(())
}

fn audit_dom_no_forbidden(handle: &Handle, in_svg: bool) -> Result<(), CmsPatcherError> {
    match &handle.data {
        NodeData::Document => {
            for c in handle.children.borrow().iter() {
                audit_dom_no_forbidden(c, in_svg)?;
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
            if FORBIDDEN_ELEMENTS.contains(&tag.as_str()) {
                return Err(CmsPatcherError::MxssBlocked(format!(
                    "mxss-reparse-element:<{tag}>"
                )));
            }
            let is_svg_tag = ALLOWED_SVG_ELEMENTS.contains(&tag.as_str());
            let is_html_tag = ALLOWED_HTML_ELEMENTS.contains(&tag.as_str());
            if !is_svg_tag && !is_html_tag {
                return Err(CmsPatcherError::MxssBlocked(format!(
                    "mxss-reparse-disallowed:<{tag}>"
                )));
            }
            if in_svg && !is_svg_tag {
                return Err(CmsPatcherError::MxssBlocked(format!(
                    "mxss-reparse-svg-escape:<{tag}>"
                )));
            }
            let next_in_svg = in_svg || tag == "svg";
            if tag == "style" {
                let mut raw_css = String::new();
                collect_text_content(handle, &mut raw_css);
                validate_and_scrub_css_tokens(&raw_css, false)?;
                return Ok(());
            }
            for attr in attrs.borrow().iter() {
                let k: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
                let v = attr.value.to_string();
                if k.starts_with("on") || k == "srcdoc" || k == "xlink:href" {
                    return Err(CmsPatcherError::MxssBlocked(format!(
                        "mxss-reparse-attr:{k}"
                    )));
                }
                if next_in_svg && k == "href" {
                    return Err(CmsPatcherError::MxssBlocked(
                        "mxss-reparse-svg-href".to_string(),
                    ));
                }
                if (k == "src" || k == "href") && !is_safe_asset_url(&v) {
                    return Err(CmsPatcherError::MxssBlocked(format!(
                        "mxss-reparse-unsafe-url:{k}"
                    )));
                }
                if k == "style" {
                    validate_and_scrub_css_tokens(&v, true)?;
                }
            }
            for c in handle.children.borrow().iter() {
                audit_dom_no_forbidden(c, next_in_svg)?;
            }
        }
        NodeData::ProcessingInstruction { .. } => {
            return Err(CmsPatcherError::MxssBlocked("mxss-reparse-pi".to_string()));
        }
        _ => {}
    }
    Ok(())
}

fn parse_html5_dom(html: &str) -> RcDom {
    parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_default()
}

/// Fail-closed `html5ever` two-pass mXSS & CSS sanitizer with optional `one-page` `scope_slide_uid`.
pub fn sanitize_slide_html_deep_scoped(
    raw_html: &str,
    expected_ff_ids: &[String],
    resync_structured_slots: bool,
    scope_slide_uid: Option<&str>,
) -> Result<DeepSanitizeOutput, CmsPatcherError> {
    if raw_html.len() > MAX_SLIDE_HTML_BYTES {
        return Err(CmsPatcherError::PayloadTooLarge {
            actual: raw_html.len(),
            max: MAX_SLIDE_HTML_BYTES,
        });
    }

    // Pass 1: parse + strict allowlist serialize & extract slots
    let dom1 = parse_html5_dom(raw_html);
    let mut pass1_html = String::with_capacity(raw_html.len());
    let mut extracted = Vec::new();
    let mut seen_ff_ids: HashSet<(String, String)> = HashSet::new();
    let mut stamped = false;
    sanitize_and_serialize_node(
        &dom1.document,
        &mut pass1_html,
        &mut extracted,
        &mut seen_ff_ids,
        false,
        None,
        scope_slide_uid,
        None,
        &mut stamped,
    )?;

    // In `one-page` mode, restrict slot extraction & convergence strictly to `section[data-slide-uid="<uid>"]`
    let scoped_ff_ids: HashSet<String> = if let Some(uid) = scope_slide_uid {
        let (scoped_ids, _) = collect_scoped_ff_ids(&dom1.document, Some(uid));
        extracted.retain(|s| scoped_ids.contains(&s.ff_id));
        scoped_ids
    } else {
        seen_ff_ids.into_iter().map(|(_, ff)| ff).collect()
    };

    let unmapped_slot_ids: Vec<String> = expected_ff_ids
        .iter()
        .filter(|id| !scoped_ff_ids.contains(id.as_str()))
        .cloned()
        .collect();

    let custom_html_override = !unmapped_slot_ids.is_empty() || !resync_structured_slots;

    let final_html = if custom_html_override {
        let mut restamped = String::with_capacity(pass1_html.len() + 32);
        let mut extracted2 = Vec::new();
        let mut seen2: HashSet<(String, String)> = HashSet::new();
        let mut stamped2 = false;
        let dom_stamp = parse_html5_dom(&pass1_html);
        sanitize_and_serialize_node(
            &dom_stamp.document,
            &mut restamped,
            &mut extracted2,
            &mut seen2,
            false,
            Some(true),
            scope_slide_uid,
            None,
            &mut stamped2,
        )?;
        if scope_slide_uid.is_some() {
            extracted2.retain(|s| scoped_ff_ids.contains(&s.ff_id));
        }
        extracted = extracted2;
        restamped
    } else {
        pass1_html
    };

    if final_html.len() > MAX_SLIDE_HTML_BYTES {
        return Err(CmsPatcherError::PayloadTooLarge {
            actual: final_html.len(),
            max: MAX_SLIDE_HTML_BYTES,
        });
    }

    // Pass 2: double-parse mXSS idempotency verification
    let dom2 = parse_html5_dom(&final_html);
    audit_dom_no_forbidden(&dom2.document, false)?;

    let content_sha256 = compute_sha256_hex(final_html.as_bytes());
    let converged_slots_count = extracted.len();
    Ok(DeepSanitizeOutput {
        sanitized_html: final_html,
        content_sha256,
        extracted_slots: extracted,
        converged_slots_count,
        custom_html_override,
        unmapped_slot_ids,
    })
}

/// Fail-closed `html5ever` two-pass mXSS & CSS sanitizer with `[data-ff-id]` convergence extraction.
pub fn sanitize_slide_html_deep(
    raw_html: &str,
    expected_ff_ids: &[String],
    resync_structured_slots: bool,
) -> Result<DeepSanitizeOutput, CmsPatcherError> {
    sanitize_slide_html_deep_scoped(raw_html, expected_ff_ids, resync_structured_slots, None)
}

fn collect_scoped_ff_ids(root: &Handle, scope_slide_uid: Option<&str>) -> (HashSet<String>, bool) {
    let mut found_ids = HashSet::new();
    let mut has_override_flag = false;

    fn walk(
        handle: &Handle,
        scope_uid: Option<&str>,
        in_scope: bool,
        found_ids: &mut HashSet<String>,
        has_override_flag: &mut bool,
    ) {
        let mut next_in_scope = in_scope;
        if let NodeData::Element { name, attrs, .. } = &handle.data {
            let tag_lower: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
            let borrowed = attrs.borrow();
            if tag_lower == "section" {
                if let Some(target_uid) = scope_uid {
                    for attr in borrowed.iter() {
                        let k: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
                        if k == "data-slide-uid" {
                            next_in_scope = attr.value.trim() == target_uid;
                        }
                    }
                }
            }
            for attr in borrowed.iter() {
                let k: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
                if k == "data-ff-id" && next_in_scope {
                    found_ids.insert(attr.value.to_string());
                } else if k == CUSTOM_OVERRIDE_ATTR && attr.value.trim() == "1" && next_in_scope {
                    *has_override_flag = true;
                }
            }
        }
        for child in handle.children.borrow().iter() {
            walk(
                child,
                scope_uid,
                next_in_scope,
                found_ids,
                has_override_flag,
            );
        }
    }

    walk(
        root,
        scope_slide_uid,
        scope_slide_uid.is_none(),
        &mut found_ids,
        &mut has_override_flag,
    );
    (found_ids, has_override_flag)
}

/// Inspects committed SHA-256-verified slide HTML (optionally scoped to `scope_slide_uid`).
pub fn inspect_slide_html_convergence_scoped(
    html: &str,
    committed_ff_ids: &[String],
    scope_slide_uid: Option<&str>,
) -> (bool, Vec<String>) {
    let dom = parse_html5_dom(html);
    let (found_ids, has_override_flag) = collect_scoped_ff_ids(&dom.document, scope_slide_uid);
    let unmapped: Vec<String> = committed_ff_ids
        .iter()
        .filter(|id| !found_ids.contains(id.as_str()))
        .cloned()
        .collect();
    let is_override = has_override_flag || !unmapped.is_empty();
    (is_override, unmapped)
}

/// Inspects committed SHA-256-verified slide HTML to determine zero-migration `customHtmlOverride` state.
pub fn inspect_slide_html_convergence(
    html: &str,
    committed_ff_ids: &[String],
) -> (bool, Vec<String>) {
    inspect_slide_html_convergence_scoped(html, committed_ff_ids, None)
}

fn section_has_non_empty_content(handle: &Handle) -> bool {
    for child in handle.children.borrow().iter() {
        match &child.data {
            NodeData::Element { .. } => return true,
            NodeData::Text { contents } if !contents.borrow().trim().is_empty() => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Extracts all non-empty `<section data-slide-uid="...">` values present in an HTML document
/// (enforces real sibling `<section>` preservation in `one-page` decks; rejects spoofing via `<div>` or empty stubs).
pub fn extract_slide_uids_in_html(html: &str) -> HashSet<String> {
    let dom = parse_html5_dom(html);
    let mut uids = HashSet::new();
    fn walk(handle: &Handle, uids: &mut HashSet<String>) {
        if let NodeData::Element { name, attrs, .. } = &handle.data {
            let tag_lower: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
            if tag_lower == "section" && section_has_non_empty_content(handle) {
                for attr in attrs.borrow().iter() {
                    let k: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
                    if k == "data-slide-uid" {
                        let v = attr.value.trim();
                        if !v.is_empty() {
                            uids.insert(v.to_string());
                        }
                    }
                }
            }
        }
        for child in handle.children.borrow().iter() {
            walk(child, uids);
        }
    }
    walk(&dom.document, &mut uids);
    uids
}
