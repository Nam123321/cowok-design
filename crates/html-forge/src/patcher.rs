// @story 05.4
// @implements AC-04, AC-05
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use html5ever::tendril::TendrilSink;
use html5ever::{parse_document, ParseOpts};
use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, RewriteStrSettings};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const MAX_HTML_BYTES: usize = 512 * 1024;
pub const MAX_SLOT_TEXT_CHARS: usize = 8_000;
pub const MAX_FF_ID_LEN: usize = 64;

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
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "gap",
    "display",
    "align-items",
    "justify-content",
    "flex-direction",
    "flex-wrap",
    "max-width",
    "box-shadow",
];

const ALLOWED_DISPLAY_VALUES: &[&str] = &[
    "none",
    "block",
    "inline",
    "inline-block",
    "flex",
    "inline-flex",
    "grid",
];

const ALLOWED_ALIGN_ITEMS_VALUES: &[&str] = &[
    "stretch",
    "center",
    "flex-start",
    "flex-end",
    "start",
    "end",
    "baseline",
];

const ALLOWED_JUSTIFY_CONTENT_VALUES: &[&str] = &[
    "flex-start",
    "flex-end",
    "start",
    "end",
    "center",
    "space-between",
    "space-around",
    "space-evenly",
];

const ALLOWED_FLEX_DIRECTION_VALUES: &[&str] = &["row", "row-reverse", "column", "column-reverse"];

const ALLOWED_FLEX_WRAP_VALUES: &[&str] = &["nowrap", "wrap", "wrap-reverse"];

const ALLOWED_TEXT_ALIGN_VALUES: &[&str] = &["left", "center", "right", "justify", "start", "end"];

const ALLOWED_FONT_WEIGHT_VALUES: &[&str] = &[
    "normal", "bold", "lighter", "bolder", "100", "200", "300", "400", "500", "600", "700", "800",
    "900",
];

const FORBIDDEN_ELEMENTS_COMMON: &[&str] = &[
    "script",
    "foreignobject",
    "iframe",
    "object",
    "embed",
    "meta",
    "base",
    "noscript",
    "template",
    "applet",
    "link",
    "math",
];

const FORBIDDEN_SLIDE_ONLY_ELEMENTS: &[&str] = &["form", "input", "textarea", "select"];

const ALLOWED_HTML_ELEMENTS_COMMON: &[&str] = &[
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

const ALLOWED_APP_PREVIEW_EXTRA_ELEMENTS: &[&str] = &[
    "button", "label", "form", "input", "textarea", "select", "option", "fieldset", "legend",
    "dialog", "details", "summary", "dl", "dt", "dd",
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

const ALLOWED_VOID_ELEMENTS: &[&str] = &["br", "hr", "img", "input"];

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

const ALLOWED_APP_PREVIEW_EXTRA_ATTRS: &[&str] = &[
    "type",
    "name",
    "value",
    "placeholder",
    "disabled",
    "checked",
    "selected",
    "readonly",
    "for",
    "tabindex",
    "open",
    "rows",
    "cols",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SanitizeProfile {
    Slide,
    AppPreview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HtmlPatchOperation {
    TextEdit {
        text: String,
    },
    TokenSwap {
        css_vars: BTreeMap<String, String>,
    },
    SteeringChip {
        chip_id: String,
        safe_styles: BTreeMap<String, String>,
    },
    StylePatch {
        safe_styles: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlPatchOutput {
    pub patched_html: String,
    pub patched_count: usize,
    pub content_sha256: String,
    pub execution_time_micros: u64,
}

#[derive(Debug, Error, PartialEq)]
pub enum HtmlForgeError {
    #[error("E_PAYLOAD_TOO_LARGE: HTML byte length {actual} exceeds {max}")]
    PayloadTooLarge { actual: usize, max: usize },
    #[error("E_PAYLOAD_TOO_LARGE: Slot text length {actual} exceeds {max} characters")]
    SlotTextTooLong { actual: usize, max: usize },
    #[error("VALIDATION_ERROR: Invalid data-ff-id '{0}'")]
    InvalidFfId(String),
    #[error("E_MXSS_BLOCKED: Blocked dangerous HTML construct '{0}'")]
    MxssBlocked(String),
    #[error("E_MXSS_BLOCKED: Blocked unsafe CSS token '{0}'")]
    CssTokenBlocked(String),
    #[error("VALIDATION_ERROR: Invalid patch payload: {0}")]
    InvalidPayload(String),
    #[error("STALE_MAPPING: Unable to reconcile data-ff-id(s) {missing_ff_ids:?} (lowest_confidence={lowest_confidence:.2}, threshold={threshold:.2})")]
    StaleMapping {
        missing_ff_ids: Vec<String>,
        lowest_confidence: f64,
        threshold: f64,
    },
    #[error("INTERNAL_ERROR: HTML rewriter failed: {0}")]
    RewriterFailure(String),
}

pub fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Validates canonical `[data-ff-id]` syntax (`^[a-zA-Z0-9_-]{1,64}$`) and rejects JS prototype keys.
pub fn is_valid_ff_id(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_FF_ID_LEN {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    if matches!(lower.as_str(), "__proto__" | "constructor" | "prototype") {
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

pub fn strip_css_comments_fail_closed(input: &str) -> Result<String, HtmlForgeError> {
    if input.contains('\0') {
        return Err(HtmlForgeError::CssTokenBlocked("null-byte".to_string()));
    }
    if input.contains('\\') {
        return Err(HtmlForgeError::CssTokenBlocked(
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
                return Err(HtmlForgeError::CssTokenBlocked(
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

pub fn scan_forbidden_css_tokens(stripped: &str) -> Result<(), HtmlForgeError> {
    let lower = stripped.to_ascii_lowercase();
    let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();
    if lower.contains('<') || compact.contains("</style") {
        return Err(HtmlForgeError::CssTokenBlocked("</style".to_string()));
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
            return Err(HtmlForgeError::CssTokenBlocked(needle.to_string()));
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

fn is_valid_css_var_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("--") else {
        return false;
    };
    !rest.is_empty()
        && rest.len() <= 64
        && rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_valid_css_var_fallback(fallback: &str) -> bool {
    let fb = fallback.trim();
    if fb.is_empty() || fb.len() > 64 {
        return false;
    }
    if is_valid_hex_color(fb) || is_valid_rgb_color(fb) || is_valid_css_length_token(fb) {
        return true;
    }
    // Safe ASCII CSS system/named color keyword (e.g. LinkText, GrayText, Canvas, Green, inherit)
    fb.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-')
}

fn is_valid_css_var(v: &str) -> bool {
    let Some(inner) = v.strip_prefix("var(--").and_then(|s| s.strip_suffix(')')) else {
        return false;
    };
    let (var_name, fallback) = match inner.split_once(',') {
        Some((name, fb)) => (name.trim(), Some(fb)),
        None => (inner.trim(), None),
    };
    let valid_name = !var_name.is_empty()
        && var_name.len() <= 64
        && var_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if !valid_name {
        return false;
    }
    fallback.is_none_or(is_valid_css_var_fallback)
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
    !num.is_empty() && num.parse::<f64>().is_ok_and(f64::is_finite)
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
    if is_valid_css_var_name(prop) {
        return val == "transparent"
            || val == "inherit"
            || is_valid_hex_color(val)
            || is_valid_rgb_color(val)
            || is_valid_css_length_token(val)
            || val.parse::<f64>().is_ok_and(f64::is_finite);
    }
    match prop {
        "color" | "background-color" | "border-color" => {
            val == "transparent"
                || val == "currentColor"
                || val == "inherit"
                || is_valid_hex_color(val)
                || is_valid_rgb_color(val)
        }
        "font-size" | "letter-spacing" | "border-radius" | "gap" | "padding" | "padding-top"
        | "padding-right" | "padding-bottom" | "padding-left" | "margin" | "margin-top"
        | "margin-right" | "margin-bottom" | "margin-left" | "max-width" => {
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
        "align-items" => ALLOWED_ALIGN_ITEMS_VALUES.contains(&val.to_ascii_lowercase().as_str()),
        "justify-content" => {
            ALLOWED_JUSTIFY_CONTENT_VALUES.contains(&val.to_ascii_lowercase().as_str())
        }
        "flex-direction" => {
            ALLOWED_FLEX_DIRECTION_VALUES.contains(&val.to_ascii_lowercase().as_str())
        }
        "flex-wrap" => ALLOWED_FLEX_WRAP_VALUES.contains(&val.to_ascii_lowercase().as_str()),
        "font-family" => {
            val.len() <= 128
                && val.bytes().all(|b| {
                    b.is_ascii_alphanumeric()
                        || matches!(b, b' ' | b',' | b'-' | b'_' | b'"' | b'\'')
                })
        }
        "box-shadow" => {
            val == "none"
                || (val.len() <= 128
                    && val.bytes().all(|b| {
                        b.is_ascii_alphanumeric()
                            || matches!(b, b' ' | b',' | b'-' | b'.' | b'#' | b'(' | b')' | b'%')
                    }))
        }
        _ => false,
    }
}

/// Fail-closed deterministic CSS sanitizer for both `style=""` inline attributes and `<style>` blocks.
pub fn validate_and_scrub_css_tokens(
    css: &str,
    is_inline_style: bool,
) -> Result<String, HtmlForgeError> {
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
                return Err(HtmlForgeError::CssTokenBlocked(format!(
                    "malformed-decl:{trimmed}"
                )));
            };
            let prop = raw_prop.trim().to_ascii_lowercase();
            let val = raw_val.trim();
            if !is_valid_css_var_name(&prop) && !ALLOWED_INLINE_CSS_PROPS.contains(&prop.as_str()) {
                return Err(HtmlForgeError::CssTokenBlocked(format!(
                    "disallowed-prop:{prop}"
                )));
            }
            if !is_valid_css_value(&prop, val) {
                return Err(HtmlForgeError::CssTokenBlocked(format!(
                    "invalid-value:{prop}:{val}"
                )));
            }
            normalized.push(format!("{prop}: {val}"));
        }
        return Ok(normalized.join("; "));
    }

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
                    return Err(HtmlForgeError::CssTokenBlocked(
                        "excessive-css-nesting".to_string(),
                    ));
                }
                selector_stack.push(std::mem::take(&mut pending_prelude));
                body_stack.push(String::new());
            }
            '}' => {
                let Some(sel) = selector_stack.pop() else {
                    return Err(HtmlForgeError::CssTokenBlocked(
                        "unbalanced-braces".to_string(),
                    ));
                };
                let block = body_stack.pop().unwrap_or_default();
                let combined_block = format!("{block}{pending_prelude}");
                pending_prelude.clear();
                if selector_targets_protected_root(&sel)
                    && combined_block.to_ascii_lowercase().contains("!important")
                {
                    return Err(HtmlForgeError::CssTokenBlocked(
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
        return Err(HtmlForgeError::CssTokenBlocked(
            "unbalanced-braces".to_string(),
        ));
    }
    Ok(stripped)
}

pub fn escape_html_text(input: &str) -> String {
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

pub fn escape_html_attr(input: &str) -> String {
    escape_html_text(input)
}

pub fn parse_html5_dom(html: &str) -> RcDom {
    parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_default()
}

fn is_forbidden_element(tag: &str, profile: SanitizeProfile) -> bool {
    if FORBIDDEN_ELEMENTS_COMMON.contains(&tag) {
        return true;
    }
    matches!(profile, SanitizeProfile::Slide) && FORBIDDEN_SLIDE_ONLY_ELEMENTS.contains(&tag)
}

fn is_allowed_html_element(tag: &str, profile: SanitizeProfile) -> bool {
    ALLOWED_HTML_ELEMENTS_COMMON.contains(&tag)
        || (matches!(profile, SanitizeProfile::AppPreview)
            && ALLOWED_APP_PREVIEW_EXTRA_ELEMENTS.contains(&tag))
}

fn is_allowed_attr(attr_name: &str, in_svg: bool, profile: SanitizeProfile) -> bool {
    if attr_name.starts_with("data-") || attr_name.starts_with("aria-") {
        return attr_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    }
    if in_svg && ALLOWED_SVG_ATTRS.contains(&attr_name) {
        return true;
    }
    if ALLOWED_COMMON_ATTRS.contains(&attr_name) {
        return true;
    }
    matches!(profile, SanitizeProfile::AppPreview)
        && ALLOWED_APP_PREVIEW_EXTRA_ATTRS.contains(&attr_name)
}

fn collect_text_content(handle: &Handle, out: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => out.push_str(&contents.borrow()),
        NodeData::Element { .. } | NodeData::Document => {
            for c in handle.children.borrow().iter() {
                collect_text_content(c, out);
            }
        }
        _ => {}
    }
}

fn sanitize_and_serialize_node(
    handle: &Handle,
    out: &mut String,
    in_svg: bool,
    profile: SanitizeProfile,
    seen_ff_ids: &mut HashSet<String>,
) -> Result<(), HtmlForgeError> {
    match &handle.data {
        NodeData::Document => {
            for child in handle.children.borrow().iter() {
                sanitize_and_serialize_node(child, out, in_svg, profile, seen_ff_ids)?;
            }
        }
        NodeData::Doctype { .. } => out.push_str("<!DOCTYPE html>"),
        NodeData::Comment { .. } => {}
        NodeData::Text { contents } => out.push_str(&escape_html_text(&contents.borrow())),
        NodeData::Element { name, attrs, .. } => {
            let tag: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
            if is_forbidden_element(&tag, profile) {
                return Err(HtmlForgeError::MxssBlocked(format!("<{tag}>")));
            }
            let is_svg_tag = ALLOWED_SVG_ELEMENTS.contains(&tag.as_str());
            let is_html_tag = is_allowed_html_element(&tag, profile);
            if !is_svg_tag && !is_html_tag {
                return Err(HtmlForgeError::MxssBlocked(format!("disallowed:<{tag}>")));
            }
            if in_svg && !is_svg_tag {
                return Err(HtmlForgeError::MxssBlocked(format!(
                    "svg-namespace-escape:<{tag}>"
                )));
            }
            let next_in_svg = in_svg || tag == "svg";

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
            for attr in attrs.borrow().iter() {
                let attr_name: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
                let mut attr_val = attr.value.to_string();
                if attr_name.starts_with("on") || attr_name == "srcdoc" || attr_name == "xlink:href"
                {
                    return Err(HtmlForgeError::MxssBlocked(format!("attr:{attr_name}")));
                }
                if next_in_svg && attr_name == "href" {
                    return Err(HtmlForgeError::MxssBlocked("svg-href".to_string()));
                }
                if !is_allowed_attr(&attr_name, next_in_svg, profile) {
                    continue;
                }
                if (attr_name == "src" || attr_name == "href") && !is_safe_asset_url(&attr_val) {
                    return Err(HtmlForgeError::MxssBlocked(format!(
                        "unsafe-url:{attr_name}"
                    )));
                }
                if attr_name == "style" {
                    attr_val = validate_and_scrub_css_tokens(&attr_val, true)?;
                }
                if attr_name == "data-ff-id" {
                    if !is_valid_ff_id(&attr_val) {
                        return Err(HtmlForgeError::InvalidFfId(attr_val));
                    }
                    if !seen_ff_ids.insert(attr_val.clone()) {
                        return Err(HtmlForgeError::InvalidPayload(format!(
                            "Duplicate data-ff-id '{attr_val}'"
                        )));
                    }
                }
                safe_attrs.push((attr_name, attr_val));
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
                    sanitize_and_serialize_node(child, out, next_in_svg, profile, seen_ff_ids)?;
                }
                out.push_str("</");
                out.push_str(&tag);
                out.push('>');
            }
        }
        NodeData::ProcessingInstruction { .. } => {
            return Err(HtmlForgeError::MxssBlocked(
                "processing-instruction".to_string(),
            ));
        }
    }
    Ok(())
}

fn audit_dom_no_forbidden(
    handle: &Handle,
    in_svg: bool,
    profile: SanitizeProfile,
) -> Result<(), HtmlForgeError> {
    match &handle.data {
        NodeData::Document => {
            for c in handle.children.borrow().iter() {
                audit_dom_no_forbidden(c, in_svg, profile)?;
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
            if is_forbidden_element(&tag, profile) {
                return Err(HtmlForgeError::MxssBlocked(format!(
                    "mxss-reparse-element:<{tag}>"
                )));
            }
            let is_svg_tag = ALLOWED_SVG_ELEMENTS.contains(&tag.as_str());
            let is_html_tag = is_allowed_html_element(&tag, profile);
            if !is_svg_tag && !is_html_tag {
                return Err(HtmlForgeError::MxssBlocked(format!(
                    "mxss-reparse-disallowed:<{tag}>"
                )));
            }
            if in_svg && !is_svg_tag {
                return Err(HtmlForgeError::MxssBlocked(format!(
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
                    return Err(HtmlForgeError::MxssBlocked(format!(
                        "mxss-reparse-attr:{k}"
                    )));
                }
                if next_in_svg && k == "href" {
                    return Err(HtmlForgeError::MxssBlocked(
                        "mxss-reparse-svg-href".to_string(),
                    ));
                }
                if (k == "src" || k == "href") && !is_safe_asset_url(&v) {
                    return Err(HtmlForgeError::MxssBlocked(format!(
                        "mxss-reparse-unsafe-url:{k}"
                    )));
                }
                if k == "style" {
                    validate_and_scrub_css_tokens(&v, true)?;
                }
            }
            for c in handle.children.borrow().iter() {
                audit_dom_no_forbidden(c, next_in_svg, profile)?;
            }
        }
        NodeData::ProcessingInstruction { .. } => {
            return Err(HtmlForgeError::MxssBlocked("mxss-reparse-pi".to_string()));
        }
        _ => {}
    }
    Ok(())
}

/// Fail-closed two-pass `html5ever` mXSS sanitizer supporting both `Slide` and `AppPreview` profiles.
pub fn sanitize_html_with_profile(
    raw_html: &str,
    profile: SanitizeProfile,
) -> Result<(String, String), HtmlForgeError> {
    if raw_html.len() > MAX_HTML_BYTES {
        return Err(HtmlForgeError::PayloadTooLarge {
            actual: raw_html.len(),
            max: MAX_HTML_BYTES,
        });
    }
    let dom1 = parse_html5_dom(raw_html);
    let mut sanitized = String::with_capacity(raw_html.len());
    let mut seen_ff_ids = HashSet::new();
    sanitize_and_serialize_node(
        &dom1.document,
        &mut sanitized,
        false,
        profile,
        &mut seen_ff_ids,
    )?;
    if sanitized.len() > MAX_HTML_BYTES {
        return Err(HtmlForgeError::PayloadTooLarge {
            actual: sanitized.len(),
            max: MAX_HTML_BYTES,
        });
    }
    let dom2 = parse_html5_dom(&sanitized);
    audit_dom_no_forbidden(&dom2.document, false, profile)?;
    let sha = compute_sha256_hex(sanitized.as_bytes());
    Ok((sanitized, sha))
}

fn format_safe_styles_map(styles: &BTreeMap<String, String>) -> Result<String, HtmlForgeError> {
    if styles.is_empty() {
        return Ok(String::new());
    }
    let mut decls = Vec::with_capacity(styles.len());
    for (k, v) in styles {
        decls.push(format!("{k}: {v}"));
    }
    validate_and_scrub_css_tokens(&decls.join("; "), true)
}

fn merge_inline_style(existing: Option<String>, incoming: &str) -> Result<String, HtmlForgeError> {
    if incoming.is_empty() {
        return Ok(existing.unwrap_or_default());
    }
    let mut merged: BTreeMap<String, String> = BTreeMap::new();
    if let Some(prev) = existing {
        if !prev.trim().is_empty() {
            let scrubbed_prev = validate_and_scrub_css_tokens(&prev, true)?;
            for decl in scrubbed_prev.split(';') {
                if let Some((k, v)) = decl.trim().split_once(':') {
                    merged.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
                }
            }
        }
    }
    for decl in incoming.split(';') {
        if let Some((k, v)) = decl.trim().split_once(':') {
            merged.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    format_safe_styles_map(&merged)
}

/// Executes a targeted `lol_html` surgical patch on `[data-ff-id="<ff_id>"]`, then verifies
/// double-parse mXSS safety via `html5ever` under `profile`.
pub fn patch_html_element(
    raw_html: &str,
    ff_id: &str,
    op: &HtmlPatchOperation,
    profile: SanitizeProfile,
) -> Result<HtmlPatchOutput, HtmlForgeError> {
    let start = Instant::now();
    if raw_html.len() > MAX_HTML_BYTES {
        return Err(HtmlForgeError::PayloadTooLarge {
            actual: raw_html.len(),
            max: MAX_HTML_BYTES,
        });
    }
    if !is_valid_ff_id(ff_id) {
        return Err(HtmlForgeError::InvalidFfId(ff_id.to_string()));
    }

    let prepared_style = match op {
        HtmlPatchOperation::TextEdit { text } => {
            if text.chars().count() > MAX_SLOT_TEXT_CHARS {
                return Err(HtmlForgeError::SlotTextTooLong {
                    actual: text.chars().count(),
                    max: MAX_SLOT_TEXT_CHARS,
                });
            }
            if text.contains('\0') {
                return Err(HtmlForgeError::InvalidPayload(
                    "NULL_BYTE_FORBIDDEN in text edit".to_string(),
                ));
            }
            None
        }
        HtmlPatchOperation::TokenSwap { css_vars } => {
            if css_vars.is_empty() {
                return Err(HtmlForgeError::InvalidPayload(
                    "TokenSwap requires at least one --* CSS variable".to_string(),
                ));
            }
            for k in css_vars.keys() {
                if !is_valid_css_var_name(k) {
                    return Err(HtmlForgeError::CssTokenBlocked(format!(
                        "invalid-css-var:{k}"
                    )));
                }
            }
            Some(format_safe_styles_map(css_vars)?)
        }
        HtmlPatchOperation::SteeringChip {
            chip_id,
            safe_styles,
        } => {
            if chip_id.trim().is_empty() || chip_id.len() > 64 {
                return Err(HtmlForgeError::InvalidPayload(
                    "Invalid steering chip_id".to_string(),
                ));
            }
            Some(format_safe_styles_map(safe_styles)?)
        }
        HtmlPatchOperation::StylePatch { safe_styles } => {
            Some(format_safe_styles_map(safe_styles)?)
        }
    };

    let patched_count = Rc::new(RefCell::new(0usize));
    let err_slot: Rc<RefCell<Option<HtmlForgeError>>> = Rc::new(RefCell::new(None));
    let count_ref = Rc::clone(&patched_count);
    let err_ref = Rc::clone(&err_slot);
    let target_ff_id = ff_id.to_string();
    let op_clone = op.clone();
    let style_clone = prepared_style.clone();

    let rewritten = rewrite_str(
        raw_html,
        RewriteStrSettings::new().append_element_content_handler(element!(
            "[data-ff-id]",
            move |el| {
                let current_id = el.get_attribute("data-ff-id").unwrap_or_default();
                if current_id != target_ff_id {
                    return Ok(());
                }
                *count_ref.borrow_mut() += 1;
                match &op_clone {
                    HtmlPatchOperation::TextEdit { text } => {
                        el.set_inner_content(text, ContentType::Text);
                    }
                    HtmlPatchOperation::TokenSwap { .. }
                    | HtmlPatchOperation::StylePatch { .. } => {
                        if let Some(ref incoming) = style_clone {
                            match merge_inline_style(el.get_attribute("style"), incoming) {
                                Ok(merged) => {
                                    if !merged.is_empty() {
                                        let _ = el.set_attribute("style", &merged);
                                    }
                                }
                                Err(e) => {
                                    *err_ref.borrow_mut() = Some(e);
                                }
                            }
                        }
                    }
                    HtmlPatchOperation::SteeringChip { chip_id, .. } => {
                        let _ = el.set_attribute("data-steering-chip", chip_id);
                        if let Some(ref incoming) = style_clone {
                            match merge_inline_style(el.get_attribute("style"), incoming) {
                                Ok(merged) => {
                                    if !merged.is_empty() {
                                        let _ = el.set_attribute("style", &merged);
                                    }
                                }
                                Err(e) => {
                                    *err_ref.borrow_mut() = Some(e);
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
        )),
    )
    .map_err(|e| HtmlForgeError::RewriterFailure(e.to_string()))?;

    if let Some(err) = err_slot.borrow_mut().take() {
        return Err(err);
    }

    let (sanitized_html, content_sha256) = sanitize_html_with_profile(&rewritten, profile)?;
    let count = *patched_count.borrow();
    Ok(HtmlPatchOutput {
        patched_html: sanitized_html,
        patched_count: count,
        content_sha256,
        execution_time_micros: start.elapsed().as_micros() as u64,
    })
}
