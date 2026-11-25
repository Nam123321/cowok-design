// @story 07.2
// @implements AC-01, AC-03, AC-04
//! Self-contained offline HTML bundler, deterministic 16:9 Vector PDF generator,
//! and Developer Handoff packager inside the shared `crates/html-forge` kernel (ADR-006).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, text, RewriteStrSettings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Strict 25 MiB cap for offline self-contained bundles and PDF outputs to prevent OOM.
pub const MAX_BUNDLE_BYTES: usize = 25 * 1024 * 1024;

/// Hardened zero-network base Content-Security-Policy embedded into every offline `.html` export.
/// Inline scripts are NEVER permitted via `'unsafe-inline'`; `bundle_offline_html` appends either
/// `script-src 'sha256-<presenter_hash>';` (when presenter is enabled) or `script-src 'none';`.
pub const OFFLINE_CSP_CONTENT: &str = "default-src 'none'; style-src 'self' 'unsafe-inline' data:; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' data: blob:; connect-src 'none'; base-uri 'none'; form-action 'none'; object-src 'none'; frame-src 'none';";

/// Standard 16:9 landscape page dimensions in PDF points (1920x1080 @ 96dpi -> 1440x810pt).
pub const VECTOR_PDF_WIDTH_PT: f64 = 1440.0;
pub const VECTOR_PDF_HEIGHT_PT: f64 = 810.0;

const OFFLINE_IMG_PLACEHOLDER_DATA_URI: &str = "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%20320%20180%22%3E%3Crect%20width%3D%22320%22%20height%3D%22180%22%20fill%3D%22%23e2e8f0%22%2F%3E%3Ctext%20x%3D%22160%22%20y%3D%2296%22%20font-family%3D%22sans-serif%22%20font-size%3D%2214%22%20text-anchor%3D%22middle%22%20fill%3D%22%23475569%22%3EOffline%20asset%20placeholder%3C%2Ftext%3E%3C%2Fsvg%3E";

const STAGESHOW_PRESENTER_CSS: &str = r#"
/* Stageshow 16:9 Offline Presenter Runtime */
html, body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  background: #0f172a;
  color: #f8fafc;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
}
[data-stageshow-stage="true"] {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  padding: 24px;
  box-sizing: border-box;
}
[data-stageshow-slide="true"] {
  width: min(96vw, calc(96vh * 16 / 9));
  aspect-ratio: 16 / 9;
  box-sizing: border-box;
  overflow: auto;
  border-radius: 12px;
  box-shadow: 0 20px 48px rgba(0, 0, 0, 0.35);
}
[data-stageshow-slide="true"][data-slide-hidden="true"] {
  display: none !important;
}
.stageshow-hud {
  position: fixed;
  right: 16px;
  bottom: 16px;
  z-index: 9999;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border-radius: 9999px;
  background: rgba(15, 23, 42, 0.85);
  color: #f8fafc;
  font-size: 12px;
  border: 1px solid rgba(148, 163, 184, 0.35);
}
.stageshow-hud button {
  background: transparent;
  color: inherit;
  border: 1px solid rgba(148, 163, 184, 0.4);
  border-radius: 6px;
  padding: 2px 8px;
  cursor: pointer;
}
"#;

const STAGESHOW_PRESENTER_JS: &str = r#"
(function () {
  var slides = Array.prototype.slice.call(
    document.querySelectorAll('section[data-slide], .slide, main > section, body > section')
  );
  if (!slides.length) {
    var wrapper = document.createElement('section');
    wrapper.setAttribute('data-slide', '1');
    while (document.body.firstChild) {
      wrapper.appendChild(document.body.firstChild);
    }
    document.body.appendChild(wrapper);
    slides = [wrapper];
  }
  var idx = 0;
  function renderSlide(next) {
    idx = Math.max(0, Math.min(slides.length - 1, next));
    slides.forEach(function (el, i) {
      el.setAttribute('data-stageshow-slide', 'true');
      if (i === idx) {
        el.removeAttribute('data-slide-hidden');
        el.setAttribute('data-slide-active', 'true');
      } else {
        el.setAttribute('data-slide-hidden', 'true');
        el.removeAttribute('data-slide-active');
      }
    });
    document.documentElement.setAttribute('data-stageshow-index', String(idx + 1));
    var counter = document.getElementById('stageshow-counter');
    if (counter) counter.textContent = (idx + 1) + ' / ' + slides.length;
  }
  function toggleFullscreen() {
    if (!document.fullscreenElement && document.documentElement.requestFullscreen) {
      document.documentElement.requestFullscreen().catch(function () {});
    } else if (document.fullscreenElement && document.exitFullscreen) {
      document.exitFullscreen().catch(function () {});
    }
  }
  var hud = document.createElement('div');
  hud.className = 'stageshow-hud';
  hud.setAttribute('data-stageshow-presenter', 'true');
  hud.innerHTML =
    '<button type="button" id="stageshow-prev" aria-label="Previous slide">&larr;</button>' +
    '<span id="stageshow-counter">1 / ' + slides.length + '</span>' +
    '<button type="button" id="stageshow-next" aria-label="Next slide">&rarr;</button>' +
    '<button type="button" id="stageshow-fs" aria-label="Toggle fullscreen">F</button>';
  document.body.appendChild(hud);
  document.getElementById('stageshow-prev').addEventListener('click', function () { renderSlide(idx - 1); });
  document.getElementById('stageshow-next').addEventListener('click', function () { renderSlide(idx + 1); });
  document.getElementById('stageshow-fs').addEventListener('click', toggleFullscreen);
  window.addEventListener('keydown', function (ev) {
    if (ev.key === 'ArrowRight' || ev.key === 'PageDown' || ev.key === ' ') {
      ev.preventDefault();
      renderSlide(idx + 1);
    } else if (ev.key === 'ArrowLeft' || ev.key === 'PageUp') {
      ev.preventDefault();
      renderSlide(idx - 1);
    } else if (ev.key === 'Home') {
      ev.preventDefault();
      renderSlide(0);
    } else if (ev.key === 'End') {
      ev.preventDefault();
      renderSlide(slides.length - 1);
    } else if (ev.key === 'f' || ev.key === 'F') {
      toggleFullscreen();
    }
  });
  renderSlide(0);
})();
"#;

#[derive(Debug, Error)]
pub enum ExportBundlerError {
    #[error("export artifact exceeds limit of {limit_bytes} bytes (actual: {actual_bytes} bytes)")]
    AssetSizeExceeded {
        actual_bytes: usize,
        limit_bytes: usize,
    },
    #[error(
        "offline export blocked: {count} external network reference(s) remained after sanitization"
    )]
    ExternalCdnLeak { count: usize },
    #[error("failed to rewrite HTML for offline bundle: {0}")]
    HtmlRewrite(String),
    #[error("invalid export input: {0}")]
    InvalidInput(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineHtmlInput {
    pub title: String,
    pub html: String,
    #[serde(default)]
    pub inline_css: Option<String>,
    #[serde(default)]
    pub cas_assets: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    pub enable_presenter: bool,
    #[serde(default)]
    pub max_bytes: Option<usize>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineHtmlBundle {
    pub file_name: String,
    pub html_content: String,
    pub sha256: String,
    pub byte_size: usize,
    pub external_cdn_refs_count: usize,
    pub stripped_external_refs_count: usize,
    pub presenter_enabled: bool,
    pub slide_count: usize,
    pub svg_count: usize,
    pub broken_svg_count: usize,
    pub inline_assets_count: usize,
    pub broken_cas_assets_count: usize,
    pub csp_header: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorPdfSlide {
    pub heading: String,
    #[serde(default)]
    pub body_lines: Vec<String>,
    #[serde(default)]
    pub footer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorPdfInput {
    pub title: String,
    #[serde(default)]
    pub slides: Vec<VectorPdfSlide>,
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorPdfBundle {
    pub file_name: String,
    pub pdf_base64: String,
    pub sha256: String,
    pub byte_size: usize,
    pub page_count: usize,
    pub width_pt: f64,
    pub height_pt: f64,
    pub engine: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperHandoffInput {
    pub project_name: String,
    #[serde(default)]
    pub spec_markdown: Option<String>,
    #[serde(default)]
    pub design_tokens_css: Option<String>,
    #[serde(default)]
    pub design_tokens_json: Option<String>,
    #[serde(default)]
    pub screen_tsx: Option<String>,
    #[serde(default)]
    pub html_source: Option<String>,
    #[serde(default)]
    pub e2e_spec_ts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperHandoffFile {
    pub path: String,
    pub mime_type: String,
    pub byte_size: usize,
    pub sha256: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperHandoffBundle {
    pub file_name: String,
    pub sha256: String,
    pub byte_size: usize,
    pub files: Vec<DeveloperHandoffFile>,
}

#[derive(Default)]
struct RewriteStats {
    stripped_external_refs: usize,
    slide_count: usize,
    svg_count: usize,
    broken_svg_count: usize,
    inline_assets_count: usize,
    broken_cas_assets_count: usize,
    head_injected: bool,
    body_injected: bool,
    cumulative_bytes: usize,
    size_exceeded: Option<usize>,
    warnings: Vec<String>,
}

impl RewriteStats {
    fn account_growth(&mut self, added_bytes: usize, limit: usize) -> Result<(), String> {
        self.cumulative_bytes = self.cumulative_bytes.saturating_add(added_bytes);
        if self.cumulative_bytes > limit {
            self.size_exceeded = Some(self.cumulative_bytes);
            return Err("ASSET_SIZE_EXCEEDED".to_string());
        }
        Ok(())
    }
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha256_bytes(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Computes the canonical hardened CSP header value for an offline HTML bundle,
/// pinning the Stageshow runtime script by its exact SHA-256 digest instead of `'unsafe-inline'`.
pub fn build_offline_csp_header(enable_presenter: bool) -> String {
    if enable_presenter {
        let digest = sha256_bytes(STAGESHOW_PRESENTER_JS.as_bytes());
        let b64 = encode_base64(&digest);
        format!("{OFFLINE_CSP_CONTENT} script-src 'sha256-{b64}';")
    } else {
        format!("{OFFLINE_CSP_CONTENT} script-src 'none';")
    }
}

fn slugify_filename(raw: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        fallback.to_string()
    } else if trimmed.len() > 64 {
        trimmed[..64].trim_end_matches('-').to_string()
    } else {
        trimmed
    }
}

fn escape_html_text_basic(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn normalize_uri_for_scheme_check(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_ascii_control() && !c.is_ascii_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn is_dangerous_uri(raw: &str) -> bool {
    let norm = normalize_uri_for_scheme_check(raw);
    norm.starts_with("javascript:")
        || norm.starts_with("vbscript:")
        || norm.starts_with("data:text/html")
        || norm.starts_with("data:application/xhtml+xml")
}

fn is_external_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }
    let decoded = decode_css_escapes_all(trimmed);
    let norm = normalize_uri_for_scheme_check(&decoded);
    norm.starts_with("http://") || norm.starts_with("https://") || norm.starts_with("//")
}

fn is_valid_data_uri(raw: &str) -> bool {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("data:") || is_dangerous_uri(trimmed) {
        return false;
    }
    match trimmed.split_once(',') {
        Some((meta, payload)) => {
            let meta_ok = meta.len() >= 5;
            let payload_trimmed = payload.trim();
            if !meta_ok || payload_trimmed.is_empty() {
                return false;
            }
            if meta.to_ascii_lowercase().contains(";base64") {
                payload_trimmed.bytes().all(|b| {
                    b.is_ascii_alphanumeric()
                        || matches!(b, b'+' | b'/' | b'=' | b'-' | b'_' | b'\n' | b'\r' | b' ')
                })
            } else {
                true
            }
        }
        None => false,
    }
}

fn is_valid_svg_viewbox(raw: &str) -> bool {
    let parts: Vec<&str> = raw
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 4 {
        return false;
    }
    let nums: Result<Vec<f64>, _> = parts.iter().map(|p| p.parse::<f64>()).collect();
    match nums {
        Ok(v) => v.iter().all(|n| n.is_finite()) && v[2] > 0.0 && v[3] > 0.0,
        Err(_) => false,
    }
}

/// Neutralizes case-insensitive `</style` sequences inside CSS fail-closed by truncating at
/// the breakout attempt so neither `</style` nor any trailing injected `<script>` payload survives.
fn neutralize_style_breakout(css: &str) -> String {
    let lower = css.to_ascii_lowercase();
    if let Some(pos) = lower.find("</style") {
        let mut out = String::with_capacity(pos + 36);
        out.push_str(&css[..pos]);
        out.push_str("/* offline-stripped-style-close */");
        out
    } else {
        css.to_string()
    }
}

/// Strips CSS comments (`/* ... */`) and NUL bytes fail-closed: an unclosed `/*` drops the
/// remainder of the stylesheet so obfuscated tokens cannot hide past it.
fn strip_css_comments_scrub(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    let mut in_single = false;
    let mut in_double = false;

    while i < bytes.len() {
        let b = bytes[i];
        if b == 0 {
            i += 1;
            continue;
        }
        if !in_single && !in_double && i + 1 < bytes.len() && b == b'/' && bytes[i + 1] == b'*' {
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
                // Fail-closed on unclosed CSS comment
                break;
            }
            out.push(' ');
            continue;
        }
        if b == b'\\' && i + 1 < bytes.len() {
            let ch = input[i..].chars().next().unwrap_or('\\');
            out.push(ch);
            i += ch.len_utf8();
            if i < bytes.len() {
                let next_ch = input[i..].chars().next().unwrap_or(' ');
                out.push(next_ch);
                i += next_ch.len_utf8();
            }
            continue;
        }
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        }
        let ch = input[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Decodes CSS backslash escapes (1-6 hex digits + optional whitespace, or single-char escape).
/// When `preserve_string_quotes` is true, keeps `\"`, `\'`, and `\\` escaped inside quoted strings.
fn decode_css_escapes(input: &str, preserve_string_quotes: bool) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    let mut in_single = false;
    let mut in_double = false;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' {
            if i + 1 >= bytes.len() {
                i += 1;
                continue;
            }
            let next_b = bytes[i + 1];
            // Line continuation
            if next_b == b'\n' || next_b == b'\x0c' {
                i += 2;
                continue;
            }
            if next_b == b'\r' {
                i += if i + 2 < bytes.len() && bytes[i + 2] == b'\n' {
                    3
                } else {
                    2
                };
                continue;
            }
            if next_b.is_ascii_hexdigit() {
                let mut j = i + 1;
                let mut val: u32 = 0;
                while j < bytes.len() && j - (i + 1) < 6 && bytes[j].is_ascii_hexdigit() {
                    let digit = match bytes[j] {
                        b'0'..=b'9' => (bytes[j] - b'0') as u32,
                        b'a'..=b'f' => (bytes[j] - b'a' + 10) as u32,
                        b'A'..=b'F' => (bytes[j] - b'A' + 10) as u32,
                        _ => 0,
                    };
                    val = (val << 4) | digit;
                    j += 1;
                }
                // Consume optional single whitespace after hex escape
                if j < bytes.len() {
                    if bytes[j] == b'\r' && j + 1 < bytes.len() && bytes[j + 1] == b'\n' {
                        j += 2;
                    } else if matches!(bytes[j], b' ' | b'\t' | b'\n' | b'\r' | b'\x0c') {
                        j += 1;
                    }
                }
                let decoded = if val == 0 || val > 0x10FFFF {
                    '\u{FFFD}'
                } else {
                    char::from_u32(val).unwrap_or('\u{FFFD}')
                };
                if preserve_string_quotes
                    && (in_single || in_double)
                    && matches!(decoded, '"' | '\'' | '\\')
                {
                    out.push('\\');
                    out.push(decoded);
                } else {
                    out.push(decoded);
                }
                i = j;
                continue;
            } else {
                let ch = input[i + 1..].chars().next().unwrap_or(' ');
                if preserve_string_quotes
                    && (in_single || in_double)
                    && matches!(ch, '"' | '\'' | '\\')
                {
                    out.push('\\');
                    out.push(ch);
                } else {
                    out.push(ch);
                }
                i += 1 + ch.len_utf8();
                continue;
            }
        }
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        }
        let ch = input[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn decode_css_escapes_all(input: &str) -> String {
    decode_css_escapes(input, false)
}

/// Checks if `bytes[pos..]` starts with `ascii_lower_needle` (case-insensitive) and is not
/// immediately preceded by an identifier character (`[a-zA-Z0-9_-]`).
fn matches_css_func_at(bytes: &[u8], pos: usize, ascii_lower_needle: &[u8]) -> bool {
    if pos + ascii_lower_needle.len() > bytes.len() {
        return false;
    }
    if pos > 0 {
        let prev = bytes[pos - 1];
        if prev.is_ascii_alphanumeric() || prev == b'-' || prev == b'_' {
            return false;
        }
    }
    bytes[pos..pos + ascii_lower_needle.len()]
        .iter()
        .zip(ascii_lower_needle.iter())
        .all(|(a, b)| a.to_ascii_lowercase() == *b)
}

/// Finds the matching closing `)` for an open `(` at `open_paren_idx`, respecting quoted strings
/// and nested parentheses. Returns the index of `)` or `None` if unclosed.
fn find_matching_css_paren(s: &str, open_paren_idx: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = open_paren_idx + 1;
    let mut depth = 1usize;
    let mut in_single = false;
    let mut in_double = false;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        } else if !in_single && !in_double {
            if b == b'(' {
                depth += 1;
            } else if b == b')' {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

/// Splits a comma-separated CSS argument list (e.g. inside `image-set(...)`) respecting quotes and parens.
fn split_css_top_level_commas(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut depth = 0usize;
    let mut in_single = false;
    let mut in_double = false;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        } else if !in_single && !in_double {
            match b {
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => {
                    parts.push(&s[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    parts.push(&s[start..]);
    parts
}

/// Extracts quoted string literal tokens from a CSS expression (used for bare URLs inside `image-set`).
fn css_entry_has_external_url(entry: &str) -> bool {
    let bytes = entry.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i + 1;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    break;
                }
                i += 1;
            }
            let lit = &entry[start..i.min(bytes.len())];
            if is_external_url(lit) || is_dangerous_uri(lit) {
                return true;
            }
        } else if matches_css_func_at(bytes, i, b"url(") {
            if let Some(close) = find_matching_css_paren(entry, i + 3) {
                let inner = entry[i + 4..close]
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'');
                if is_external_url(inner) || is_dangerous_uri(inner) {
                    return true;
                }
                i = close;
            }
        }
        i += 1;
    }
    false
}

/// Strips CSS comments, decodes CSS backslash escapes, removes external `@import` statements
/// (respecting quotes/parens so `;` inside query strings cannot bypass), neutralizes external
/// `url(...)` and `image-set(...)` / `-webkit-image-set(...)` references in $O(N)$, and
/// neutralizes `</style` breakouts.
fn strip_external_css_urls(css: &str) -> (String, usize) {
    let uncommented = strip_css_comments_scrub(css);
    let decoded = decode_css_escapes(&uncommented, true);
    let mut stripped = 0usize;

    // 1. Single-pass O(N) @import stripping respecting quotes & parens
    let mut after_imports = String::with_capacity(decoded.len());
    let bytes = decoded.as_bytes();
    let mut i = 0usize;
    let mut in_single = false;
    let mut in_double = false;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' && i + 1 < bytes.len() {
            let ch = decoded[i..].chars().next().unwrap_or('\\');
            after_imports.push(ch);
            i += ch.len_utf8();
            if i < bytes.len() {
                let next_ch = decoded[i..].chars().next().unwrap_or(' ');
                after_imports.push(next_ch);
                i += next_ch.len_utf8();
            }
            continue;
        }
        if !in_single
            && !in_double
            && i + 7 <= bytes.len()
            && decoded[i..i + 7].eq_ignore_ascii_case("@import")
            && (i + 7 == bytes.len()
                || !bytes[i + 7].is_ascii_alphanumeric()
                    && bytes[i + 7] != b'-'
                    && bytes[i + 7] != b'_')
        {
            // Scan forward to the terminating `;` outside quotes and parens (or `}` / EOF)
            let mut j = i + 7;
            let mut q_single = false;
            let mut q_double = false;
            let mut paren_depth = 0usize;
            while j < bytes.len() {
                let c = bytes[j];
                if c == b'\\' && j + 1 < bytes.len() {
                    j += 2;
                    continue;
                }
                if c == b'\'' && !q_double {
                    q_single = !q_single;
                } else if c == b'"' && !q_single {
                    q_double = !q_double;
                } else if !q_single && !q_double {
                    match c {
                        b'(' => paren_depth += 1,
                        b')' => paren_depth = paren_depth.saturating_sub(1),
                        b';' if paren_depth == 0 => {
                            j += 1;
                            break;
                        }
                        b'}' if paren_depth == 0 => {
                            break;
                        }
                        _ => {}
                    }
                }
                j += 1;
            }
            let stmt = &decoded[i..j];
            let lower_stmt = stmt.to_ascii_lowercase();
            if lower_stmt.contains("http://")
                || lower_stmt.contains("https://")
                || lower_stmt.contains("//")
                || css_entry_has_external_url(stmt)
            {
                stripped += 1;
                after_imports.push_str("/* offline-stripped-import */");
            } else {
                after_imports.push_str(stmt);
            }
            i = j;
            continue;
        }
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        }
        let ch = decoded[i..].chars().next().unwrap_or(' ');
        after_imports.push(ch);
        i += ch.len_utf8();
    }

    // 2. Single-pass O(N) stripping of external image-set(...) / -webkit-image-set(...) and url(...)
    let mut out = String::with_capacity(after_imports.len());
    let b2 = after_imports.as_bytes();
    let mut k = 0usize;
    in_single = false;
    in_double = false;

    while k < b2.len() {
        let b = b2[k];
        if b == b'\\' && k + 1 < b2.len() {
            let ch = after_imports[k..].chars().next().unwrap_or('\\');
            out.push(ch);
            k += ch.len_utf8();
            if k < b2.len() {
                let next_ch = after_imports[k..].chars().next().unwrap_or(' ');
                out.push(next_ch);
                k += next_ch.len_utf8();
            }
            continue;
        }
        if !in_single && !in_double {
            // Check image-set( or -webkit-image-set(
            let img_set_prefix: Option<&[u8]> = if matches_css_func_at(b2, k, b"-webkit-image-set(")
            {
                Some(b"-webkit-image-set(")
            } else if matches_css_func_at(b2, k, b"image-set(") {
                Some(b"image-set(")
            } else {
                None
            };

            if let Some(prefix) = img_set_prefix {
                let open_idx = k + prefix.len() - 1;
                if let Some(close_idx) = find_matching_css_paren(&after_imports, open_idx) {
                    let inner = &after_imports[open_idx + 1..close_idx];
                    let entries = split_css_top_level_commas(inner);
                    let mut kept = Vec::new();
                    let mut removed_any = false;
                    for entry in entries {
                        let trimmed = entry.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if css_entry_has_external_url(trimmed) {
                            stripped += 1;
                            removed_any = true;
                        } else {
                            kept.push(trimmed);
                        }
                    }
                    if removed_any {
                        if kept.is_empty() {
                            out.push_str("none");
                        } else {
                            out.push_str(&after_imports[k..open_idx + 1]);
                            out.push_str(&kept.join(", "));
                            out.push(')');
                        }
                    } else {
                        out.push_str(&after_imports[k..=close_idx]);
                    }
                    k = close_idx + 1;
                    continue;
                } else {
                    // Unclosed image-set( at EOF: CSS Syntax Level 3 auto-closes EOF, so fail closed.
                    let inner = &after_imports[open_idx + 1..];
                    let entries = split_css_top_level_commas(inner);
                    let has_external = entries
                        .iter()
                        .any(|entry| css_entry_has_external_url(entry.trim()));
                    if has_external {
                        stripped += 1;
                        out.push_str("none");
                    } else {
                        out.push_str(&after_imports[k..]);
                    }
                    k = after_imports.len();
                    continue;
                }
            }

            // Check url(...)
            if matches_css_func_at(b2, k, b"url(") {
                let open_idx = k + 3;
                if let Some(close_idx) = find_matching_css_paren(&after_imports, open_idx) {
                    let raw_inner = &after_imports[open_idx + 1..close_idx];
                    let inner = raw_inner.trim().trim_matches(|c| c == '"' || c == '\'');
                    if is_external_url(inner) || is_dangerous_uri(inner) {
                        stripped += 1;
                        out.push_str("none");
                    } else {
                        out.push_str(&after_imports[k..=close_idx]);
                    }
                    k = close_idx + 1;
                    continue;
                } else {
                    // Unclosed url( at EOF: CSS Syntax Level 3 auto-closes EOF, so fail closed.
                    let raw_inner = &after_imports[open_idx + 1..];
                    let inner = raw_inner
                        .trim()
                        .trim_end_matches(';')
                        .trim()
                        .trim_matches(|c| c == '"' || c == '\'');
                    if is_external_url(inner) || is_dangerous_uri(inner) {
                        stripped += 1;
                        out.push_str("none");
                    } else {
                        out.push_str(&after_imports[k..]);
                    }
                    k = after_imports.len();
                    continue;
                }
            }
        }
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        }
        let ch = after_imports[k..].chars().next().unwrap_or(' ');
        out.push(ch);
        k += ch.len_utf8();
    }

    (neutralize_style_breakout(&out), stripped)
}

/// Parses an HTML `srcset` / `imagesrcset` attribute into individual candidate entries,
/// stripping any external (`http://`, `https://`, `//`) or dangerous (`javascript:`, `vbscript:`)
/// entry while preserving valid local / `data:` URI entries (including base64 commas).
fn sanitize_srcset(raw: &str) -> (Option<String>, usize) {
    let bytes = raw.as_bytes();
    let mut i = 0usize;
    let mut kept: Vec<String> = Vec::new();
    let mut stripped = 0usize;

    while i < bytes.len() {
        // Skip leading whitespace and stray commas
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        let url_start = i;
        let starts_data = raw[url_start..]
            .get(..5)
            .is_some_and(|p| p.eq_ignore_ascii_case("data:"));

        // Read URL token: for data: URIs keep embedded commas until whitespace; otherwise stop at whitespace or trailing comma
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            if !starts_data && bytes[i] == b',' {
                // In srcset, a comma terminates a candidate when followed by whitespace or end, or starts a new candidate
                break;
            }
            i += 1;
        }
        let ended_with_comma = raw[url_start..i].ends_with(',');
        let url_token = raw[url_start..i].trim_end_matches(',');

        // If we stopped on comma (or the token ended with a trailing comma), there is no descriptor
        let mut descriptor = "";
        if ended_with_comma {
            // Trailing comma already consumed; next non-whitespace starts the next candidate.
        } else if i < bytes.len() && bytes[i] == b',' {
            i += 1;
        } else {
            // Skip whitespace before optional descriptor
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let desc_start = i;
            let mut paren_depth = 0usize;
            while i < bytes.len() {
                if bytes[i] == b'(' {
                    paren_depth += 1;
                } else if bytes[i] == b')' {
                    paren_depth = paren_depth.saturating_sub(1);
                } else if bytes[i] == b',' && paren_depth == 0 {
                    break;
                }
                i += 1;
            }
            descriptor = raw[desc_start..i].trim();
            if i < bytes.len() && bytes[i] == b',' {
                i += 1;
            }
        }

        if url_token.is_empty() {
            continue;
        }
        if is_external_url(url_token) || is_dangerous_uri(url_token) {
            stripped += 1;
        } else if descriptor.is_empty() {
            kept.push(url_token.to_string());
        } else {
            kept.push(format!("{url_token} {descriptor}"));
        }
    }

    if kept.is_empty() {
        (None, stripped)
    } else {
        (Some(kept.join(", ")), stripped)
    }
}

/// Counts any remaining external network references across HTML resource-loader attributes
/// (`src`, `href` on non-`<a>`/`<area>`, `xlink:href`, `srcset`, `imagesrcset`, `poster`,
/// `data`, `background`) and CSS (`url(...)`, `@import`, `image-set(...)`) in an emitted HTML bundle.
pub fn count_external_cdn_refs(html: &str) -> usize {
    let lower = html.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut count = 0usize;

    const LOADER_ATTRS: &[&str] = &[
        "src=",
        "href=",
        "xlink:href=",
        "srcset=",
        "imagesrcset=",
        "poster=",
        "data=",
        "background=",
    ];

    for attr in LOADER_ATTRS {
        let mut offset = 0usize;
        while let Some(rel) = lower[offset..].find(attr) {
            let pos = offset + rel;
            offset = pos + attr.len();

            // Must be preceded by whitespace so `data-src=` or `data-href=` does not false-match
            if pos == 0 || !bytes[pos - 1].is_ascii_whitespace() {
                continue;
            }

            // Ensure we are inside an HTML tag `<...>` and inspect the tag name
            let prefix = &lower[..pos];
            let Some(lt) = prefix.rfind('<') else {
                continue;
            };
            if prefix[lt..].contains('>') {
                continue;
            }
            let tag_header = prefix[lt + 1..].trim_start();
            if tag_header.starts_with("!--") || tag_header.starts_with('/') {
                continue;
            }
            let tag_name = tag_header
                .split(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
                .next()
                .unwrap_or("");

            // Skip anchor hyperlinks (`<a href="...">`, `<area href="...">`)
            if (*attr == "href=" || *attr == "xlink:href=")
                && (tag_name == "a" || tag_name == "area")
            {
                continue;
            }

            let after = lower[pos + attr.len()..].trim_start();
            let val = match after.chars().next() {
                Some('"') => after[1..].split('"').next().unwrap_or(""),
                Some('\'') => after[1..].split('\'').next().unwrap_or(""),
                _ => after
                    .split(|c: char| c.is_ascii_whitespace() || c == '>')
                    .next()
                    .unwrap_or(""),
            };

            if *attr == "srcset=" || *attr == "imagesrcset=" {
                let (_, ext_count) = sanitize_srcset(val);
                count += ext_count;
            } else if is_external_url(val) {
                count += 1;
            }
        }
    }

    // Audit inline style="..." attributes and <style>...</style> blocks using full CSS parser
    let mut style_attr_off = 0usize;
    while let Some(rel) = lower[style_attr_off..].find("style=") {
        let pos = style_attr_off + rel;
        style_attr_off = pos + 6;
        if pos == 0 || !bytes[pos - 1].is_ascii_whitespace() {
            continue;
        }
        let prefix = &lower[..pos];
        let Some(lt) = prefix.rfind('<') else {
            continue;
        };
        if prefix[lt..].contains('>') {
            continue;
        }
        let after = &html[pos + 6..].trim_start();
        let val = match after.chars().next() {
            Some('"') => after[1..].split('"').next().unwrap_or(""),
            Some('\'') => after[1..].split('\'').next().unwrap_or(""),
            _ => "",
        };
        if !val.is_empty() {
            let (_, ext_css) = strip_external_css_urls(val);
            count += ext_css;
        }
    }

    let mut style_tag_off = 0usize;
    while let Some(rel) = lower[style_tag_off..].find("<style") {
        let open_pos = style_tag_off + rel;
        let Some(gt_rel) = lower[open_pos..].find('>') else {
            break;
        };
        let body_start = open_pos + gt_rel + 1;
        let body_end = lower[body_start..]
            .find("</style")
            .map(|r| body_start + r)
            .unwrap_or(html.len());
        let (_, ext_css) = strip_external_css_urls(&html[body_start..body_end]);
        count += ext_css;
        style_tag_off = body_end.saturating_add(7).min(html.len());
    }

    count
}

/// Finds the first real opening tag `<tag ...>` (or closing tag `</tag>` when `closing` is true),
/// strictly skipping HTML comments (`<!-- ... -->`) and `<script>`/`<style>` rawtext bodies.
fn find_real_tag(html: &str, tag_lower: &str, closing: bool) -> Option<(usize, usize)> {
    let bytes = html.as_bytes();
    let needle = tag_lower.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'<' {
            // Skip HTML comments <!-- ... -->
            if bytes[i..].starts_with(b"<!--") {
                let end_rel = html[i + 4..].find("-->")?;
                i = i + 4 + end_rel + 3;
                continue;
            }
            let mut j = i + 1;
            let is_close = j < bytes.len() && bytes[j] == b'/';
            if is_close {
                j += 1;
            }
            if is_close == closing
                && j + needle.len() <= bytes.len()
                && bytes[j..j + needle.len()]
                    .iter()
                    .zip(needle.iter())
                    .all(|(a, b)| a.to_ascii_lowercase() == *b)
            {
                let after_name = j + needle.len();
                let boundary_ok = after_name < bytes.len()
                    && (bytes[after_name].is_ascii_whitespace()
                        || bytes[after_name] == b'>'
                        || bytes[after_name] == b'/');
                if boundary_ok {
                    // Find closing `>` respecting quoted attributes
                    let mut k = after_name;
                    let mut in_s = false;
                    let mut in_d = false;
                    while k < bytes.len() {
                        if bytes[k] == b'\'' && !in_d {
                            in_s = !in_s;
                        } else if bytes[k] == b'"' && !in_s {
                            in_d = !in_d;
                        } else if bytes[k] == b'>' && !in_s && !in_d {
                            return Some((i, k));
                        }
                        k += 1;
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn ensure_full_html_document(raw_html: &str, title: &str) -> String {
    let safe_title = escape_html_text_basic(if title.trim().is_empty() {
        "Presentation Export"
    } else {
        title.trim()
    });

    let real_html = find_real_tag(raw_html, "html", false);
    let real_head = find_real_tag(raw_html, "head", false);
    let real_body = find_real_tag(raw_html, "body", false);

    if real_html.is_some() && real_head.is_some() && real_body.is_some() {
        return raw_html.to_string();
    }

    if let Some((_, html_gt)) = real_html {
        let mut doc = raw_html.to_string();
        if real_head.is_none() {
            doc.insert_str(
                html_gt + 1,
                &format!("<head><meta charset=\"utf-8\"><title>{safe_title}</title></head>"),
            );
        }
        if find_real_tag(&doc, "body", false).is_none() {
            let body_insert_start = find_real_tag(&doc, "head", true)
                .map(|(_, gt)| gt + 1)
                .or_else(|| find_real_tag(&doc, "html", false).map(|(_, gt)| gt + 1))
                .unwrap_or(0);
            let body_insert_end = find_real_tag(&doc, "html", true)
                .map(|(lt, _)| lt)
                .unwrap_or(doc.len());
            if body_insert_start <= body_insert_end {
                let inner = doc[body_insert_start..body_insert_end].to_string();
                let mut rebuilt = String::with_capacity(doc.len() + 16);
                rebuilt.push_str(&doc[..body_insert_start]);
                rebuilt.push_str("<body>");
                rebuilt.push_str(&inner);
                rebuilt.push_str("</body>");
                rebuilt.push_str(&doc[body_insert_end..]);
                doc = rebuilt;
            }
        }
        return doc;
    }

    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n<title>{safe_title}</title>\n</head>\n<body>\n{raw_html}\n</body>\n</html>"
    )
}

/// Packages HTML + inline CSS + CAS data-URI assets into a single self-contained offline `.html` file
/// using `lol_html`, guaranteeing `external_cdn_refs_count == 0` (fail-closed), strict SHA-256 CSP,
/// stored-XSS neutralization, 16:9 stageshow keyboard presenter parity, `<svg viewBox>` integrity checks,
/// and `<= 25 MiB` total size limit with incremental CAS amplification protection.
pub fn bundle_offline_html(
    input: &OfflineHtmlInput,
) -> Result<OfflineHtmlBundle, ExportBundlerError> {
    let limit = input
        .max_bytes
        .unwrap_or(MAX_BUNDLE_BYTES)
        .min(MAX_BUNDLE_BYTES);
    if limit == 0 {
        return Err(ExportBundlerError::InvalidInput(
            "max_bytes must be greater than zero".to_string(),
        ));
    }

    let raw_asset_bytes: usize = input.cas_assets.values().map(String::len).sum();
    let inline_css_bytes = input.inline_css.as_deref().map_or(0, str::len);
    let raw_total = input
        .html
        .len()
        .saturating_add(inline_css_bytes)
        .saturating_add(raw_asset_bytes);
    if raw_total > limit {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes: raw_total,
            limit_bytes: limit,
        });
    }

    let doc = ensure_full_html_document(&input.html, &input.title);
    let (sanitized_inline_css, inline_css_stripped) = match input.inline_css.as_deref() {
        Some(css) if !css.trim().is_empty() => {
            let (clean, count) = strip_external_css_urls(css);
            (Some(clean), count)
        }
        _ => (None, 0),
    };

    let enable_presenter = input.enable_presenter;
    let csp_header = build_offline_csp_header(enable_presenter);
    let csp_meta =
        format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{csp_header}\">");

    let mut head_injection = String::new();
    head_injection.push_str(&csp_meta);
    if let Some(css) = sanitized_inline_css {
        head_injection.push_str("\n<style data-offline-inline=\"true\">\n");
        head_injection.push_str(&css);
        head_injection.push_str("\n</style>");
    }
    if enable_presenter {
        head_injection.push_str("\n<style data-stageshow-style=\"true\">");
        head_injection.push_str(STAGESHOW_PRESENTER_CSS);
        head_injection.push_str("</style>");
    }

    let body_injection = if enable_presenter {
        format!("\n<script data-stageshow-runtime=\"true\">{STAGESHOW_PRESENTER_JS}</script>")
    } else {
        String::new()
    };

    let initial_bytes = doc
        .len()
        .saturating_add(head_injection.len())
        .saturating_add(body_injection.len());
    if initial_bytes > limit {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes: initial_bytes,
            limit_bytes: limit,
        });
    }

    let stats = Rc::new(RefCell::new(RewriteStats {
        stripped_external_refs: inline_css_stripped,
        cumulative_bytes: initial_bytes,
        ..RewriteStats::default()
    }));
    if inline_css_stripped > 0 {
        stats.borrow_mut().warnings.push(format!(
            "Stripped {inline_css_stripped} external CSS URL reference(s) from inline stylesheet"
        ));
    }

    let cas_assets = Rc::new(input.cas_assets.clone());
    let style_buf = Rc::new(RefCell::new(String::new()));

    let s_meta = Rc::clone(&stats);
    let s_script = Rc::clone(&stats);
    let s_danger_el = Rc::clone(&stats);
    let s_any = Rc::clone(&stats);
    let s_link = Rc::clone(&stats);
    let s_media = Rc::clone(&stats);
    let s_svg_loader = Rc::clone(&stats);
    let s_style_attr = Rc::clone(&stats);
    let s_style_text = Rc::clone(&stats);
    let s_svg = Rc::clone(&stats);
    let s_slide = Rc::clone(&stats);
    let s_head = Rc::clone(&stats);
    let s_body = Rc::clone(&stats);
    let c_link = Rc::clone(&cas_assets);
    let c_media = Rc::clone(&cas_assets);
    let c_svg_loader = Rc::clone(&cas_assets);
    let style_buf_text = Rc::clone(&style_buf);
    let head_inj_clone = head_injection.clone();
    let body_inj_clone = body_injection.clone();

    let rewrite_result = rewrite_str(
        &doc,
        RewriteStrSettings::new()
            // 1. Strip any existing CSP or meta refresh tags
            .append_element_content_handler(element!("meta[http-equiv]", move |el| {
                if let Some(eq) = el.get_attribute("http-equiv") {
                    let norm = eq.trim().to_ascii_lowercase();
                    if norm == "content-security-policy"
                        || norm == "content-security-policy-report-only"
                        || norm == "refresh"
                    {
                        if norm == "refresh" {
                            let mut st = s_meta.borrow_mut();
                            if let Some(content) = el.get_attribute("content") {
                                let cl = content.to_ascii_lowercase();
                                if cl.contains("http://")
                                    || cl.contains("https://")
                                    || cl.contains("//")
                                {
                                    st.stripped_external_refs += 1;
                                }
                            }
                            st.warnings
                                .push("Removed <meta http-equiv=\"refresh\"> tag".to_string());
                        }
                        el.remove();
                    }
                }
                Ok(())
            }))
            // 2. Strip ALL user <script> tags (external AND inline!) before Stageshow injection
            .append_element_content_handler(element!("script", move |el| {
                let mut st = s_script.borrow_mut();
                if let Some(src) = el.get_attribute("src") {
                    if is_external_url(&src) {
                        st.stripped_external_refs += 1;
                        st.warnings
                            .push(format!("Removed external script reference: {src}"));
                    } else {
                        st.warnings.push("Removed user <script> tag".to_string());
                    }
                } else {
                    st.warnings
                        .push("Removed inline user <script> tag".to_string());
                }
                el.remove();
                Ok(())
            }))
            // 3. Strip active/embedding/base elements completely (<iframe>, <object>, <embed>, <base>, <applet>)
            .append_element_content_handler(element!(
                "iframe, object, embed, base, applet",
                move |el| {
                    let tag = el.tag_name().to_ascii_lowercase();
                    let mut st = s_danger_el.borrow_mut();
                    for attr in ["src", "href", "data"] {
                        if let Some(val) = el.get_attribute(attr) {
                            if is_external_url(&val) {
                                st.stripped_external_refs += 1;
                            }
                        }
                    }
                    st.warnings.push(format!(
                        "Removed unsafe <{tag}> element from offline export"
                    ));
                    el.remove();
                    Ok(())
                }
            ))
            // 4. Strip all on* event handlers, srcdoc, formaction, and javascript:/vbscript:/data:text/html URIs & external background
            .append_element_content_handler(element!("*", move |el| {
                let mut remove_attrs: Vec<String> = Vec::new();
                let mut stripped_ext = 0usize;
                let mut stripped_xss = 0usize;

                for attr in el.attributes() {
                    let name_raw = attr.name();
                    let name_lower = name_raw.to_ascii_lowercase();
                    let val = attr.value();

                    if name_lower.starts_with("on")
                        || name_lower == "srcdoc"
                        || name_lower == "formaction"
                    {
                        remove_attrs.push(name_raw);
                        stripped_xss += 1;
                        continue;
                    }

                    if matches!(
                        name_lower.as_str(),
                        "href"
                            | "xlink:href"
                            | "src"
                            | "action"
                            | "data"
                            | "poster"
                            | "background"
                            | "cite"
                            | "longdesc"
                    ) {
                        if is_dangerous_uri(&val) {
                            remove_attrs.push(name_raw);
                            stripped_xss += 1;
                            continue;
                        }
                        if name_lower == "background" && is_external_url(&val) {
                            remove_attrs.push(name_raw);
                            stripped_ext += 1;
                        }
                    }
                }

                for name in remove_attrs {
                    el.remove_attribute(&name);
                }
                if stripped_xss > 0 || stripped_ext > 0 {
                    let mut st = s_any.borrow_mut();
                    st.stripped_external_refs += stripped_ext;
                    if stripped_xss > 0 {
                        st.warnings.push(
                            "Stripped unsafe event handler or script URI attribute".to_string(),
                        );
                    }
                }
                Ok(())
            }))
            // 5. Handle <link> (href AND imagesrcset)
            .append_element_content_handler(element!("link", move |el| {
                if let Some(imagesrcset) = el.get_attribute("imagesrcset") {
                    let (clean, ext_count) = sanitize_srcset(&imagesrcset);
                    if ext_count > 0 {
                        match clean {
                            Some(v) => {
                                let _ = el.set_attribute("imagesrcset", &v);
                            }
                            None => el.remove_attribute("imagesrcset"),
                        }
                        let mut st = s_link.borrow_mut();
                        st.stripped_external_refs += ext_count;
                        st.warnings
                            .push("Stripped external imagesrcset on <link>".to_string());
                    }
                }
                if let Some(href) = el.get_attribute("href") {
                    let trimmed = href.trim();
                    if let Some(inlined) = c_link.get(trimmed) {
                        if is_valid_data_uri(inlined) {
                            let added = inlined.len().saturating_sub(href.len());
                            s_link.borrow_mut().account_growth(added, limit)?;
                            let _ = el.set_attribute("href", inlined);
                            s_link.borrow_mut().inline_assets_count += 1;
                            return Ok(());
                        }
                    }
                    if is_external_url(trimmed) {
                        let mut st = s_link.borrow_mut();
                        st.stripped_external_refs += 1;
                        st.warnings
                            .push(format!("Removed external link reference: {trimmed}"));
                        el.remove();
                    }
                }
                Ok(())
            }))
            // 6. Media & resource loaders: img, source, video, audio, track, input
            .append_element_content_handler(element!(
                "img, source, video, audio, track, input",
                move |el| {
                    let tag = el.tag_name().to_ascii_lowercase();

                    if let Some(srcset) = el.get_attribute("srcset") {
                        let (clean, ext_count) = sanitize_srcset(&srcset);
                        if ext_count > 0 {
                            match clean {
                                Some(v) => {
                                    let _ = el.set_attribute("srcset", &v);
                                }
                                None => el.remove_attribute("srcset"),
                            }
                            let mut st = s_media.borrow_mut();
                            st.stripped_external_refs += ext_count;
                            st.warnings
                                .push(format!("Stripped external srcset on <{tag}>"));
                        }
                    }

                    if tag == "video" {
                        if let Some(poster) = el.get_attribute("poster") {
                            let trimmed = poster.trim();
                            if let Some(inlined) = c_media.get(trimmed) {
                                if is_valid_data_uri(inlined) {
                                    let added = inlined.len().saturating_sub(poster.len());
                                    s_media.borrow_mut().account_growth(added, limit)?;
                                    let _ = el.set_attribute("poster", inlined);
                                    s_media.borrow_mut().inline_assets_count += 1;
                                } else {
                                    el.remove_attribute("poster");
                                    s_media.borrow_mut().broken_cas_assets_count += 1;
                                }
                            } else if is_external_url(trimmed) {
                                el.remove_attribute("poster");
                                let mut st = s_media.borrow_mut();
                                st.stripped_external_refs += 1;
                                st.warnings.push(format!(
                                    "Neutralized external <video> poster: {trimmed}"
                                ));
                            }
                        }
                    }

                    if let Some(src) = el.get_attribute("src") {
                        let trimmed = src.trim();
                        if let Some(inlined) = c_media.get(trimmed) {
                            if is_valid_data_uri(inlined) {
                                let added = inlined.len().saturating_sub(src.len());
                                s_media.borrow_mut().account_growth(added, limit)?;
                                let _ = el.set_attribute("src", inlined);
                                s_media.borrow_mut().inline_assets_count += 1;
                            } else {
                                let added = OFFLINE_IMG_PLACEHOLDER_DATA_URI
                                    .len()
                                    .saturating_sub(src.len());
                                s_media.borrow_mut().account_growth(added, limit)?;
                                let _ = el.set_attribute("src", OFFLINE_IMG_PLACEHOLDER_DATA_URI);
                                let mut st = s_media.borrow_mut();
                                st.broken_cas_assets_count += 1;
                                st.warnings
                                    .push(format!("Invalid CAS data-URI for asset: {trimmed}"));
                            }
                        } else if trimmed.to_ascii_lowercase().starts_with("data:") {
                            if is_valid_data_uri(trimmed) {
                                s_media.borrow_mut().inline_assets_count += 1;
                            } else {
                                let mut st = s_media.borrow_mut();
                                st.broken_cas_assets_count += 1;
                                st.warnings
                                    .push(format!("Malformed inline data-URI on <{tag}>"));
                            }
                        } else if trimmed.starts_with("cas://")
                            || trimmed.starts_with("/api/v1/cas/")
                        {
                            let mut st = s_media.borrow_mut();
                            st.broken_cas_assets_count += 1;
                            st.warnings
                                .push(format!("Unresolved CAS blob reference: {trimmed}"));
                            if tag == "img" {
                                let added = OFFLINE_IMG_PLACEHOLDER_DATA_URI
                                    .len()
                                    .saturating_sub(src.len());
                                st.account_growth(added, limit)?;
                                let _ = el.set_attribute("src", OFFLINE_IMG_PLACEHOLDER_DATA_URI);
                            } else {
                                el.remove_attribute("src");
                            }
                        } else if is_external_url(trimmed) {
                            let mut st = s_media.borrow_mut();
                            st.stripped_external_refs += 1;
                            st.warnings
                                .push(format!("Neutralized external <{tag}> src: {trimmed}"));
                            if tag == "img" {
                                let added = OFFLINE_IMG_PLACEHOLDER_DATA_URI
                                    .len()
                                    .saturating_sub(src.len());
                                st.account_growth(added, limit)?;
                                let _ = el.set_attribute("src", OFFLINE_IMG_PLACEHOLDER_DATA_URI);
                                let _ = el.set_attribute("data-offline-stripped", "true");
                            } else {
                                el.remove_attribute("src");
                                let _ = el.set_attribute("data-offline-stripped", "true");
                            }
                        }
                    }
                    Ok(())
                }
            ))
            // 7. SVG external resource loaders: image, use, feImage (both href and xlink:href)
            .append_element_content_handler(element!("image, use, feimage", move |el| {
                let tag = el.tag_name().to_ascii_lowercase();
                for attr_name in ["href", "xlink:href"] {
                    if let Some(val) = el.get_attribute(attr_name) {
                        let trimmed = val.trim();
                        if let Some(inlined) = c_svg_loader.get(trimmed) {
                            if is_valid_data_uri(inlined) {
                                let added = inlined.len().saturating_sub(val.len());
                                s_svg_loader.borrow_mut().account_growth(added, limit)?;
                                let _ = el.set_attribute(attr_name, inlined);
                                s_svg_loader.borrow_mut().inline_assets_count += 1;
                                continue;
                            }
                        }
                        if is_external_url(trimmed) {
                            el.remove_attribute(attr_name);
                            let _ = el.set_attribute("data-offline-stripped", "true");
                            let mut st = s_svg_loader.borrow_mut();
                            st.stripped_external_refs += 1;
                            st.warnings.push(format!(
                                "Neutralized external SVG <{tag}> {attr_name}: {trimmed}"
                            ));
                        }
                    }
                }
                Ok(())
            }))
            // 8. Sanitize inline style="..." attributes
            .append_element_content_handler(element!("[style]", move |el| {
                if let Some(style_val) = el.get_attribute("style") {
                    let (clean, count) = strip_external_css_urls(&style_val);
                    if count > 0 || clean != style_val {
                        let added = clean.len().saturating_sub(style_val.len());
                        s_style_attr.borrow_mut().account_growth(added, limit)?;
                        let _ = el.set_attribute("style", &clean);
                        if count > 0 {
                            let mut st = s_style_attr.borrow_mut();
                            st.stripped_external_refs += count;
                            st.warnings.push(
                                "Stripped external CSS url() from inline style attribute"
                                    .to_string(),
                            );
                        }
                    }
                }
                Ok(())
            }))
            // 9. Sanitize <style> text nodes directly inside lol_html
            .append_element_content_handler(text!("style", move |chunk| {
                let mut buf = style_buf_text.borrow_mut();
                buf.push_str(chunk.as_str());
                if buf.len() > limit {
                    let len = buf.len();
                    s_style_text.borrow_mut().size_exceeded = Some(len);
                    return Err("ASSET_SIZE_EXCEEDED".into());
                }
                if !chunk.last_in_text_node() {
                    chunk.remove();
                    return Ok(());
                }
                let raw_css = std::mem::take(&mut *buf);
                let (clean_css, count) = strip_external_css_urls(&raw_css);
                let added = clean_css.len().saturating_sub(raw_css.len());
                s_style_text.borrow_mut().account_growth(added, limit)?;
                if count > 0 {
                    let mut st = s_style_text.borrow_mut();
                    st.stripped_external_refs += count;
                    st.warnings.push(format!(
                        "Stripped {count} external CSS URL/import reference(s) from <style> block"
                    ));
                }
                chunk.replace(&clean_css, ContentType::Html);
                Ok(())
            }))
            // 10. Audit every <svg> for valid viewBox attribute
            .append_element_content_handler(element!("svg", move |el| {
                let mut st = s_svg.borrow_mut();
                st.svg_count += 1;
                let vb = el
                    .get_attribute("viewBox")
                    .or_else(|| el.get_attribute("viewbox"));
                match vb {
                    Some(v) if is_valid_svg_viewbox(&v) => {}
                    _ => {
                        st.broken_svg_count += 1;
                        st.warnings
                            .push("Detected <svg> missing valid viewBox attribute".to_string());
                    }
                }
                Ok(())
            }))
            // 11. Count slides
            .append_element_content_handler(element!(
                "section[data-slide], .slide, main > section, body > section",
                move |_| {
                    s_slide.borrow_mut().slide_count += 1;
                    Ok(())
                }
            ))
            // 12. Inject offline CSP & presenter styles into the first real <head>
            .append_element_content_handler(element!("head", move |el| {
                let mut st = s_head.borrow_mut();
                if !st.head_injected {
                    st.head_injected = true;
                    el.prepend(&head_inj_clone, ContentType::Html);
                }
                Ok(())
            }))
            // 13. Inject keyboard presenter script at end of the first real <body>
            .append_element_content_handler(element!("body", move |el| {
                let mut st = s_body.borrow_mut();
                if !st.body_injected {
                    st.body_injected = true;
                    if !body_inj_clone.is_empty() {
                        el.append(&body_inj_clone, ContentType::Html);
                    }
                }
                Ok(())
            })),
    );

    if let Some(actual_bytes) = stats.borrow().size_exceeded {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes,
            limit_bytes: limit,
        });
    }

    let mut final_html =
        rewrite_result.map_err(|e| ExportBundlerError::HtmlRewrite(e.to_string()))?;

    // Guarantee <head> and <body> injections fired even on malformed/comment-spoofed inputs
    let (head_injected, body_injected) = {
        let st = stats.borrow();
        (st.head_injected, st.body_injected)
    };
    if !head_injected || !body_injected {
        let safe_title = escape_html_text_basic(if input.title.trim().is_empty() {
            "Presentation Export"
        } else {
            input.title.trim()
        });
        if !head_injected && !body_injected {
            final_html = format!(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n{head_injection}\n<meta charset=\"utf-8\">\n<title>{safe_title}</title>\n</head>\n<body>\n{final_html}{body_injection}\n</body>\n</html>"
            );
        } else if !head_injected {
            final_html = format!(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n{head_injection}\n<meta charset=\"utf-8\">\n<title>{safe_title}</title>\n</head>\n{final_html}\n</html>"
            );
        } else if !body_injected {
            final_html.push_str(&format!("\n<body>{body_injection}</body>"));
        }
    }

    if final_html.len() > limit {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes: final_html.len(),
            limit_bytes: limit,
        });
    }

    // Fail-closed external CDN audit
    let remaining_external = count_external_cdn_refs(&final_html);
    if remaining_external > 0 {
        return Err(ExportBundlerError::ExternalCdnLeak {
            count: remaining_external,
        });
    }

    let final_stats = stats.borrow();
    let sha256 = sha256_hex(final_html.as_bytes());
    let file_name = format!(
        "{}-offline.html",
        slugify_filename(&input.title, "presentation")
    );
    let slide_count = final_stats.slide_count.max(1);

    Ok(OfflineHtmlBundle {
        file_name,
        byte_size: final_html.len(),
        html_content: final_html,
        sha256,
        external_cdn_refs_count: 0,
        stripped_external_refs_count: final_stats.stripped_external_refs,
        presenter_enabled: enable_presenter,
        slide_count,
        svg_count: final_stats.svg_count,
        broken_svg_count: final_stats.broken_svg_count,
        inline_assets_count: final_stats.inline_assets_count,
        broken_cas_assets_count: final_stats.broken_cas_assets_count,
        csp_header,
        warnings: final_stats.warnings.clone(),
    })
}

fn strip_html_tags_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_style_or_script = false;
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if !in_tag && (lower[i..].starts_with("<style") || lower[i..].starts_with("<script")) {
            in_style_or_script = true;
            in_tag = true;
        } else if in_style_or_script && lower[i..].starts_with("</style>") {
            in_style_or_script = false;
            i += "</style>".len();
            continue;
        } else if in_style_or_script && lower[i..].starts_with("</script>") {
            in_style_or_script = false;
            i += "</script>".len();
            continue;
        }
        let ch = html[i..].chars().next().unwrap_or(' ');
        let ch_len = ch.len_utf8();
        if ch == '<' {
            in_tag = true;
            out.push(' ');
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag && !in_style_or_script {
            out.push(ch);
        }
        i += ch_len;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_slides_from_html(html: &str, fallback_title: &str) -> Vec<VectorPdfSlide> {
    let mut slides = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut offset = 0usize;

    while let Some(start_rel) = lower[offset..].find("<section") {
        let start = offset + start_rel;
        let Some(end_rel) = lower[start..].find("</section>") else {
            break;
        };
        let end = start + end_rel + "</section>".len();
        let chunk = &html[start..end];
        let plain = strip_html_tags_to_text(chunk);
        if !plain.is_empty() {
            let mut sentences: Vec<String> = plain
                .split(". ")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let heading = if sentences.is_empty() {
                format!("Slide {}", slides.len() + 1)
            } else {
                sentences.remove(0)
            };
            slides.push(VectorPdfSlide {
                heading,
                body_lines: sentences,
                footer: Some(format!("{fallback_title} — Slide {}", slides.len() + 1)),
            });
        }
        offset = end;
    }

    if slides.is_empty() {
        let plain = strip_html_tags_to_text(html);
        let body = if plain.is_empty() {
            vec!["Generated by Cowok-Design Vector PDF Exporter".to_string()]
        } else {
            wrap_ascii_lines(&plain, 88)
        };
        slides.push(VectorPdfSlide {
            heading: if fallback_title.trim().is_empty() {
                "Presentation Deck".to_string()
            } else {
                fallback_title.trim().to_string()
            },
            body_lines: body,
            footer: Some("16:9 Vector Slide 1 / 1".to_string()),
        });
    }
    slides
}

fn wrap_ascii_lines(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_chars = 0usize;
    for word in text.split_whitespace() {
        let word_chars = word.chars().count();
        if current_chars + word_chars + 1 > max_chars && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
            current_chars = word_chars;
        } else {
            if !current.is_empty() {
                current.push(' ');
                current_chars += 1;
            }
            current.push_str(word);
            current_chars += word_chars;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Transliterates Vietnamese and common Latin diacritics cleanly into printable ASCII
/// and escapes PDF literal-string metacharacters (`\`, `(`, `)`).
fn transliterate_latin_to_ascii(ch: char) -> Option<&'static str> {
    match ch {
        // Vietnamese + Latin lowercase a
        'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ' | 'ắ' | 'ặ'
        | 'ẳ' | 'ẵ' | 'ä' | 'å' | 'ā' => Some("a"),
        // Vietnamese + Latin uppercase A
        'À' | 'Á' | 'Ạ' | 'Ả' | 'Ã' | 'Â' | 'Ầ' | 'Ấ' | 'Ậ' | 'Ẩ' | 'Ẫ' | 'Ă' | 'Ằ' | 'Ắ' | 'Ặ'
        | 'Ẳ' | 'Ẵ' | 'Ä' | 'Å' | 'Ā' => Some("A"),
        // Vietnamese + Latin lowercase e
        'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' | 'ë' | 'ē' => {
            Some("e")
        }
        // Vietnamese + Latin uppercase E
        'È' | 'É' | 'Ẹ' | 'Ẻ' | 'Ẽ' | 'Ê' | 'Ề' | 'Ế' | 'Ệ' | 'Ể' | 'Ễ' | 'Ë' | 'Ē' => {
            Some("E")
        }
        // Vietnamese + Latin lowercase i
        'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' | 'î' | 'ï' | 'ī' => Some("i"),
        // Vietnamese + Latin uppercase I
        'Ì' | 'Í' | 'Ị' | 'Ỉ' | 'Ĩ' | 'Î' | 'Ï' | 'Ī' => Some("I"),
        // Vietnamese + Latin lowercase o
        'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ' | 'ớ' | 'ợ'
        | 'ở' | 'ỡ' | 'ö' | 'ø' | 'ō' => Some("o"),
        // Vietnamese + Latin uppercase O
        'Ò' | 'Ó' | 'Ọ' | 'Ỏ' | 'Õ' | 'Ô' | 'Ồ' | 'Ố' | 'Ộ' | 'Ổ' | 'Ỗ' | 'Ơ' | 'Ờ' | 'Ớ' | 'Ợ'
        | 'Ở' | 'Ỡ' | 'Ö' | 'Ø' | 'Ō' => Some("O"),
        // Vietnamese + Latin lowercase u
        'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' | 'û' | 'ü' | 'ū' => {
            Some("u")
        }
        // Vietnamese + Latin uppercase U
        'Ù' | 'Ú' | 'Ụ' | 'Ủ' | 'Ũ' | 'Ư' | 'Ừ' | 'Ứ' | 'Ự' | 'Ử' | 'Ữ' | 'Û' | 'Ü' | 'Ū' => {
            Some("U")
        }
        // Vietnamese + Latin y / Y
        'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' | 'ÿ' => Some("y"),
        'Ỳ' | 'Ý' | 'Ỵ' | 'Ỷ' | 'Ỹ' | 'Ÿ' => Some("Y"),
        // Vietnamese d / D stroke
        'đ' => Some("d"),
        'Đ' => Some("D"),
        // Other common Latin letters & ligatures
        'ç' | 'ć' | 'č' => Some("c"),
        'Ç' | 'Ć' | 'Č' => Some("C"),
        'ñ' | 'ń' => Some("n"),
        'Ñ' | 'Ń' => Some("N"),
        'š' | 'ś' => Some("s"),
        'Š' | 'Ś' => Some("S"),
        'ž' | 'ź' | 'ż' => Some("z"),
        'Ž' | 'Ź' | 'Ż' => Some("Z"),
        'ł' => Some("l"),
        'Ł' => Some("L"),
        'ß' => Some("ss"),
        'æ' => Some("ae"),
        'Æ' => Some("AE"),
        'œ' => Some("oe"),
        'Œ' => Some("OE"),
        // Typography punctuation
        '—' | '–' | '−' => Some("-"),
        '•' => Some("*"),
        '‘' | '’' | '‚' => Some("'"),
        '“' | '”' | '„' => Some("\""),
        '…' => Some("..."),
        '\u{00A0}' => Some(" "),
        // Strip NFD combining diacritical marks cleanly
        '\u{0300}'..='\u{036F}' => Some(""),
        _ => None,
    }
}

fn pdf_escape_ascii(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            other => {
                if let Some(mapped) = transliterate_latin_to_ascii(other) {
                    out.push_str(mapped);
                } else {
                    out.push('?');
                }
            }
        }
    }
    out
}

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let (chunks, rem) = input.as_chunks::<3>();
    for chunk in chunks {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        out.push(TABLE[(n & 0x3f) as usize] as char);
    }
    match rem.len() {
        1 => {
            let n = (rem[0] as u32) << 16;
            out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let n = ((rem[0] as u32) << 16) | ((rem[1] as u32) << 8);
            out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

/// Builds a valid deterministic 16:9 (`1440 x 810` pt / `1920 x 1080` @ 96dpi) PDF 1.4 vector
/// document containing crisp slide frames and text vectors, enforcing upfront and incremental
/// 25 MiB bundle caps.
pub fn build_vector_pdf(input: &VectorPdfInput) -> Result<VectorPdfBundle, ExportBundlerError> {
    let limit = input
        .max_bytes
        .unwrap_or(MAX_BUNDLE_BYTES)
        .min(MAX_BUNDLE_BYTES);
    if limit == 0 {
        return Err(ExportBundlerError::InvalidInput(
            "max_bytes must be greater than zero".to_string(),
        ));
    }

    // Upfront input-size bound check before cloning or parsing slides
    let slides_raw_bytes: usize = input.slides.iter().fold(0usize, |acc, s| {
        let body_len: usize = s.body_lines.iter().map(String::len).sum();
        acc.saturating_add(s.heading.len())
            .saturating_add(body_len)
            .saturating_add(s.footer.as_deref().map_or(0, str::len))
    });
    let raw_input_bytes = input
        .title
        .len()
        .saturating_add(input.html.as_deref().map_or(0, str::len))
        .saturating_add(slides_raw_bytes);
    if raw_input_bytes > limit {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes: raw_input_bytes,
            limit_bytes: limit,
        });
    }

    let slides = if !input.slides.is_empty() {
        input.slides.clone()
    } else if let Some(html) = input.html.as_deref() {
        extract_slides_from_html(html, &input.title)
    } else {
        vec![VectorPdfSlide {
            heading: if input.title.trim().is_empty() {
                "Presentation Deck".to_string()
            } else {
                input.title.trim().to_string()
            },
            body_lines: vec!["16:9 Vector PDF slide export".to_string()],
            footer: Some("Slide 1 / 1".to_string()),
        }]
    };

    let page_count = slides.len();
    // Guard against huge empty-slide arrays before allocating object vectors
    let min_bytes_per_page = 320usize;
    let min_estimated = page_count.saturating_mul(min_bytes_per_page);
    if min_estimated > limit {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes: min_estimated,
            limit_bytes: limit,
        });
    }

    let mut objects: Vec<Vec<u8>> = Vec::with_capacity(4 + page_count * 2);
    let mut cumulative_obj_bytes = 64usize;

    // Obj 1: Catalog
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());

    // Obj 2: Pages (Kids start at obj 5, step 2: 5, 7, 9, ...)
    let kids: String = (0..page_count)
        .map(|i| format!("{} 0 R", 5 + i * 2))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push(format!("<< /Type /Pages /Kids [ {kids} ] /Count {page_count} >>").into_bytes());

    // Obj 3: Helvetica Regular
    objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec());
    // Obj 4: Helvetica Bold
    objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".to_vec());

    for obj in &objects {
        cumulative_obj_bytes = cumulative_obj_bytes.saturating_add(obj.len() + 24);
    }

    for (idx, slide) in slides.iter().enumerate() {
        let content_obj_num = 6 + idx * 2;
        let page_dict = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
            VECTOR_PDF_WIDTH_PT as u32,
            VECTOR_PDF_HEIGHT_PT as u32,
            content_obj_num
        );
        let page_dict_bytes = page_dict.into_bytes();
        cumulative_obj_bytes = cumulative_obj_bytes.saturating_add(page_dict_bytes.len() + 24);
        objects.push(page_dict_bytes);

        let mut stream = String::new();
        // Clean slide canvas background + border vector + brand accent rule
        stream.push_str("0.98 0.98 0.97 rg 0 0 1440 810 re f\n");
        stream.push_str("0.00 0.34 1.00 rg 72 742 120 6 re f\n");
        stream.push_str("0.85 0.85 0.88 RG 1.5 w 40 40 1360 730 re S\n");
        // Heading text vector
        let safe_heading = pdf_escape_ascii(&slide.heading);
        stream.push_str(&format!(
            "0.03 0.12 0.23 rg BT /F2 36 Tf 72 680 Td ({safe_heading}) Tj ET\n"
        ));
        // Body lines
        let mut y = 610i32;
        for line in &slide.body_lines {
            for wrapped in wrap_ascii_lines(line, 86) {
                if y < 110 {
                    break;
                }
                let safe_line = pdf_escape_ascii(&wrapped);
                stream.push_str(&format!(
                    "0.20 0.25 0.33 rg BT /F1 20 Tf 72 {y} Td ({safe_line}) Tj ET\n"
                ));
                y -= 32;
            }
        }
        // Footer
        let footer_text = slide
            .footer
            .clone()
            .unwrap_or_else(|| format!("{} - Slide {} / {}", input.title, idx + 1, page_count));
        let safe_footer = pdf_escape_ascii(&footer_text);
        stream.push_str(&format!(
            "0.40 0.44 0.52 rg BT /F1 13 Tf 72 64 Td ({safe_footer}) Tj ET\n"
        ));

        let content_obj = format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        );
        let content_obj_bytes = content_obj.into_bytes();
        cumulative_obj_bytes = cumulative_obj_bytes.saturating_add(content_obj_bytes.len() + 24);
        if cumulative_obj_bytes > limit {
            return Err(ExportBundlerError::AssetSizeExceeded {
                actual_bytes: cumulative_obj_bytes,
                limit_bytes: limit,
            });
        }
        objects.push(content_obj_bytes);
    }

    let mut pdf: Vec<u8> = Vec::with_capacity(cumulative_obj_bytes.min(limit));
    pdf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets: Vec<usize> = Vec::with_capacity(objects.len() + 1);
    offsets.push(0);

    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        pdf.extend_from_slice(obj);
        pdf.extend_from_slice(b"\nendobj\n");
        if pdf.len() > limit {
            return Err(ExportBundlerError::AssetSizeExceeded {
                actual_bytes: pdf.len(),
                limit_bytes: limit,
            });
        }
    }

    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );

    if pdf.len() > limit {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes: pdf.len(),
            limit_bytes: limit,
        });
    }

    let sha256 = sha256_hex(&pdf);
    let pdf_base64 = encode_base64(&pdf);
    let file_name = format!(
        "{}-16x9.pdf",
        slugify_filename(&input.title, "presentation")
    );

    Ok(VectorPdfBundle {
        file_name,
        pdf_base64,
        sha256,
        byte_size: pdf.len(),
        page_count,
        width_pt: VECTOR_PDF_WIDTH_PT,
        height_pt: VECTOR_PDF_HEIGHT_PT,
        engine: "html_forge_vector".to_string(),
        warnings: Vec::new(),
    })
}

/// Packages structured Developer Handoff artifacts (`SPEC.md`, `DESIGN_TOKENS.css`,
/// `DESIGN_TOKENS.json`, `Screen.tsx`, `flow.e2e.spec.ts`) with deterministic SHA-256 digests,
/// upfront input bounds, and safe JSX escaping.
pub fn build_developer_handoff(
    input: &DeveloperHandoffInput,
) -> Result<DeveloperHandoffBundle, ExportBundlerError> {
    // Upfront input-size bound check before allocating or hashing
    let raw_input_bytes = input
        .project_name
        .len()
        .saturating_add(input.spec_markdown.as_deref().map_or(0, str::len))
        .saturating_add(input.design_tokens_css.as_deref().map_or(0, str::len))
        .saturating_add(input.design_tokens_json.as_deref().map_or(0, str::len))
        .saturating_add(input.screen_tsx.as_deref().map_or(0, str::len))
        .saturating_add(input.html_source.as_deref().map_or(0, str::len))
        .saturating_add(input.e2e_spec_ts.as_deref().map_or(0, str::len));
    if raw_input_bytes > MAX_BUNDLE_BYTES {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes: raw_input_bytes,
            limit_bytes: MAX_BUNDLE_BYTES,
        });
    }

    let project_name = if input.project_name.trim().is_empty() {
        "CowokDesignProject"
    } else {
        input.project_name.trim()
    };
    let slug = slugify_filename(project_name, "project");
    let jsx_project_name = serde_json::to_string(project_name)
        .unwrap_or_else(|_| "\"CowokDesignProject\"".to_string());

    let spec_md = input.spec_markdown.clone().unwrap_or_else(|| {
        format!(
            "# {project_name} — Executable Design Specification\n\n- **Audience:** Developer Handoff\n- **Tokens:** `DESIGN_TOKENS.css` & `DESIGN_TOKENS.json`\n- **Component Entry:** `Screen.tsx`\n- **E2E Verification:** `flow.e2e.spec.ts`\n"
        )
    });

    let tokens_css = input.design_tokens_css.clone().unwrap_or_else(|| {
        ":root {\n  --bg-canvas: #F7F5F0;\n  --bg-surface: #FFFFFF;\n  --text-primary: #081E3A;\n  --action-primary: #0057FF;\n  --border-subtle: #E6E2DB;\n}\n"
            .to_string()
    });

    let tokens_json = input.design_tokens_json.clone().unwrap_or_else(|| {
        serde_json::json!({
            "project": project_name,
            "tokens": {
                "bgCanvas": "var(--bg-canvas)",
                "bgSurface": "var(--bg-surface)",
                "textPrimary": "var(--text-primary)",
                "actionPrimary": "var(--action-primary)",
                "borderSubtle": "var(--border-subtle)"
            }
        })
        .to_string()
    });

    let screen_tsx = input.screen_tsx.clone().unwrap_or_else(|| {
        let preview_comment = input
            .html_source
            .as_deref()
            .map(|h| format!("// Source HTML length: {} bytes", h.len()))
            .unwrap_or_else(|| "// Generated clean React handoff shell".to_string());
        format!(
            "import React from \"react\";\nimport \"./DESIGN_TOKENS.css\";\n\n{preview_comment}\nexport const ExportedScreen: React.FC = () => (\n  <main style={{{{ background: \"var(--bg-canvas)\", color: \"var(--text-primary)\", padding: \"24px\" }}}}>\n    <h1>{{{jsx_project_name}}}</h1>\n  </main>\n);\n\nexport default ExportedScreen;\n"
        )
    });

    let e2e_spec = input.e2e_spec_ts.clone().unwrap_or_else(|| {
        format!(
            "import {{ expect, test }} from \"@playwright/test\";\n\ntest(\"{slug} developer handoff smoke\", async ({{ page }}) => {{\n  await page.goto(\"/\");\n  await expect(page.locator(\"main\")).toBeVisible();\n}});\n"
        )
    });

    let entries = vec![
        ("DESIGN_TOKENS.css", "text/css", tokens_css),
        ("DESIGN_TOKENS.json", "application/json", tokens_json),
        ("SPEC.md", "text/markdown", spec_md),
        ("Screen.tsx", "text/tsx", screen_tsx),
        ("flow.e2e.spec.ts", "text/typescript", e2e_spec),
    ];

    let mut files = Vec::with_capacity(entries.len());
    let mut manifest_hasher = Sha256::new();
    let mut total_bytes = 0usize;

    for (path, mime, content) in entries {
        let byte_size = content.len();
        total_bytes = total_bytes.saturating_add(byte_size);
        if total_bytes > MAX_BUNDLE_BYTES {
            return Err(ExportBundlerError::AssetSizeExceeded {
                actual_bytes: total_bytes,
                limit_bytes: MAX_BUNDLE_BYTES,
            });
        }
        let file_sha = sha256_hex(content.as_bytes());
        manifest_hasher.update(path.as_bytes());
        manifest_hasher.update(b":");
        manifest_hasher.update(file_sha.as_bytes());
        manifest_hasher.update(b"\n");
        files.push(DeveloperHandoffFile {
            path: path.to_string(),
            mime_type: mime.to_string(),
            byte_size,
            sha256: file_sha,
            content,
        });
    }

    let digest = manifest_hasher.finalize();
    let mut sha256 = String::with_capacity(64);
    for b in digest {
        sha256.push_str(&format!("{b:02x}"));
    }

    Ok(DeveloperHandoffBundle {
        file_name: format!("{slug}-developer-handoff.json"),
        sha256,
        byte_size: total_bytes,
        files,
    })
}
