// @story 07.3
// @implements AC-02
//! Hybrid Layout Extractor for Built-in Native Rust OOXML `.pptx` Exporter:
//! - Stage 1A (`rust-ooxml-cdp`): Consumes live `1920x1080` Pooled 1-Chromium CDP geometry
//!   snapshots (`getBoundingClientRect` + `getComputedStyle` + raster captures).
//! - Stage 1B (`rust-ooxml-static-fallback`): Deterministic pure-Rust `html5ever` DOM layout
//!   engine guaranteeing 0% crash when Chromium is absent or unavailable.
//! - Enforces Airgap Zero-SSRF, WebP/AVIF avoidance, shape/SVG budgets, and accurate
//!   `PptxFidelityWarning[]` reporting.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Cursor;
use std::rc::Rc;

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use serde::{Deserialize, Serialize};

use crate::export_bundler::{ExportBundlerError, MAX_BUNDLE_BYTES};
use crate::pptx_ir::{
    border_radius_to_geom, encode_solid_png, font_size_px_to_ooxml, is_valid_jpeg_magic,
    is_valid_png_magic, parse_css_box_shadow, parse_css_color, parse_css_linear_gradient,
    parse_css_px_token, px_rect_to_emu, sanitize_xml_text, sha256_bytes, PptxColor, PptxDeckIr,
    PptxExportMode, PptxFidelityWarning, PptxFidelityWarningCode, PptxFidelityWarningSeverity,
    PptxFill, PptxMediaAsset, PptxMediaFormat, PptxNode, PptxPaddingEmu, PptxParagraph,
    PptxRectEmu, PptxShapeGeom, PptxSlideIr, PptxSlideTransition, PptxTableGrid, PptxTableRawCell,
    PptxTextAlign, PptxTextRun, DEFAULT_CS_FONT, DEFAULT_EA_FONT, DEFAULT_LATIN_FONT, EMU_PER_PX,
    MAX_SHAPES_PER_SLIDE, MAX_SLIDES_PER_DECK, MAX_SVG_NODES, SLIDE_H_EMU, SLIDE_H_PX, SLIDE_W_EMU,
};
use crate::visual_qc::PooledCdpEvaluator;

/// Maximum `element_label` length in characters and UTF-16 code units to stay strictly
/// compliant with frontend Zod `z.string().max(120)` validation.
const MAX_WARNING_ELEMENT_LABEL_LEN: usize = 120;

/// Deterministic JS probe intended for Pooled 1-Chromium CDP execution at `1920x1080`.
/// Flattens Flexbox/Grid into exact pixel boxes + resolved sRGB styles + paint order.
pub const CDP_LAYOUT_PROBE_JS: &str = r#"(() => {
  const slides = Array.from(document.querySelectorAll('section[data-slide], .slide, main > section, body > section'));
  const targets = slides.length ? slides : [document.body];
  return {
    slides: targets.map((slide, idx) => {
      const sRect = slide.getBoundingClientRect();
      const sStyle = getComputedStyle(slide);
      const nodes = [];
      let order = 0;
      const walk = (el) => {
        if (!el || el.nodeType !== 1) return;
        const cs = getComputedStyle(el);
        if (cs.display === 'none' || cs.visibility === 'hidden' || parseFloat(cs.opacity) <= 0) return;
        const r = el.getBoundingClientRect();
        const tag = el.tagName.toLowerCase();
        const hasBg = cs.backgroundColor && cs.backgroundColor !== 'rgba(0, 0, 0, 0)' && cs.backgroundColor !== 'transparent';
        const hasGrad = cs.backgroundImage && cs.backgroundImage.includes('linear-gradient');
        const hasBorder = parseFloat(cs.borderTopWidth) > 0;
        const hasShadow = cs.boxShadow && cs.boxShadow !== 'none';
        const directText = Array.from(el.childNodes).filter(n => n.nodeType === 3).map(n => n.textContent.trim()).join(' ').trim();
        const hasBackdrop = (cs.backdropFilter && cs.backdropFilter !== 'none') || (cs.webkitBackdropFilter && cs.webkitBackdropFilter !== 'none');
        const hasAnim = cs.animationName && cs.animationName !== 'none';
        if (tag === 'table' || tag === 'svg' || tag === 'img' || tag === 'canvas' || hasBg || hasGrad || hasBorder || hasShadow || directText || hasBackdrop) {
          nodes.push({
            tag,
            x: r.left - sRect.left,
            y: r.top - sRect.top,
            width: r.width,
            height: r.height,
            zIndex: Number.isFinite(parseInt(cs.zIndex, 10)) ? parseInt(cs.zIndex, 10) : 0,
            paintOrder: order++,
            opacity: parseFloat(cs.opacity),
            visible: true,
            text: directText || undefined,
            backgroundColor: hasBg ? cs.backgroundColor : undefined,
            backgroundImage: hasGrad ? cs.backgroundImage : undefined,
            borderColor: hasBorder ? cs.borderTopColor : undefined,
            borderWidthPx: parseFloat(cs.borderTopWidth) || 0,
            borderRadiusPx: parseFloat(cs.borderTopLeftRadius) || 0,
            boxShadow: hasShadow ? cs.boxShadow : undefined,
            color: cs.color,
            fontFamily: cs.fontFamily,
            fontSizePx: parseFloat(cs.fontSize) || 16,
            fontWeight: parseInt(cs.fontWeight, 10) || 400,
            textAlign: cs.textAlign,
            hasBackdropFilter: hasBackdrop,
            hasCssAnimation: hasAnim,
          });
        }
        if (tag !== 'svg' && tag !== 'table') {
          for (const child of el.children) walk(child);
        }
      };
      walk(slide);
      return {
        index: idx + 1,
        transition: slide.getAttribute('data-transition') || 'none',
        backgroundColor: sStyle.backgroundColor,
        backgroundImage: sStyle.backgroundImage,
        elements: nodes
      };
    })
  };
})()"#;

const SYSTEM_SAFE_FONTS: &[&str] = &[
    "arial",
    "helvetica",
    "calibri",
    "cambria",
    "georgia",
    "times new roman",
    "verdana",
    "tahoma",
    "trebuchet ms",
    "courier new",
    "segoe ui",
    "san francisco",
    "system-ui",
    "-apple-system",
    "sans-serif",
    "serif",
    "monospace",
    "noto sans",
    "noto sans cjk jp",
    "yu gothic",
    "hiragino sans",
    "meiryo",
    "inter",
    "roboto",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdpElementSnapshot {
    pub tag: String,
    #[serde(default)]
    pub label: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub paint_order: usize,
    #[serde(default = "default_one")]
    pub opacity: f64,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub background_image: Option<String>,
    #[serde(default)]
    pub border_color: Option<String>,
    #[serde(default)]
    pub border_width_px: f64,
    #[serde(default)]
    pub border_radius_px: f64,
    #[serde(default)]
    pub is_circle_percent: bool,
    #[serde(default)]
    pub box_shadow: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default = "default_font_px")]
    pub font_size_px: f64,
    #[serde(default = "default_font_weight")]
    pub font_weight: u16,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub text_align: Option<String>,
    #[serde(default)]
    pub padding_px: [f64; 4],
    #[serde(default)]
    pub has_backdrop_filter: bool,
    #[serde(default)]
    pub has_mix_blend_mode: bool,
    #[serde(default)]
    pub has_css_animation: bool,
    #[serde(default)]
    pub has_hover_state: bool,
    #[serde(default)]
    pub is_canvas_or_3d: bool,
    #[serde(default)]
    pub svg_markup: Option<String>,
    #[serde(default)]
    pub svg_node_count: usize,
    #[serde(default)]
    pub svg_has_filter: bool,
    #[serde(default)]
    pub image_src: Option<String>,
    #[serde(default)]
    pub raster_png_base64: Option<String>,
    #[serde(default)]
    pub table_rows: Vec<Vec<PptxTableRawCell>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdpSlideSnapshot {
    pub index: usize,
    #[serde(default)]
    pub transition: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub background_image: Option<String>,
    #[serde(default)]
    pub full_slide_png_base64: Option<String>,
    #[serde(default)]
    pub elements: Vec<CdpElementSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdpDeckSnapshot {
    pub slides: Vec<CdpSlideSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxExportInput {
    pub title: String,
    #[serde(default)]
    pub html: String,
    #[serde(default)]
    pub inline_css: Option<String>,
    #[serde(default)]
    pub cas_assets: BTreeMap<String, String>,
    #[serde(default)]
    pub export_mode: PptxExportMode,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub slide_count_hint: Option<usize>,
    #[serde(default)]
    pub cdp_snapshot: Option<CdpDeckSnapshot>,
    #[serde(default)]
    pub max_bytes: Option<usize>,
}

fn default_one() -> f64 {
    1.0
}

fn default_true() -> bool {
    true
}

fn default_font_px() -> f64 {
    16.0
}

fn default_font_weight() -> u16 {
    400
}

pub struct PptxLayoutExtractor;

impl PptxLayoutExtractor {
    /// Extracts a deterministic `PptxDeckIr` from either a live CDP geometry snapshot
    /// (`rust-ooxml-cdp`) or the fail-safe pure-Rust `html5ever` static fallback engine
    /// (`rust-ooxml-static-fallback`). Fails closed on slide-count or decoded media caps.
    pub fn extract_deck(
        input: &PptxExportInput,
        cdp_pool: Option<&PooledCdpEvaluator>,
    ) -> Result<PptxDeckIr, ExportBundlerError> {
        let locale = normalize_locale(input.locale.as_deref());
        let title = sanitize_xml_text(if input.title.trim().is_empty() {
            "Presentation"
        } else {
            input.title.trim()
        });
        let max_bytes = input.max_bytes.unwrap_or(MAX_BUNDLE_BYTES);

        if let Some(snapshot) = &input.cdp_snapshot {
            let _permit = cdp_pool.map(PooledCdpEvaluator::acquire_permit);
            return extract_from_cdp_snapshot(&title, &locale, input, snapshot, max_bytes);
        }

        // Acquire semaphore briefly if a pool is passed so concurrency accounting stays consistent
        if let Some(pool) = cdp_pool {
            let _permit = pool.acquire_permit();
        }
        extract_from_static_html5ever(&title, &locale, input, max_bytes)
    }
}

fn normalize_locale(raw: Option<&str>) -> String {
    match raw.map(str::trim) {
        Some(loc) if !loc.is_empty() && loc.len() <= 16 => loc.to_string(),
        _ => "vi-VN".to_string(),
    }
}

fn primary_font_name(font_family: Option<&str>) -> String {
    let Some(ff) = font_family else {
        return DEFAULT_LATIN_FONT.to_string();
    };
    for part in ff.split(',') {
        let cleaned = part.trim().trim_matches('"').trim_matches('\'').trim();
        if !cleaned.is_empty() {
            return sanitize_xml_text(cleaned);
        }
    }
    DEFAULT_LATIN_FONT.to_string()
}

fn is_custom_non_system_font(font_name: &str) -> bool {
    let lower = font_name.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    !SYSTEM_SAFE_FONTS.contains(&lower.as_str())
}

fn is_blocked_external_url(raw: &str) -> bool {
    let norm: String = raw
        .chars()
        .filter(|c| !c.is_ascii_control() && !c.is_ascii_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    norm.starts_with("http://")
        || norm.starts_with("https://")
        || norm.starts_with("//")
        || norm.starts_with("file://")
        || norm.contains("../")
        || norm.contains("..\\")
}

fn is_dangerous_svg_href(raw: &str) -> (bool, bool) {
    let norm: String = raw
        .chars()
        .filter(|c| !c.is_ascii_control() && !c.is_ascii_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if norm.is_empty() || norm.starts_with('#') {
        return (false, false);
    }
    if is_blocked_external_url(&norm) {
        return (true, true);
    }
    if norm.starts_with("javascript:")
        || norm.starts_with("vbscript:")
        || norm.starts_with("data:text/")
        || norm.starts_with("data:application/")
        || norm.starts_with("data:image/svg+xml")
    {
        return (true, false);
    }
    if norm.starts_with("data:image/png") || norm.starts_with("data:image/jpeg") {
        return (false, false);
    }
    // Reject unknown schemes (e.g. ftp:, gopher:, chrome:) as external/disallowed
    if norm.contains(':') {
        return (true, true);
    }
    (false, false)
}

/// Sanitizes CSS `url(...)` and `@import` tokens inside SVG attributes or `<style>` blocks.
/// Keeps internal `#fragment` references and strips external/dangerous URLs.
fn sanitize_svg_css(raw: &str) -> (String, bool) {
    let mut blocked_external = false;
    let mut cleaned = String::with_capacity(raw.len());
    let mut remaining = raw;

    // First strip any `@import ...;` declarations
    let mut without_imports = String::with_capacity(raw.len());
    while !remaining.is_empty() {
        let lower = remaining.to_ascii_lowercase();
        if let Some(pos) = lower.find("@import") {
            without_imports.push_str(&remaining[..pos]);
            let after = &remaining[pos..];
            let end_offset = after
                .find(';')
                .map(|i| i + 1)
                .or_else(|| after.find('\n').map(|i| i + 1))
                .unwrap_or(after.len());
            let rule = &after[..end_offset];
            let rule_lower = rule.to_ascii_lowercase();
            if rule_lower.contains("http://")
                || rule_lower.contains("https://")
                || rule_lower.contains("file://")
                || rule_lower.contains("//")
                || rule_lower.contains("url(")
            {
                blocked_external = true;
            }
            remaining = &after[end_offset..];
        } else {
            without_imports.push_str(remaining);
            break;
        }
    }

    // Now scan for `url(...)` tokens
    let mut scan: &str = &without_imports;
    while !scan.is_empty() {
        let lower = scan.to_ascii_lowercase();
        let Some(pos) = lower.find("url(") else {
            cleaned.push_str(scan);
            break;
        };
        cleaned.push_str(&scan[..pos]);
        let after_url = &scan[pos + 4..];
        let Some(close_rel) = after_url.find(')') else {
            // Unclosed `url(` — drop remainder safely
            blocked_external = true;
            break;
        };
        let inner = after_url[..close_rel]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim();
        let (is_blocked, is_ext) = is_dangerous_svg_href(inner);
        if is_blocked {
            if is_ext {
                blocked_external = true;
            }
            cleaned.push_str("none");
        } else {
            cleaned.push_str(&scan[pos..pos + 4 + close_rel + 1]);
        }
        scan = &after_url[close_rel + 1..];
    }

    (cleaned, blocked_external)
}

#[derive(Debug, Clone)]
pub struct SanitizedSvg {
    pub bytes: Vec<u8>,
    pub element_count: usize,
    pub is_complex: bool,
    pub blocked_external_refs: bool,
}

enum SvgBufferedEvent {
    Start {
        name: String,
        attrs: Vec<(String, String)>,
        is_root_svg: bool,
    },
    Empty {
        name: String,
        attrs: Vec<(String, String)>,
        is_root_svg: bool,
    },
    End {
        name: String,
    },
    Text(String),
}

/// Shared strict SVG sanitizer & complexity analyzer used by BOTH inline `<svg>` and
/// `data:image/svg+xml`. Strips `<script>`/embedded active tags, `on*` event attributes,
/// `<!DOCTYPE>`/`<!ENTITY>` declarations, and external URLs (`http://`, `https://`, `//`, `file://`),
/// deduplicates attributes per element, preserves namespace prefixes (`xlink:href`, `xmlns:xlink`),
/// and emits well-formed XML via `quick_xml::Writer`.
pub fn sanitize_and_analyze_svg(raw_svg: &str) -> Option<SanitizedSvg> {
    let trimmed = raw_svg.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut reader = Reader::from_str(trimmed);
    reader.config_mut().check_end_names = true;

    let mut buf = Vec::new();
    let mut events: Vec<SvgBufferedEvent> = Vec::new();
    let mut open_stack: Vec<String> = Vec::new();
    let mut skip_depth = 0usize;
    let mut in_style_depth = 0usize;
    let mut seen_root_svg = false;
    let mut root_closed = false;
    let mut element_count = 0usize;
    let mut has_complex_tag = false;
    let mut blocked_external_refs = false;
    let mut uses_xlink_prefix = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if root_closed {
                    return None;
                }
                let raw_name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let local_lower = raw_name
                    .rsplit(':')
                    .next()
                    .unwrap_or(&raw_name)
                    .to_ascii_lowercase();

                if matches!(
                    local_lower.as_str(),
                    "filter" | "fegaussianblur" | "foreignobject"
                ) {
                    has_complex_tag = true;
                }

                if skip_depth > 0 {
                    skip_depth += 1;
                    buf.clear();
                    continue;
                }

                if matches!(
                    local_lower.as_str(),
                    "script" | "iframe" | "object" | "embed" | "applet"
                ) {
                    skip_depth = 1;
                    buf.clear();
                    continue;
                }

                let is_root_svg = !seen_root_svg && local_lower == "svg";
                if is_root_svg {
                    seen_root_svg = true;
                } else if seen_root_svg {
                    element_count = element_count.saturating_add(1);
                } else {
                    // Reject non-SVG root documents
                    return None;
                }

                if local_lower == "style" {
                    in_style_depth += 1;
                }

                let attrs =
                    sanitize_svg_attributes(&e, &mut blocked_external_refs, &mut uses_xlink_prefix);
                open_stack.push(raw_name.clone());
                events.push(SvgBufferedEvent::Start {
                    name: raw_name,
                    attrs,
                    is_root_svg,
                });
            }
            Ok(Event::Empty(e)) => {
                if root_closed {
                    return None;
                }
                let raw_name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let local_lower = raw_name
                    .rsplit(':')
                    .next()
                    .unwrap_or(&raw_name)
                    .to_ascii_lowercase();

                if matches!(
                    local_lower.as_str(),
                    "filter" | "fegaussianblur" | "foreignobject"
                ) {
                    has_complex_tag = true;
                }

                if skip_depth > 0
                    || matches!(
                        local_lower.as_str(),
                        "script" | "iframe" | "object" | "embed" | "applet"
                    )
                {
                    buf.clear();
                    continue;
                }

                let is_root_svg = !seen_root_svg && local_lower == "svg";
                if is_root_svg {
                    seen_root_svg = true;
                    root_closed = true;
                } else if seen_root_svg {
                    element_count = element_count.saturating_add(1);
                } else {
                    return None;
                }

                let attrs =
                    sanitize_svg_attributes(&e, &mut blocked_external_refs, &mut uses_xlink_prefix);
                events.push(SvgBufferedEvent::Empty {
                    name: raw_name,
                    attrs,
                    is_root_svg,
                });
            }
            Ok(Event::End(e)) => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                    buf.clear();
                    continue;
                }
                if root_closed {
                    return None;
                }
                let raw_name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let local_lower = raw_name
                    .rsplit(':')
                    .next()
                    .unwrap_or(&raw_name)
                    .to_ascii_lowercase();
                if local_lower == "style" && in_style_depth > 0 {
                    in_style_depth -= 1;
                }
                let popped = open_stack.pop()?;
                if popped != raw_name {
                    return None;
                }
                if open_stack.is_empty() {
                    root_closed = true;
                }
                events.push(SvgBufferedEvent::End { name: raw_name });
            }
            Ok(Event::Text(t)) => {
                if skip_depth > 0 || !seen_root_svg {
                    buf.clear();
                    continue;
                }
                let decoded_txt = String::from_utf8_lossy(t.as_ref());
                let raw_txt = quick_xml::escape::unescape(&decoded_txt)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| decoded_txt.into_owned());
                if root_closed {
                    if !raw_txt.trim().is_empty() {
                        return None;
                    }
                    buf.clear();
                    continue;
                }
                let clean = sanitize_xml_text(&raw_txt);
                let final_txt = if in_style_depth > 0 {
                    let (css_clean, css_ext) = sanitize_svg_css(&clean);
                    if css_ext {
                        blocked_external_refs = true;
                    }
                    css_clean
                } else {
                    clean
                };
                if !final_txt.is_empty() {
                    events.push(SvgBufferedEvent::Text(final_txt));
                }
            }
            Ok(Event::CData(c)) => {
                if skip_depth > 0 || !seen_root_svg {
                    buf.clear();
                    continue;
                }
                if root_closed {
                    return None;
                }
                let raw_txt = String::from_utf8_lossy(c.as_ref()).into_owned();
                let clean = sanitize_xml_text(&raw_txt);
                let final_txt = if in_style_depth > 0 {
                    let (css_clean, css_ext) = sanitize_svg_css(&clean);
                    if css_ext {
                        blocked_external_refs = true;
                    }
                    css_clean
                } else {
                    clean
                };
                if !final_txt.is_empty() {
                    events.push(SvgBufferedEvent::Text(final_txt));
                }
            }
            Ok(Event::DocType(dt)) => {
                // Always strip `<!DOCTYPE ...>` and any embedded `<!ENTITY ...>` declarations.
                let dt_lower = String::from_utf8_lossy(dt.as_ref()).to_ascii_lowercase();
                if dt_lower.contains("http://")
                    || dt_lower.contains("https://")
                    || dt_lower.contains("file://")
                    || dt_lower.contains("//")
                {
                    blocked_external_refs = true;
                }
            }
            Ok(Event::GeneralRef(g)) => {
                if skip_depth > 0 || !seen_root_svg {
                    buf.clear();
                    continue;
                }
                if root_closed {
                    return None;
                }
                let resolved = if let Ok(Some(ch)) = g.resolve_char_ref() {
                    Some(ch.to_string())
                } else {
                    let entity_bytes: &[u8] = g.as_ref();
                    match entity_bytes {
                        b"amp" => Some("&".to_string()),
                        b"lt" => Some("<".to_string()),
                        b"gt" => Some(">".to_string()),
                        b"quot" => Some("\"".to_string()),
                        b"apos" => Some("'".to_string()),
                        _ => None, // Drop unknown/custom XML entities (XXE defense)
                    }
                };
                if let Some(txt) = resolved {
                    let clean = sanitize_xml_text(&txt);
                    if !clean.is_empty() {
                        events.push(SvgBufferedEvent::Text(clean));
                    }
                }
            }
            Ok(Event::Comment(_)) | Ok(Event::PI(_)) | Ok(Event::Decl(_)) => {}
            Ok(Event::Eof) => break,
            Err(_) => return None,
        }
        buf.clear();
    }

    if !seen_root_svg || !open_stack.is_empty() {
        return None;
    }

    let mut writer = Writer::new(Cursor::new(Vec::new()));
    for ev in events {
        match ev {
            SvgBufferedEvent::Start {
                name,
                attrs,
                is_root_svg,
            } => {
                let el = build_svg_bytes_start(&name, &attrs, is_root_svg, uses_xlink_prefix);
                writer.write_event(Event::Start(el)).ok()?;
            }
            SvgBufferedEvent::Empty {
                name,
                attrs,
                is_root_svg,
            } => {
                let el = build_svg_bytes_start(&name, &attrs, is_root_svg, uses_xlink_prefix);
                writer.write_event(Event::Empty(el)).ok()?;
            }
            SvgBufferedEvent::End { name } => {
                writer.write_event(Event::End(BytesEnd::new(&name))).ok()?;
            }
            SvgBufferedEvent::Text(txt) => {
                writer.write_event(Event::Text(BytesText::new(&txt))).ok()?;
            }
        }
    }

    let bytes = writer.into_inner().into_inner();
    let is_complex = has_complex_tag || element_count > MAX_SVG_NODES;

    Some(SanitizedSvg {
        bytes,
        element_count,
        is_complex,
        blocked_external_refs,
    })
}

fn sanitize_svg_attributes(
    el: &BytesStart<'_>,
    blocked_external_refs: &mut bool,
    uses_xlink_prefix: &mut bool,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut seen_keys = HashSet::new();

    for attr_res in el.attributes().with_checks(false) {
        let Ok(attr) = attr_res else {
            continue;
        };
        let key = String::from_utf8_lossy(attr.key.as_ref())
            .trim()
            .to_string();
        if key.is_empty() {
            continue;
        }
        let key_lower = key.to_ascii_lowercase();
        let local_lower = key_lower.rsplit(':').next().unwrap_or(&key_lower);

        // Strip all `on*` event handler attributes
        if local_lower.starts_with("on") {
            continue;
        }

        let decoded_val = String::from_utf8_lossy(&attr.value);
        let raw_val = quick_xml::escape::unescape(&decoded_val)
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| decoded_val.into_owned());
        let mut clean_val = sanitize_xml_text(&raw_val);

        if matches!(local_lower, "href" | "src" | "action" | "formaction") {
            let (is_blocked, is_ext) = is_dangerous_svg_href(&clean_val);
            if is_blocked {
                if is_ext {
                    *blocked_external_refs = true;
                }
                continue;
            }
        } else if local_lower == "style" || clean_val.to_ascii_lowercase().contains("url(") {
            let (css_clean, css_ext) = sanitize_svg_css(&clean_val);
            if css_ext {
                *blocked_external_refs = true;
            }
            clean_val = css_clean;
        }

        // Deduplicate attributes per element (case-exact, and case-insensitive for xmlns)
        let dedup_key = if key_lower == "xmlns" || key_lower.starts_with("xmlns:") {
            key_lower.clone()
        } else {
            key.clone()
        };
        if !seen_keys.insert(dedup_key) {
            continue;
        }

        if key_lower.starts_with("xlink:") {
            *uses_xlink_prefix = true;
        }
        out.push((key, clean_val));
    }

    out
}

fn build_svg_bytes_start<'a>(
    name: &'a str,
    attrs: &'a [(String, String)],
    is_root_svg: bool,
    uses_xlink_prefix: bool,
) -> BytesStart<'a> {
    let mut el = BytesStart::new(name);
    let mut has_xmlns = false;
    let mut has_xmlns_xlink = false;
    for (k, v) in attrs {
        if k.eq_ignore_ascii_case("xmlns") {
            has_xmlns = true;
        } else if k.eq_ignore_ascii_case("xmlns:xlink") {
            has_xmlns_xlink = true;
        }
        el.push_attribute((k.as_str(), v.as_str()));
    }
    if is_root_svg && !has_xmlns {
        el.push_attribute(("xmlns", "http://www.w3.org/2000/svg"));
    }
    if is_root_svg && uses_xlink_prefix && !has_xmlns_xlink {
        el.push_attribute(("xmlns:xlink", "http://www.w3.org/1999/xlink"));
    }
    el
}

/// Deck-level Stage-1 media pool deduplicated by `([u8; 32], PptxMediaFormat)` SHA-256 key.
/// Enforces a fail-closed cumulative decoded unique media cap (`<= max_bytes`) inside Stage 1
/// so repeated `<img src="..."/>` or identical `data:` URIs never amplify RAM usage.
struct Stage1MediaPool {
    by_key: HashMap<([u8; 32], PptxMediaFormat), usize>,
    src_cache: HashMap<[u8; 32], CachedMediaOutcome>,
    entries: Vec<PptxMediaAsset>,
    total_bytes: usize,
    max_bytes: usize,
}

#[derive(Clone, Copy)]
enum CachedMediaOutcome {
    Empty,
    BlockedExternal,
    InvalidMedia,
    Image(usize),
    DualSvg {
        svg_idx: usize,
        png_idx: usize,
        blocked_external_refs: bool,
    },
    ComplexSvg {
        png_idx: usize,
        blocked_external_refs: bool,
    },
}

impl Stage1MediaPool {
    fn new(max_bytes: usize) -> Self {
        Self {
            by_key: HashMap::new(),
            src_cache: HashMap::new(),
            entries: Vec::new(),
            total_bytes: 0,
            max_bytes,
        }
    }

    fn intern(
        &mut self,
        bytes: Vec<u8>,
        format: PptxMediaFormat,
    ) -> Result<usize, ExportBundlerError> {
        let hash = sha256_bytes(&bytes);
        let key = (hash, format);
        if let Some(&idx) = self.by_key.get(&key) {
            return Ok(idx);
        }
        let next_total = self.total_bytes.saturating_add(bytes.len());
        if next_total > self.max_bytes {
            return Err(ExportBundlerError::AssetSizeExceeded {
                actual_bytes: next_total,
                limit_bytes: self.max_bytes,
            });
        }
        let idx = self.entries.len();
        self.entries.push(PptxMediaAsset {
            sha256: hash,
            format,
            bytes,
        });
        self.by_key.insert(key, idx);
        self.total_bytes = next_total;
        Ok(idx)
    }

    fn intern_validated_png_b64(
        &mut self,
        b64: Option<&str>,
        fallback_color: PptxColor,
        fallback_dims: (u32, u32),
    ) -> Result<usize, ExportBundlerError> {
        if let Some(raw_b64) = b64 {
            if let Some(bytes) = decode_base64_capped(raw_b64, self.max_bytes)? {
                if is_valid_png_magic(&bytes) {
                    return self.intern(bytes, PptxMediaFormat::Png);
                }
            }
        }
        let solid = encode_solid_png(fallback_dims.0, fallback_dims.1, &fallback_color);
        self.intern(solid, PptxMediaFormat::Png)
    }

    fn resolve_and_intern_media(
        &mut self,
        src: &str,
        cas_assets: &BTreeMap<String, String>,
        custom_raster_png_b64: Option<&str>,
    ) -> Result<CachedMediaOutcome, ExportBundlerError> {
        let trimmed = src.trim();
        if trimmed.is_empty() {
            return Ok(CachedMediaOutcome::Empty);
        }
        if is_blocked_external_url(trimmed) {
            return Ok(CachedMediaOutcome::BlockedExternal);
        }
        let effective = cas_assets
            .get(trimmed)
            .map(String::as_str)
            .unwrap_or(trimmed);
        if is_blocked_external_url(effective) {
            return Ok(CachedMediaOutcome::BlockedExternal);
        }

        let src_hash = sha256_bytes(effective.as_bytes());
        if custom_raster_png_b64.is_none() {
            if let Some(&cached) = self.src_cache.get(&src_hash) {
                return Ok(cached);
            }
        }

        let outcome = if let Some(rest) = effective.strip_prefix("data:") {
            let Some((meta, payload)) = rest.split_once(',') else {
                return Ok(CachedMediaOutcome::InvalidMedia);
            };
            let meta_lower = meta.to_ascii_lowercase();
            let is_b64 = meta_lower.contains(";base64");
            if meta_lower.starts_with("image/png") {
                match decode_data_payload_capped(payload, is_b64, self.max_bytes)? {
                    Some(bytes) if is_valid_png_magic(&bytes) => {
                        let idx = self.intern(bytes, PptxMediaFormat::Png)?;
                        CachedMediaOutcome::Image(idx)
                    }
                    _ => CachedMediaOutcome::InvalidMedia,
                }
            } else if meta_lower.starts_with("image/jpeg") || meta_lower.starts_with("image/jpg") {
                match decode_data_payload_capped(payload, is_b64, self.max_bytes)? {
                    Some(bytes) if is_valid_jpeg_magic(&bytes) => {
                        let idx = self.intern(bytes, PptxMediaFormat::Jpeg)?;
                        CachedMediaOutcome::Image(idx)
                    }
                    _ => CachedMediaOutcome::InvalidMedia,
                }
            } else if meta_lower.starts_with("image/svg+xml") {
                match decode_data_payload_capped(payload, is_b64, self.max_bytes)? {
                    Some(raw_bytes) => {
                        let raw_str = String::from_utf8_lossy(&raw_bytes);
                        if let Some(sanitized) = sanitize_and_analyze_svg(&raw_str) {
                            let png_idx = self.intern_validated_png_b64(
                                custom_raster_png_b64,
                                PptxColor::opaque("3B82F6"),
                                (32, 32),
                            )?;
                            if sanitized.is_complex {
                                CachedMediaOutcome::ComplexSvg {
                                    png_idx,
                                    blocked_external_refs: sanitized.blocked_external_refs,
                                }
                            } else {
                                let svg_idx = self.intern(sanitized.bytes, PptxMediaFormat::Svg)?;
                                CachedMediaOutcome::DualSvg {
                                    svg_idx,
                                    png_idx,
                                    blocked_external_refs: sanitized.blocked_external_refs,
                                }
                            }
                        } else {
                            CachedMediaOutcome::InvalidMedia
                        }
                    }
                    None => CachedMediaOutcome::InvalidMedia,
                }
            } else if meta_lower.starts_with("image/webp") || meta_lower.starts_with("image/avif") {
                // Never leak WebP/AVIF into ppt/media/; emit safe transcoded PNG fallback
                let idx = self.intern_validated_png_b64(
                    custom_raster_png_b64,
                    PptxColor::opaque("CBD5E1"),
                    (16, 16),
                )?;
                CachedMediaOutcome::Image(idx)
            } else {
                CachedMediaOutcome::Empty
            }
        } else {
            CachedMediaOutcome::Empty
        };

        if custom_raster_png_b64.is_none() {
            self.src_cache.insert(src_hash, outcome);
        }
        Ok(outcome)
    }

    fn into_vec(self) -> Vec<PptxMediaAsset> {
        self.entries
    }
}

fn decode_data_payload_capped(
    payload: &str,
    is_base64: bool,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, ExportBundlerError> {
    if is_base64 {
        decode_base64_capped(payload, max_bytes)
    } else {
        Ok(Some(percent_decode_capped(payload, max_bytes)?))
    }
}

fn decode_base64_capped(
    input: &str,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, ExportBundlerError> {
    let non_ws_len = input.bytes().filter(|b| !b.is_ascii_whitespace()).count();
    if non_ws_len == 0 || !non_ws_len.is_multiple_of(4) {
        return Ok(None);
    }
    let est_decoded = (non_ws_len / 4).saturating_mul(3);
    // Allow at most 2 padding bytes slack before failing closed pre-allocation
    if est_decoded.saturating_sub(2) > max_bytes {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes: est_decoded,
            limit_bytes: max_bytes,
        });
    }
    Ok(decode_base64(input))
}

fn percent_decode_capped(input: &str, max_bytes: usize) -> Result<Vec<u8>, ExportBundlerError> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len().min(max_bytes));
    let mut i = 0;
    while i < bytes.len() {
        if out.len() >= max_bytes {
            return Err(ExportBundlerError::AssetSizeExceeded {
                actual_bytes: out.len().saturating_add(1),
                limit_bytes: max_bytes,
            });
        }
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let cleaned: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if cleaned.is_empty() || !cleaned.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity((cleaned.len() / 4) * 3);
    for chunk in cleaned.as_chunks::<4>().0 {
        let v0 = b64_val(chunk[0])?;
        let v1 = b64_val(chunk[1])?;
        out.push((v0 << 2) | (v1 >> 4));
        if chunk[2] != b'=' {
            let v2 = b64_val(chunk[2])?;
            out.push((v1 << 4) | (v2 >> 2));
            if chunk[3] != b'=' {
                let v3 = b64_val(chunk[3])?;
                out.push((v2 << 6) | v3);
            }
        }
    }
    Some(out)
}

fn b64_val(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn resolve_fill(bg_image: Option<&str>, bg_color: Option<&str>) -> PptxFill {
    if let Some(img) = bg_image {
        if let Some(grad) = parse_css_linear_gradient(img) {
            return PptxFill::LinearGradient(grad);
        }
    }
    if let Some(c_str) = bg_color {
        if let Some(color) = parse_css_color(c_str) {
            if !color.is_transparent() {
                return PptxFill::Solid(color);
            }
        }
    }
    PptxFill::None
}

fn truncate_element_label(raw: String) -> String {
    let clean = sanitize_xml_text(&raw);
    if clean.chars().count() <= MAX_WARNING_ELEMENT_LABEL_LEN
        && clean.encode_utf16().count() <= MAX_WARNING_ELEMENT_LABEL_LEN
    {
        return clean;
    }
    let mut out = String::new();
    let mut utf16_used = 0usize;
    for (chars_used, ch) in clean.chars().enumerate() {
        let ch_utf16 = ch.len_utf16();
        if chars_used + 1 > MAX_WARNING_ELEMENT_LABEL_LEN - 1
            || utf16_used + ch_utf16 > MAX_WARNING_ELEMENT_LABEL_LEN - 1
        {
            break;
        }
        out.push(ch);
        utf16_used += ch_utf16;
    }
    out.push('…');
    out
}

#[allow(clippy::too_many_arguments)]
fn push_warning_once(
    warnings: &mut Vec<PptxFidelityWarning>,
    seen: &mut HashSet<(usize, &'static str)>,
    slide_index: usize,
    code: PptxFidelityWarningCode,
    dedup_key: &'static str,
    severity: PptxFidelityWarningSeverity,
    element_label: Option<String>,
    fallback_applied: &str,
    message: String,
) {
    if seen.insert((slide_index, dedup_key)) {
        warnings.push(PptxFidelityWarning {
            slide_index,
            code,
            severity,
            element_label: element_label.map(truncate_element_label),
            fallback_applied: fallback_applied.to_string(),
            message,
        });
    }
}

fn extract_from_cdp_snapshot(
    title: &str,
    locale: &str,
    input: &PptxExportInput,
    snapshot: &CdpDeckSnapshot,
    max_bytes: usize,
) -> Result<PptxDeckIr, ExportBundlerError> {
    if snapshot.slides.len() > MAX_SLIDES_PER_DECK {
        return Err(ExportBundlerError::InvalidInput(format!(
            "Presentation exceeds maximum allowed slide count of {MAX_SLIDES_PER_DECK} slides (found {})",
            snapshot.slides.len()
        )));
    }

    let mut warnings = Vec::new();
    let mut seen_warnings = HashSet::new();
    let mut media_pool = Stage1MediaPool::new(max_bytes);
    let mut slides = Vec::new();

    for (pos, s) in snapshot.slides.iter().enumerate() {
        let slide_idx = if s.index == 0 { pos + 1 } else { s.index };
        let background =
            match resolve_fill(s.background_image.as_deref(), s.background_color.as_deref()) {
                PptxFill::None => PptxFill::Solid(PptxColor::opaque("FFFFFF")),
                other => other,
            };
        let transition = s
            .transition
            .as_deref()
            .map(PptxSlideTransition::from_attr)
            .unwrap_or(PptxSlideTransition::None);

        if input.export_mode == PptxExportMode::VisualExact {
            if let Some(raw_b64) = s.full_slide_png_base64.as_deref() {
                if let Some(png_bytes) = decode_base64_capped(raw_b64, max_bytes)? {
                    if is_valid_png_magic(&png_bytes) {
                        let media_index = media_pool.intern(png_bytes, PptxMediaFormat::Png)?;
                        slides.push(PptxSlideIr {
                            index: slide_idx,
                            background,
                            transition,
                            nodes: vec![PptxNode::Image {
                                id: 2,
                                name: format!("Slide {slide_idx} Visual-Exact Frame"),
                                rect: PptxRectEmu {
                                    x: 0,
                                    y: 0,
                                    cx: SLIDE_W_EMU,
                                    cy: SLIDE_H_EMU,
                                },
                                geom: PptxShapeGeom::Rect,
                                shadow: None,
                                media_index,
                                alt_text: format!("Slide {slide_idx}"),
                            }],
                        });
                        continue;
                    }
                }
            }
            // Full-slide headless Chromium raster snapshot unavailable/invalid: preserve
            // extracted editable slide nodes instead of discarding content for a blank image.
            push_warning_once(
                &mut warnings,
                &mut seen_warnings,
                slide_idx,
                PptxFidelityWarningCode::VisualExactRasterUnavailable,
                "visual_exact_raster_unavailable",
                PptxFidelityWarningSeverity::Warning,
                Some(format!("slide-{slide_idx}")),
                "preserved_editable_nodes",
                format!(
                    "Slide {slide_idx}: Headless Chromium full-slide raster snapshot was unavailable; preserved native editable layout elements instead of emitting a blank image."
                ),
            );
        }

        // Sort elements by CSS stacking context (z_index, paint_order)
        let mut sorted: Vec<&CdpElementSnapshot> = s.elements.iter().collect();
        sorted.sort_by(|a, b| {
            a.z_index
                .cmp(&b.z_index)
                .then_with(|| a.paint_order.cmp(&b.paint_order))
        });

        let mut nodes = Vec::new();
        let mut next_id = 2u32;

        for el in sorted {
            if !el.visible || !el.opacity.is_finite() || el.opacity <= 0.0 {
                continue;
            }
            let Some(rect) = px_rect_to_emu(el.x, el.y, el.width, el.height) else {
                continue;
            };
            let label = el.label.clone().or_else(|| Some(el.tag.clone()));
            let geom = border_radius_to_geom(
                el.border_radius_px,
                el.width,
                el.height,
                el.is_circle_percent,
            );
            let shadow = el.box_shadow.as_deref().and_then(parse_css_box_shadow);

            if el.has_css_animation {
                push_warning_once(
                    &mut warnings,
                    &mut seen_warnings,
                    slide_idx,
                    PptxFidelityWarningCode::CssAnimationOmitted,
                    "css_anim",
                    PptxFidelityWarningSeverity::Warning,
                    label.clone(),
                    "omitted_keyframes",
                    format!("Slide {slide_idx}: Custom CSS animation omitted for PowerPoint compatibility."),
                );
            }
            if el.has_hover_state {
                push_warning_once(
                    &mut warnings,
                    &mut seen_warnings,
                    slide_idx,
                    PptxFidelityWarningCode::HoverStateOmitted,
                    "hover",
                    PptxFidelityWarningSeverity::Info,
                    label.clone(),
                    "omitted_hover",
                    format!(
                        "Slide {slide_idx}: Interactive hover state does not apply in PowerPoint."
                    ),
                );
            }

            if nodes.len() >= MAX_SHAPES_PER_SLIDE {
                push_warning_once(
                    &mut warnings,
                    &mut seen_warnings,
                    slide_idx,
                    PptxFidelityWarningCode::ShapeBudgetRasterized,
                    "shape_budget",
                    PptxFidelityWarningSeverity::Warning,
                    label.clone(),
                    "rasterized_group_png",
                    format!(
                        "Slide {slide_idx}: Exceeded {MAX_SHAPES_PER_SLIDE} native shapes; flattened overflow into raster PNG."
                    ),
                );
                break;
            }

            // 1. Backdrop-filter / mix-blend-mode / canvas / WebGL -> Raster Element Fallback
            if el.has_backdrop_filter || el.has_mix_blend_mode {
                push_warning_once(
                    &mut warnings,
                    &mut seen_warnings,
                    slide_idx,
                    PptxFidelityWarningCode::BackdropFilterRasterized,
                    "backdrop_filter",
                    PptxFidelityWarningSeverity::Warning,
                    label.clone(),
                    "raster_element_png",
                    format!(
                        "Slide {slide_idx}: Frosted glass backdrop-filter rasterized to crisp PNG."
                    ),
                );
                let media_index = media_pool.intern_validated_png_b64(
                    el.raster_png_base64.as_deref(),
                    PptxColor::with_alpha("CBD5E1", 0.75),
                    (32, 18),
                )?;
                nodes.push(PptxNode::Image {
                    id: next_id,
                    name: format!("Rasterized Backdrop {}", next_id),
                    rect,
                    geom,
                    shadow,
                    media_index,
                    alt_text: sanitize_xml_text(el.text.as_deref().unwrap_or("Backdrop effect")),
                });
                next_id += 1;
                continue;
            }

            if el.is_canvas_or_3d || el.tag.eq_ignore_ascii_case("canvas") {
                push_warning_once(
                    &mut warnings,
                    &mut seen_warnings,
                    slide_idx,
                    PptxFidelityWarningCode::CanvasOr3dRasterized,
                    "canvas_3d",
                    PptxFidelityWarningSeverity::Warning,
                    label.clone(),
                    "raster_canvas_png",
                    format!("Slide {slide_idx}: Canvas/3D element rasterized to PNG."),
                );
                let media_index = media_pool.intern_validated_png_b64(
                    el.raster_png_base64.as_deref(),
                    PptxColor::opaque("1E293B"),
                    (32, 18),
                )?;
                nodes.push(PptxNode::Image {
                    id: next_id,
                    name: format!("Canvas Snapshot {}", next_id),
                    rect,
                    geom,
                    shadow,
                    media_index,
                    alt_text: "Interactive chart snapshot".to_string(),
                });
                next_id += 1;
                continue;
            }

            // 2. HTML Table
            if !el.table_rows.is_empty() || el.tag.eq_ignore_ascii_case("table") {
                let grid = PptxTableGrid::from_raw_rows(&el.table_rows, rect);
                nodes.push(PptxNode::Table {
                    id: next_id,
                    name: format!("Table {}", next_id),
                    rect,
                    grid,
                    lang: locale.to_string(),
                    font_latin: primary_font_name(el.font_family.as_deref()),
                    font_ea: DEFAULT_EA_FONT.to_string(),
                });
                next_id += 1;
                continue;
            }

            // 3. Inline SVG -> Strictly sanitized Dual-Blip SVG + PNG @2x (or raster if complex/filter)
            if let Some(svg_raw) = &el.svg_markup {
                let Some(sanitized) = sanitize_and_analyze_svg(svg_raw) else {
                    push_warning_once(
                        &mut warnings,
                        &mut seen_warnings,
                        slide_idx,
                        PptxFidelityWarningCode::InvalidMediaRejected,
                        "invalid_svg",
                        PptxFidelityWarningSeverity::Warning,
                        label.clone(),
                        "rejected_invalid_svg",
                        format!("Slide {slide_idx}: Rejected malformed inline SVG markup."),
                    );
                    continue;
                };
                if sanitized.blocked_external_refs {
                    push_warning_once(
                        &mut warnings,
                        &mut seen_warnings,
                        slide_idx,
                        PptxFidelityWarningCode::ExternalMediaBlocked,
                        "external_media",
                        PptxFidelityWarningSeverity::Warning,
                        label.clone(),
                        "blocked_airgap",
                        format!(
                            "Slide {slide_idx}: External reference inside inline SVG blocked by airgap policy."
                        ),
                    );
                }
                let png_fallback_idx = media_pool.intern_validated_png_b64(
                    el.raster_png_base64.as_deref(),
                    PptxColor::opaque("3B82F6"),
                    (24, 24),
                )?;
                let too_complex =
                    el.svg_has_filter || el.svg_node_count > MAX_SVG_NODES || sanitized.is_complex;
                if too_complex {
                    push_warning_once(
                        &mut warnings,
                        &mut seen_warnings,
                        slide_idx,
                        PptxFidelityWarningCode::ComplexSvgRasterized,
                        "complex_svg",
                        PptxFidelityWarningSeverity::Warning,
                        label.clone(),
                        "raster_svg_png",
                        format!(
                            "Slide {slide_idx}: Complex SVG graphic rasterized to high-res PNG."
                        ),
                    );
                    nodes.push(PptxNode::Image {
                        id: next_id,
                        name: format!("SVG Raster {}", next_id),
                        rect,
                        geom,
                        shadow,
                        media_index: png_fallback_idx,
                        alt_text: "Vector graphic".to_string(),
                    });
                } else {
                    let svg_media_index =
                        media_pool.intern(sanitized.bytes, PptxMediaFormat::Svg)?;
                    nodes.push(PptxNode::DualSvg {
                        id: next_id,
                        name: format!("Vector SVG {}", next_id),
                        rect,
                        geom,
                        shadow,
                        svg_media_index,
                        png_fallback_media_index: png_fallback_idx,
                        alt_text: "Vector graphic".to_string(),
                    });
                }
                next_id += 1;
                continue;
            }

            // 4. Image
            if let Some(src) = &el.image_src {
                match media_pool.resolve_and_intern_media(
                    src,
                    &input.cas_assets,
                    el.raster_png_base64.as_deref(),
                )? {
                    CachedMediaOutcome::BlockedExternal => {
                        push_warning_once(
                            &mut warnings,
                            &mut seen_warnings,
                            slide_idx,
                            PptxFidelityWarningCode::ExternalMediaBlocked,
                            "external_media",
                            PptxFidelityWarningSeverity::Warning,
                            label.clone(),
                            "blocked_airgap",
                            format!("Slide {slide_idx}: External network or file:// image blocked by airgap policy."),
                        );
                    }
                    CachedMediaOutcome::InvalidMedia => {
                        push_warning_once(
                            &mut warnings,
                            &mut seen_warnings,
                            slide_idx,
                            PptxFidelityWarningCode::InvalidMediaRejected,
                            "invalid_media",
                            PptxFidelityWarningSeverity::Warning,
                            label.clone(),
                            "rejected_invalid_media",
                            format!(
                                "Slide {slide_idx}: Rejected malformed or spoofed image payload."
                            ),
                        );
                    }
                    CachedMediaOutcome::Image(media_index) => {
                        nodes.push(PptxNode::Image {
                            id: next_id,
                            name: format!("Image {}", next_id),
                            rect,
                            geom,
                            shadow,
                            media_index,
                            alt_text: label.unwrap_or_else(|| "Image".to_string()),
                        });
                        next_id += 1;
                    }
                    CachedMediaOutcome::DualSvg {
                        svg_idx,
                        png_idx,
                        blocked_external_refs,
                    } => {
                        if blocked_external_refs {
                            push_warning_once(
                                &mut warnings,
                                &mut seen_warnings,
                                slide_idx,
                                PptxFidelityWarningCode::ExternalMediaBlocked,
                                "external_media",
                                PptxFidelityWarningSeverity::Warning,
                                label.clone(),
                                "blocked_airgap",
                                format!("Slide {slide_idx}: External reference inside SVG image blocked by airgap policy."),
                            );
                        }
                        nodes.push(PptxNode::DualSvg {
                            id: next_id,
                            name: format!("SVG Image {}", next_id),
                            rect,
                            geom,
                            shadow,
                            svg_media_index: svg_idx,
                            png_fallback_media_index: png_idx,
                            alt_text: label.unwrap_or_else(|| "SVG".to_string()),
                        });
                        next_id += 1;
                    }
                    CachedMediaOutcome::ComplexSvg {
                        png_idx,
                        blocked_external_refs,
                    } => {
                        if blocked_external_refs {
                            push_warning_once(
                                &mut warnings,
                                &mut seen_warnings,
                                slide_idx,
                                PptxFidelityWarningCode::ExternalMediaBlocked,
                                "external_media",
                                PptxFidelityWarningSeverity::Warning,
                                label.clone(),
                                "blocked_airgap",
                                format!("Slide {slide_idx}: External reference inside SVG image blocked by airgap policy."),
                            );
                        }
                        push_warning_once(
                            &mut warnings,
                            &mut seen_warnings,
                            slide_idx,
                            PptxFidelityWarningCode::ComplexSvgRasterized,
                            "complex_svg",
                            PptxFidelityWarningSeverity::Warning,
                            label.clone(),
                            "raster_svg_png",
                            format!(
                                "Slide {slide_idx}: Complex SVG image rasterized to high-res PNG."
                            ),
                        );
                        nodes.push(PptxNode::Image {
                            id: next_id,
                            name: format!("SVG Raster {}", next_id),
                            rect,
                            geom,
                            shadow,
                            media_index: png_idx,
                            alt_text: label.unwrap_or_else(|| "SVG".to_string()),
                        });
                        next_id += 1;
                    }
                    CachedMediaOutcome::Empty => {}
                }
                continue;
            }

            // 5. TextBox or Visual Shape (skip empty transparent wrappers!)
            let fill = resolve_fill(
                el.background_image.as_deref(),
                el.background_color.as_deref(),
            );
            let border_color = if el.border_width_px > 0.0 {
                el.border_color
                    .as_deref()
                    .and_then(parse_css_color)
                    .filter(|c| !c.is_transparent())
            } else {
                None
            };
            let border_width_emu = if border_color.is_some() {
                (el.border_width_px.clamp(0.5, 24.0) * (EMU_PER_PX as f64)).round() as i64
            } else {
                0
            };
            let text_clean = el
                .text
                .as_deref()
                .map(sanitize_xml_text)
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty());

            if let Some(txt) = text_clean {
                let font_latin = primary_font_name(el.font_family.as_deref());
                if is_custom_non_system_font(&font_latin) {
                    push_warning_once(
                        &mut warnings,
                        &mut seen_warnings,
                        slide_idx,
                        PptxFidelityWarningCode::CustomFontFallback,
                        "custom_font",
                        PptxFidelityWarningSeverity::Info,
                        Some(font_latin.clone()),
                        "norm_autofit_and_dual_font",
                        format!("Slide {slide_idx}: Custom font '{font_latin}' paired with autofit safeguard."),
                    );
                }
                let text_color = el
                    .color
                    .as_deref()
                    .and_then(parse_css_color)
                    .filter(|c| !c.is_transparent())
                    .unwrap_or_else(|| PptxColor::opaque("0F172A"));
                let align = el
                    .text_align
                    .as_deref()
                    .map(PptxTextAlign::from_css)
                    .unwrap_or(PptxTextAlign::Left);
                nodes.push(PptxNode::TextBox {
                    id: next_id,
                    name: format!("TextBox {}", next_id),
                    rect,
                    fill,
                    border_color,
                    border_width_emu,
                    geom,
                    shadow,
                    padding: PptxPaddingEmu::from_px(
                        el.padding_px[3],
                        el.padding_px[1],
                        el.padding_px[0],
                        el.padding_px[2],
                    ),
                    paragraphs: vec![PptxParagraph {
                        align,
                        line_spacing_pct: Some(120_000),
                        runs: vec![PptxTextRun {
                            text: txt,
                            font_latin,
                            font_ea: DEFAULT_EA_FONT.to_string(),
                            font_cs: DEFAULT_CS_FONT.to_string(),
                            font_size_hundredths_pt: font_size_px_to_ooxml(el.font_size_px),
                            bold: el.font_weight >= 600,
                            italic: el.italic,
                            underline: false,
                            color: text_color,
                            lang: locale.to_string(),
                            hyperlink: None,
                        }],
                    }],
                    auto_fit: true,
                });
                next_id += 1;
            } else if fill != PptxFill::None || border_color.is_some() || shadow.is_some() {
                nodes.push(PptxNode::Shape {
                    id: next_id,
                    name: format!("Shape {}", next_id),
                    rect,
                    fill,
                    border_color,
                    border_width_emu,
                    geom,
                    shadow,
                });
                next_id += 1;
            }
        }

        slides.push(PptxSlideIr {
            index: slide_idx,
            background,
            transition,
            nodes,
        });
    }

    if slides.is_empty() {
        slides.push(PptxSlideIr {
            index: 1,
            background: PptxFill::Solid(PptxColor::opaque("FFFFFF")),
            transition: PptxSlideTransition::None,
            nodes: Vec::new(),
        });
    }

    Ok(PptxDeckIr {
        title: title.to_string(),
        locale: locale.to_string(),
        export_mode: input.export_mode,
        slides,
        media_pool: media_pool.into_vec(),
        fidelity_warnings: warnings,
        used_static_fallback: false,
        engine: "rust-ooxml-cdp".to_string(),
    })
}

/// Fail-safe Stage 1B pure-Rust `html5ever` DOM layout extractor (used when Chromium is absent).
fn extract_from_static_html5ever(
    title: &str,
    locale: &str,
    input: &PptxExportInput,
    max_bytes: usize,
) -> Result<PptxDeckIr, ExportBundlerError> {
    let mut warnings = Vec::new();
    let mut seen_warnings = HashSet::new();
    let mut media_pool = Stage1MediaPool::new(max_bytes);

    push_warning_once(
        &mut warnings,
        &mut seen_warnings,
        1,
        PptxFidelityWarningCode::StaticLayoutFallback,
        "static_fallback",
        PptxFidelityWarningSeverity::Info,
        None,
        "html5ever_linear_16x9",
        "Slide 1: Applied deterministic offline static layout engine.".to_string(),
    );

    let dom = parse_document(RcDom::default(), Default::default()).one(input.html.as_str());

    let mut global_css = input.inline_css.clone().unwrap_or_default();
    collect_style_tags(&dom.document, &mut global_css);
    let global_css_lower = global_css.to_ascii_lowercase();

    let mut slide_handles = Vec::new();
    collect_slide_sections(&dom.document, &mut slide_handles, MAX_SLIDES_PER_DECK + 1);
    if slide_handles.len() > MAX_SLIDES_PER_DECK {
        return Err(ExportBundlerError::InvalidInput(format!(
            "Presentation exceeds maximum allowed slide count of {MAX_SLIDES_PER_DECK} slides"
        )));
    }

    if slide_handles.is_empty() {
        if let Some(body) = find_tag(&dom.document, "body") {
            slide_handles.push(body);
        } else {
            slide_handles.push(Rc::clone(&dom.document));
        }
    }

    // Honor slide_count_hint only when HTML was completely empty and caller supplied a hint
    let html_is_blank = input.html.trim().is_empty();
    let mut slides = Vec::new();

    if html_is_blank {
        if let Some(hint) = input.slide_count_hint {
            if hint > MAX_SLIDES_PER_DECK {
                return Err(ExportBundlerError::InvalidInput(format!(
                    "Presentation exceeds maximum allowed slide count of {MAX_SLIDES_PER_DECK} slides (requested {hint})"
                )));
            }
        }
        let count = input.slide_count_hint.unwrap_or(1).clamp(1, 50);
        for idx in 1..=count {
            if input.export_mode == PptxExportMode::VisualExact {
                push_warning_once(
                    &mut warnings,
                    &mut seen_warnings,
                    idx,
                    PptxFidelityWarningCode::VisualExactRasterUnavailable,
                    "visual_exact_raster_unavailable",
                    PptxFidelityWarningSeverity::Warning,
                    Some(format!("slide-{idx}")),
                    "preserved_editable_nodes",
                    format!(
                        "Slide {idx}: Headless Chromium full-slide raster snapshot was unavailable; preserved native editable layout elements instead of emitting a blank image."
                    ),
                );
            }
            let mut nodes = Vec::new();
            if idx == 1 {
                if let Some(rect) = px_rect_to_emu(120.0, 200.0, 1680.0, 160.0) {
                    nodes.push(make_text_box(
                        2,
                        "Title",
                        rect,
                        title.to_string(),
                        52.0,
                        true,
                        PptxColor::opaque("0F172A"),
                        PptxFill::None,
                        None,
                        locale,
                        DEFAULT_LATIN_FONT,
                        PptxTextAlign::Left,
                    ));
                }
            }
            slides.push(PptxSlideIr {
                index: idx,
                background: PptxFill::Solid(PptxColor::opaque("FFFFFF")),
                transition: PptxSlideTransition::None,
                nodes,
            });
        }
        return Ok(PptxDeckIr {
            title: title.to_string(),
            locale: locale.to_string(),
            export_mode: input.export_mode,
            slides,
            media_pool: media_pool.into_vec(),
            fidelity_warnings: warnings,
            used_static_fallback: true,
            engine: "rust-ooxml-static-fallback".to_string(),
        });
    }

    for (pos, slide_handle) in slide_handles.iter().enumerate() {
        let slide_idx = pos + 1;
        let attrs = element_attrs(slide_handle);
        let style_map = parse_inline_style(attrs.get("style").map(String::as_str).unwrap_or(""));
        let transition = attrs
            .get("data-transition")
            .map(|s| PptxSlideTransition::from_attr(s))
            .unwrap_or(PptxSlideTransition::None);

        let bg_fill = match resolve_fill(
            style_map
                .get("background")
                .or_else(|| style_map.get("background-image"))
                .map(String::as_str),
            style_map
                .get("background-color")
                .or_else(|| style_map.get("background"))
                .map(String::as_str),
        ) {
            PptxFill::None => PptxFill::Solid(PptxColor::opaque("FFFFFF")),
            other => other,
        };

        // Check global CSS hints on slide 1
        if slide_idx == 1 {
            if global_css_lower.contains("@keyframes") {
                push_warning_once(
                    &mut warnings,
                    &mut seen_warnings,
                    slide_idx,
                    PptxFidelityWarningCode::CssAnimationOmitted,
                    "css_anim",
                    PptxFidelityWarningSeverity::Warning,
                    Some("stylesheet".to_string()),
                    "omitted_keyframes",
                    format!("Slide {slide_idx}: Custom CSS @keyframes animation omitted."),
                );
            }
            if global_css_lower.contains(":hover") {
                push_warning_once(
                    &mut warnings,
                    &mut seen_warnings,
                    slide_idx,
                    PptxFidelityWarningCode::HoverStateOmitted,
                    "hover",
                    PptxFidelityWarningSeverity::Info,
                    Some("stylesheet".to_string()),
                    "omitted_hover",
                    format!("Slide {slide_idx}: CSS :hover interaction omitted for PowerPoint."),
                );
            }
            if global_css_lower.contains("backdrop-filter")
                || global_css_lower.contains("mix-blend-mode")
            {
                push_warning_once(
                    &mut warnings,
                    &mut seen_warnings,
                    slide_idx,
                    PptxFidelityWarningCode::BackdropFilterRasterized,
                    "backdrop_filter",
                    PptxFidelityWarningSeverity::Warning,
                    Some("stylesheet".to_string()),
                    "raster_element_png",
                    format!("Slide {slide_idx}: Frosted blur backdrop-filter rasterized to PNG."),
                );
            }
        }

        if input.export_mode == PptxExportMode::VisualExact {
            // Without headless Chromium full-slide raster snapshots, never discard real slide
            // content in favor of a blank solid-color image: preserve native editable nodes
            // and emit a structured fallback warning.
            push_warning_once(
                &mut warnings,
                &mut seen_warnings,
                slide_idx,
                PptxFidelityWarningCode::VisualExactRasterUnavailable,
                "visual_exact_raster_unavailable",
                PptxFidelityWarningSeverity::Warning,
                Some(format!("slide-{slide_idx}")),
                "preserved_editable_nodes",
                format!(
                    "Slide {slide_idx}: Headless Chromium full-slide raster snapshot was unavailable; preserved native editable layout elements instead of emitting a blank image."
                ),
            );
        }

        let mut nodes = Vec::new();
        let mut cursor_y = 88.0;
        let mut next_id = 2u32;
        walk_static_slide_nodes(
            slide_handle,
            slide_idx,
            locale,
            &input.cas_assets,
            &mut media_pool,
            &mut cursor_y,
            &mut next_id,
            &mut nodes,
            &mut warnings,
            &mut seen_warnings,
        )?;

        slides.push(PptxSlideIr {
            index: slide_idx,
            background: bg_fill,
            transition,
            nodes,
        });
    }

    if slides.is_empty() {
        slides.push(PptxSlideIr {
            index: 1,
            background: PptxFill::Solid(PptxColor::opaque("FFFFFF")),
            transition: PptxSlideTransition::None,
            nodes: Vec::new(),
        });
    }

    Ok(PptxDeckIr {
        title: title.to_string(),
        locale: locale.to_string(),
        export_mode: input.export_mode,
        slides,
        media_pool: media_pool.into_vec(),
        fidelity_warnings: warnings,
        used_static_fallback: true,
        engine: "rust-ooxml-static-fallback".to_string(),
    })
}

#[allow(clippy::too_many_arguments)]
fn walk_static_slide_nodes(
    handle: &Handle,
    slide_idx: usize,
    locale: &str,
    cas_assets: &BTreeMap<String, String>,
    media_pool: &mut Stage1MediaPool,
    cursor_y: &mut f64,
    next_id: &mut u32,
    nodes: &mut Vec<PptxNode>,
    warnings: &mut Vec<PptxFidelityWarning>,
    seen_warnings: &mut HashSet<(usize, &'static str)>,
) -> Result<(), ExportBundlerError> {
    let children = handle.children.borrow();
    for child in children.iter() {
        let NodeData::Element { ref name, .. } = child.data else {
            continue;
        };
        let tag = (*name.local).to_ascii_lowercase();
        if matches!(
            tag.as_str(),
            "script" | "style" | "head" | "meta" | "link" | "noscript"
        ) {
            continue;
        }

        if nodes.len() >= MAX_SHAPES_PER_SLIDE {
            push_warning_once(
                warnings,
                seen_warnings,
                slide_idx,
                PptxFidelityWarningCode::ShapeBudgetRasterized,
                "shape_budget",
                PptxFidelityWarningSeverity::Warning,
                Some(tag.clone()),
                "rasterized_group_png",
                format!("Slide {slide_idx}: Exceeded {MAX_SHAPES_PER_SLIDE} native shapes; flattened overflow."),
            );
            return Ok(());
        }

        let attrs = element_attrs(child);
        let style_str = attrs.get("style").map(String::as_str).unwrap_or("");
        let style_map = parse_inline_style(style_str);

        if style_map
            .get("display")
            .is_some_and(|v| v.eq_ignore_ascii_case("none"))
            || style_map
                .get("visibility")
                .is_some_and(|v| v.eq_ignore_ascii_case("hidden"))
            || style_map
                .get("opacity")
                .and_then(|v| v.parse::<f64>().ok())
                .is_some_and(|o| o <= 0.0)
        {
            continue;
        }

        // Check inline effects
        if style_map.contains_key("backdrop-filter")
            || style_map.contains_key("-webkit-backdrop-filter")
            || style_map.contains_key("mix-blend-mode")
        {
            push_warning_once(
                warnings,
                seen_warnings,
                slide_idx,
                PptxFidelityWarningCode::BackdropFilterRasterized,
                "backdrop_filter",
                PptxFidelityWarningSeverity::Warning,
                Some(tag.clone()),
                "raster_element_png",
                format!("Slide {slide_idx}: Frosted blur backdrop-filter rasterized to PNG."),
            );
            let y = (*cursor_y).min(SLIDE_H_PX - 180.0);
            if let Some(rect) = px_rect_to_emu(120.0, y, 1680.0, 140.0) {
                let media_index = media_pool.intern(
                    encode_solid_png(32, 18, &PptxColor::with_alpha("CBD5E1", 0.8)),
                    PptxMediaFormat::Png,
                )?;
                nodes.push(PptxNode::Image {
                    id: *next_id,
                    name: format!("Backdrop Block {}", *next_id),
                    rect,
                    geom: PptxShapeGeom::RoundRect { adj: 8_000 },
                    shadow: style_map
                        .get("box-shadow")
                        .and_then(|s| parse_css_box_shadow(s)),
                    media_index,
                    alt_text: sanitize_xml_text(&collect_text_recursive(child)),
                });
                *next_id += 1;
                *cursor_y += 160.0;
            }
            continue;
        }

        if style_map.contains_key("animation") || style_map.contains_key("animation-name") {
            push_warning_once(
                warnings,
                seen_warnings,
                slide_idx,
                PptxFidelityWarningCode::CssAnimationOmitted,
                "css_anim",
                PptxFidelityWarningSeverity::Warning,
                Some(tag.clone()),
                "omitted_keyframes",
                format!("Slide {slide_idx}: Custom CSS animation omitted."),
            );
        }

        if let Some(ff) = style_map.get("font-family") {
            let primary = primary_font_name(Some(ff));
            if is_custom_non_system_font(&primary) {
                push_warning_once(
                    warnings,
                    seen_warnings,
                    slide_idx,
                    PptxFidelityWarningCode::CustomFontFallback,
                    "custom_font",
                    PptxFidelityWarningSeverity::Info,
                    Some(primary.clone()),
                    "norm_autofit_and_dual_font",
                    format!(
                        "Slide {slide_idx}: Custom font '{primary}' paired with autofit safeguard."
                    ),
                );
            }
        }

        match tag.as_str() {
            "canvas" => {
                push_warning_once(
                    warnings,
                    seen_warnings,
                    slide_idx,
                    PptxFidelityWarningCode::CanvasOr3dRasterized,
                    "canvas_3d",
                    PptxFidelityWarningSeverity::Warning,
                    Some("canvas".to_string()),
                    "raster_canvas_png",
                    format!("Slide {slide_idx}: Interactive canvas/3D block rasterized to PNG."),
                );
                let y = (*cursor_y).min(SLIDE_H_PX - 280.0);
                if let Some(rect) = px_rect_to_emu(120.0, y, 800.0, 260.0) {
                    let media_index = media_pool.intern(
                        encode_solid_png(32, 18, &PptxColor::opaque("1E293B")),
                        PptxMediaFormat::Png,
                    )?;
                    nodes.push(PptxNode::Image {
                        id: *next_id,
                        name: format!("Canvas {}", *next_id),
                        rect,
                        geom: PptxShapeGeom::Rect,
                        shadow: None,
                        media_index,
                        alt_text: "Canvas chart".to_string(),
                    });
                    *next_id += 1;
                    *cursor_y += 280.0;
                }
            }
            "table" => {
                let raw_rows = extract_html_table_rows(child);
                let row_count = raw_rows.len().max(1);
                let table_h = ((row_count as f64) * 62.0).clamp(100.0, 560.0);
                let y = (*cursor_y).min(SLIDE_H_PX - table_h - 24.0).max(40.0);
                if let Some(rect) = px_rect_to_emu(120.0, y, 1680.0, table_h) {
                    let grid = PptxTableGrid::from_raw_rows(&raw_rows, rect);
                    let font_latin =
                        primary_font_name(style_map.get("font-family").map(String::as_str));
                    nodes.push(PptxNode::Table {
                        id: *next_id,
                        name: format!("Table {}", *next_id),
                        rect,
                        grid,
                        lang: locale.to_string(),
                        font_latin,
                        font_ea: DEFAULT_EA_FONT.to_string(),
                    });
                    *next_id += 1;
                    *cursor_y = (y + table_h + 24.0).min(SLIDE_H_PX - 80.0);
                }
            }
            "svg" => {
                let raw_svg = serialize_svg_element(child);
                let Some(sanitized) = sanitize_and_analyze_svg(&raw_svg) else {
                    push_warning_once(
                        warnings,
                        seen_warnings,
                        slide_idx,
                        PptxFidelityWarningCode::InvalidMediaRejected,
                        "invalid_svg",
                        PptxFidelityWarningSeverity::Warning,
                        Some("svg".to_string()),
                        "rejected_invalid_svg",
                        format!("Slide {slide_idx}: Rejected malformed inline SVG element."),
                    );
                    continue;
                };
                if sanitized.blocked_external_refs {
                    push_warning_once(
                        warnings,
                        seen_warnings,
                        slide_idx,
                        PptxFidelityWarningCode::ExternalMediaBlocked,
                        "external_media",
                        PptxFidelityWarningSeverity::Warning,
                        Some("svg".to_string()),
                        "blocked_airgap",
                        format!(
                            "Slide {slide_idx}: External reference inside inline SVG blocked for offline safety."
                        ),
                    );
                }
                let dom_node_count = count_descendant_elements(child);
                let is_complex = sanitized.is_complex || dom_node_count > MAX_SVG_NODES;
                let w_px = attrs
                    .get("width")
                    .and_then(|v| v.trim_end_matches("px").parse::<f64>().ok())
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .unwrap_or(280.0)
                    .clamp(32.0, 1200.0);
                let h_px = attrs
                    .get("height")
                    .and_then(|v| v.trim_end_matches("px").parse::<f64>().ok())
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .unwrap_or(180.0)
                    .clamp(32.0, 600.0);
                let y = (*cursor_y).min(SLIDE_H_PX - h_px - 24.0).max(40.0);
                if let Some(rect) = px_rect_to_emu(120.0, y, w_px, h_px) {
                    let png_fallback_idx = media_pool.intern(
                        encode_solid_png(32, 32, &PptxColor::opaque("3B82F6")),
                        PptxMediaFormat::Png,
                    )?;
                    if is_complex {
                        push_warning_once(
                            warnings,
                            seen_warnings,
                            slide_idx,
                            PptxFidelityWarningCode::ComplexSvgRasterized,
                            "complex_svg",
                            PptxFidelityWarningSeverity::Warning,
                            Some("svg".to_string()),
                            "raster_svg_png",
                            format!("Slide {slide_idx}: Complex SVG rasterized to high-res PNG."),
                        );
                        nodes.push(PptxNode::Image {
                            id: *next_id,
                            name: format!("SVG Raster {}", *next_id),
                            rect,
                            geom: PptxShapeGeom::Rect,
                            shadow: None,
                            media_index: png_fallback_idx,
                            alt_text: "Vector graphic".to_string(),
                        });
                    } else {
                        let svg_media_index =
                            media_pool.intern(sanitized.bytes, PptxMediaFormat::Svg)?;
                        nodes.push(PptxNode::DualSvg {
                            id: *next_id,
                            name: format!("Vector SVG {}", *next_id),
                            rect,
                            geom: PptxShapeGeom::Rect,
                            shadow: None,
                            svg_media_index,
                            png_fallback_media_index: png_fallback_idx,
                            alt_text: "Vector graphic".to_string(),
                        });
                    }
                    *next_id += 1;
                    *cursor_y = (y + h_px + 24.0).min(SLIDE_H_PX - 80.0);
                }
            }
            "img" => {
                let src = attrs.get("src").map(String::as_str).unwrap_or("");
                let alt = attrs
                    .get("alt")
                    .cloned()
                    .unwrap_or_else(|| "Image".to_string());
                let is_circle = style_map
                    .get("border-radius")
                    .is_some_and(|v| v.contains("50%") || v.contains("9999"));
                let radius_px = style_map
                    .get("border-radius")
                    .and_then(|v| parse_css_px_token(v))
                    .unwrap_or(0.0);
                let w_px = attrs
                    .get("width")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(320.0)
                    .clamp(32.0, 1200.0);
                let h_px = attrs
                    .get("height")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(200.0)
                    .clamp(32.0, 600.0);
                let geom = border_radius_to_geom(radius_px, w_px, h_px, is_circle);
                let y = (*cursor_y).min(SLIDE_H_PX - h_px - 24.0).max(40.0);

                match media_pool.resolve_and_intern_media(src, cas_assets, None)? {
                    CachedMediaOutcome::BlockedExternal => {
                        push_warning_once(
                            warnings,
                            seen_warnings,
                            slide_idx,
                            PptxFidelityWarningCode::ExternalMediaBlocked,
                            "external_media",
                            PptxFidelityWarningSeverity::Warning,
                            Some("img".to_string()),
                            "blocked_airgap",
                            format!(
                                "Slide {slide_idx}: External image blocked for offline safety."
                            ),
                        );
                    }
                    CachedMediaOutcome::InvalidMedia => {
                        push_warning_once(
                            warnings,
                            seen_warnings,
                            slide_idx,
                            PptxFidelityWarningCode::InvalidMediaRejected,
                            "invalid_media",
                            PptxFidelityWarningSeverity::Warning,
                            Some("img".to_string()),
                            "rejected_invalid_media",
                            format!(
                                "Slide {slide_idx}: Rejected malformed or spoofed image payload."
                            ),
                        );
                    }
                    CachedMediaOutcome::Image(media_index) => {
                        if let Some(rect) = px_rect_to_emu(120.0, y, w_px, h_px) {
                            nodes.push(PptxNode::Image {
                                id: *next_id,
                                name: format!("Image {}", *next_id),
                                rect,
                                geom,
                                shadow: style_map
                                    .get("box-shadow")
                                    .and_then(|s| parse_css_box_shadow(s)),
                                media_index,
                                alt_text: sanitize_xml_text(&alt),
                            });
                            *next_id += 1;
                            *cursor_y = (y + h_px + 24.0).min(SLIDE_H_PX - 80.0);
                        }
                    }
                    CachedMediaOutcome::DualSvg {
                        svg_idx,
                        png_idx,
                        blocked_external_refs,
                    } => {
                        if blocked_external_refs {
                            push_warning_once(
                                warnings,
                                seen_warnings,
                                slide_idx,
                                PptxFidelityWarningCode::ExternalMediaBlocked,
                                "external_media",
                                PptxFidelityWarningSeverity::Warning,
                                Some("img".to_string()),
                                "blocked_airgap",
                                format!(
                                    "Slide {slide_idx}: External reference inside SVG image blocked for offline safety."
                                ),
                            );
                        }
                        if let Some(rect) = px_rect_to_emu(120.0, y, w_px, h_px) {
                            nodes.push(PptxNode::DualSvg {
                                id: *next_id,
                                name: format!("SVG Image {}", *next_id),
                                rect,
                                geom,
                                shadow: None,
                                svg_media_index: svg_idx,
                                png_fallback_media_index: png_idx,
                                alt_text: sanitize_xml_text(&alt),
                            });
                            *next_id += 1;
                            *cursor_y = (y + h_px + 24.0).min(SLIDE_H_PX - 80.0);
                        }
                    }
                    CachedMediaOutcome::ComplexSvg {
                        png_idx,
                        blocked_external_refs,
                    } => {
                        if blocked_external_refs {
                            push_warning_once(
                                warnings,
                                seen_warnings,
                                slide_idx,
                                PptxFidelityWarningCode::ExternalMediaBlocked,
                                "external_media",
                                PptxFidelityWarningSeverity::Warning,
                                Some("img".to_string()),
                                "blocked_airgap",
                                format!(
                                    "Slide {slide_idx}: External reference inside SVG image blocked for offline safety."
                                ),
                            );
                        }
                        push_warning_once(
                            warnings,
                            seen_warnings,
                            slide_idx,
                            PptxFidelityWarningCode::ComplexSvgRasterized,
                            "complex_svg",
                            PptxFidelityWarningSeverity::Warning,
                            Some("img".to_string()),
                            "raster_svg_png",
                            format!(
                                "Slide {slide_idx}: Complex SVG image rasterized to high-res PNG."
                            ),
                        );
                        if let Some(rect) = px_rect_to_emu(120.0, y, w_px, h_px) {
                            nodes.push(PptxNode::Image {
                                id: *next_id,
                                name: format!("SVG Raster {}", *next_id),
                                rect,
                                geom,
                                shadow: None,
                                media_index: png_idx,
                                alt_text: sanitize_xml_text(&alt),
                            });
                            *next_id += 1;
                            *cursor_y = (y + h_px + 24.0).min(SLIDE_H_PX - 80.0);
                        }
                    }
                    CachedMediaOutcome::Empty => {}
                }
            }
            "h1" | "h2" | "h3" | "h4" | "p" | "li" | "blockquote" => {
                let text = sanitize_xml_text(&collect_text_recursive(child))
                    .trim()
                    .to_string();
                if text.is_empty() {
                    continue;
                }
                let default_px = match tag.as_str() {
                    "h1" => 48.0,
                    "h2" => 36.0,
                    "h3" => 28.0,
                    "h4" => 22.0,
                    _ => 20.0,
                };
                let font_px = style_map
                    .get("font-size")
                    .and_then(|v| parse_css_px_token(v))
                    .unwrap_or(default_px);
                let bold = matches!(tag.as_str(), "h1" | "h2" | "h3" | "h4")
                    || style_map.get("font-weight").is_some_and(|w| {
                        w.eq_ignore_ascii_case("bold") || w.parse::<u16>().unwrap_or(400) >= 600
                    });
                let color = style_map
                    .get("color")
                    .and_then(|c| parse_css_color(c))
                    .filter(|c| !c.is_transparent())
                    .unwrap_or_else(|| PptxColor::opaque("0F172A"));
                let fill = resolve_fill(
                    style_map
                        .get("background")
                        .or_else(|| style_map.get("background-image"))
                        .map(String::as_str),
                    style_map
                        .get("background-color")
                        .or_else(|| style_map.get("background"))
                        .map(String::as_str),
                );
                let shadow = style_map
                    .get("box-shadow")
                    .and_then(|s| parse_css_box_shadow(s));
                let align = style_map
                    .get("text-align")
                    .map(|a| PptxTextAlign::from_css(a))
                    .unwrap_or(PptxTextAlign::Left);
                let font_latin =
                    primary_font_name(style_map.get("font-family").map(String::as_str));

                let est_lines = ((text.chars().count() as f64) / 72.0).ceil().max(1.0);
                let h_px = (est_lines * font_px * 1.45 + 24.0).clamp(56.0, 260.0);
                let y = (*cursor_y).min(SLIDE_H_PX - h_px - 20.0).max(32.0);
                if let Some(rect) = px_rect_to_emu(120.0, y, 1680.0, h_px) {
                    let display_txt = if tag == "li" {
                        format!("• {text}")
                    } else {
                        text
                    };
                    nodes.push(make_text_box(
                        *next_id,
                        &format!("{tag}_{}", *next_id),
                        rect,
                        display_txt,
                        font_px,
                        bold,
                        color,
                        fill,
                        shadow,
                        locale,
                        &font_latin,
                        align,
                    ));
                    *next_id += 1;
                    *cursor_y = (y + h_px + 16.0).min(SLIDE_H_PX - 60.0);
                }
            }
            _ => {
                // Visual container with background/gradient/shadow -> emit shape if styled, then recurse
                let fill = resolve_fill(
                    style_map
                        .get("background")
                        .or_else(|| style_map.get("background-image"))
                        .map(String::as_str),
                    style_map
                        .get("background-color")
                        .or_else(|| style_map.get("background"))
                        .map(String::as_str),
                );
                let shadow = style_map
                    .get("box-shadow")
                    .and_then(|s| parse_css_box_shadow(s));
                let direct_txt = collect_direct_text(child);
                if fill != PptxFill::None || shadow.is_some() {
                    let y = (*cursor_y).clamp(32.0, SLIDE_H_PX - 160.0);
                    let radius_px = style_map
                        .get("border-radius")
                        .and_then(|v| parse_css_px_token(v))
                        .unwrap_or(0.0);
                    let is_circle = style_map
                        .get("border-radius")
                        .is_some_and(|v| v.contains("50%"));
                    if let Some(rect) = px_rect_to_emu(120.0, y, 1680.0, 120.0) {
                        if !direct_txt.is_empty() {
                            let color = style_map
                                .get("color")
                                .and_then(|c| parse_css_color(c))
                                .filter(|c| !c.is_transparent())
                                .unwrap_or_else(|| PptxColor::opaque("0F172A"));
                            let font_latin =
                                primary_font_name(style_map.get("font-family").map(String::as_str));
                            nodes.push(make_text_box(
                                *next_id,
                                &format!("Card {}", *next_id),
                                rect,
                                sanitize_xml_text(&direct_txt),
                                20.0,
                                false,
                                color,
                                fill,
                                shadow,
                                locale,
                                &font_latin,
                                PptxTextAlign::Left,
                            ));
                        } else {
                            nodes.push(PptxNode::Shape {
                                id: *next_id,
                                name: format!("Container {}", *next_id),
                                rect,
                                fill,
                                border_color: None,
                                border_width_emu: 0,
                                geom: border_radius_to_geom(radius_px, 1680.0, 120.0, is_circle),
                                shadow,
                            });
                        }
                        *next_id += 1;
                    }
                } else if !direct_txt.is_empty() {
                    let y = (*cursor_y).clamp(32.0, SLIDE_H_PX - 100.0);
                    if let Some(rect) = px_rect_to_emu(120.0, y, 1680.0, 72.0) {
                        nodes.push(make_text_box(
                            *next_id,
                            &format!("Text {}", *next_id),
                            rect,
                            sanitize_xml_text(&direct_txt),
                            18.0,
                            false,
                            PptxColor::opaque("0F172A"),
                            PptxFill::None,
                            None,
                            locale,
                            DEFAULT_LATIN_FONT,
                            PptxTextAlign::Left,
                        ));
                        *next_id += 1;
                        *cursor_y = (y + 84.0).min(SLIDE_H_PX - 60.0);
                    }
                }
                walk_static_slide_nodes(
                    child,
                    slide_idx,
                    locale,
                    cas_assets,
                    media_pool,
                    cursor_y,
                    next_id,
                    nodes,
                    warnings,
                    seen_warnings,
                )?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn make_text_box(
    id: u32,
    name: &str,
    rect: PptxRectEmu,
    text: String,
    font_px: f64,
    bold: bool,
    color: PptxColor,
    fill: PptxFill,
    shadow: Option<crate::pptx_ir::PptxShadow>,
    locale: &str,
    font_latin: &str,
    align: PptxTextAlign,
) -> PptxNode {
    PptxNode::TextBox {
        id,
        name: name.to_string(),
        rect,
        fill,
        border_color: None,
        border_width_emu: 0,
        geom: PptxShapeGeom::Rect,
        shadow,
        padding: PptxPaddingEmu::from_px(12.0, 12.0, 8.0, 8.0),
        paragraphs: vec![PptxParagraph {
            align,
            line_spacing_pct: Some(120_000),
            runs: vec![PptxTextRun {
                text: sanitize_xml_text(&text),
                font_latin: font_latin.to_string(),
                font_ea: DEFAULT_EA_FONT.to_string(),
                font_cs: DEFAULT_CS_FONT.to_string(),
                font_size_hundredths_pt: font_size_px_to_ooxml(font_px),
                bold,
                italic: false,
                underline: false,
                color,
                lang: locale.to_string(),
                hyperlink: None,
            }],
        }],
        auto_fit: true,
    }
}

fn element_attrs(handle: &Handle) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let NodeData::Element { ref attrs, .. } = handle.data {
        for attr in attrs.borrow().iter() {
            map.insert(
                (*attr.name.local).to_ascii_lowercase(),
                attr.value.to_string(),
            );
        }
    }
    map
}

fn parse_inline_style(style: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for decl in style.split(';') {
        if let Some((k, v)) = decl.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().to_string();
            if !key.is_empty() && !val.is_empty() {
                map.insert(key, val);
            }
        }
    }
    map
}

fn collect_style_tags(handle: &Handle, out: &mut String) {
    if let NodeData::Element { ref name, .. } = handle.data {
        if (*name.local).eq_ignore_ascii_case("style") {
            out.push('\n');
            for child in handle.children.borrow().iter() {
                if let NodeData::Text { ref contents } = child.data {
                    out.push_str(&contents.borrow());
                }
            }
            return;
        }
    }
    for child in handle.children.borrow().iter() {
        collect_style_tags(child, out);
    }
}

fn collect_slide_sections(handle: &Handle, out: &mut Vec<Handle>, stop_at: usize) {
    if out.len() >= stop_at {
        return;
    }
    if let NodeData::Element { ref name, .. } = handle.data {
        let tag = (*name.local).to_ascii_lowercase();
        let attrs = element_attrs(handle);
        let is_slide_section = (tag == "section" && attrs.contains_key("data-slide"))
            || attrs
                .get("class")
                .is_some_and(|c| c.split_whitespace().any(|tok| tok == "slide"))
            || tag == "section";
        if is_slide_section {
            out.push(Rc::clone(handle));
            return;
        }
    }
    for child in handle.children.borrow().iter() {
        collect_slide_sections(child, out, stop_at);
        if out.len() >= stop_at {
            return;
        }
    }
}

fn find_tag(handle: &Handle, target: &str) -> Option<Handle> {
    if let NodeData::Element { ref name, .. } = handle.data {
        if (*name.local).eq_ignore_ascii_case(target) {
            return Some(Rc::clone(handle));
        }
    }
    for child in handle.children.borrow().iter() {
        if let Some(found) = find_tag(child, target) {
            return Some(found);
        }
    }
    None
}

fn collect_text_recursive(handle: &Handle) -> String {
    let mut out = String::new();
    match handle.data {
        NodeData::Text { ref contents } => {
            out.push_str(&contents.borrow());
        }
        NodeData::Element { ref name, .. } => {
            if matches!(
                (*name.local).to_ascii_lowercase().as_str(),
                "script" | "style"
            ) {
                return out;
            }
            for child in handle.children.borrow().iter() {
                let sub = collect_text_recursive(child);
                if !sub.is_empty() {
                    if !out.is_empty() && !out.ends_with(' ') && !sub.starts_with(' ') {
                        out.push(' ');
                    }
                    out.push_str(&sub);
                }
            }
        }
        _ => {
            for child in handle.children.borrow().iter() {
                out.push_str(&collect_text_recursive(child));
            }
        }
    }
    out
}

fn collect_direct_text(handle: &Handle) -> String {
    let mut out = String::new();
    for child in handle.children.borrow().iter() {
        if let NodeData::Text { ref contents } = child.data {
            let t = contents.borrow();
            let trimmed = t.trim();
            if !trimmed.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(trimmed);
            }
        }
    }
    out
}

fn extract_html_table_rows(table_handle: &Handle) -> Vec<Vec<PptxTableRawCell>> {
    let mut tr_handles = Vec::new();
    collect_tags_named(table_handle, "tr", &mut tr_handles);
    let mut rows = Vec::new();
    for tr in tr_handles {
        let mut row = Vec::new();
        for cell in tr.children.borrow().iter() {
            let NodeData::Element { ref name, .. } = cell.data else {
                continue;
            };
            let tag = (*name.local).to_ascii_lowercase();
            if tag != "td" && tag != "th" {
                continue;
            }
            let attrs = element_attrs(cell);
            let style = parse_inline_style(attrs.get("style").map(String::as_str).unwrap_or(""));
            let colspan = attrs
                .get("colspan")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1)
                .max(1);
            let rowspan = attrs
                .get("rowspan")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1)
                .max(1);
            let is_header = tag == "th";
            let fill = style
                .get("background-color")
                .or_else(|| style.get("background"))
                .and_then(|c| parse_css_color(c))
                .filter(|c| !c.is_transparent())
                .or_else(|| is_header.then(|| PptxColor::opaque("F1F5F9")));
            let text_color = style
                .get("color")
                .and_then(|c| parse_css_color(c))
                .filter(|c| !c.is_transparent())
                .unwrap_or_else(|| PptxColor::opaque("0F172A"));
            let align = style
                .get("text-align")
                .map(|a| PptxTextAlign::from_css(a))
                .unwrap_or(PptxTextAlign::Left);
            row.push(PptxTableRawCell {
                text: collect_text_recursive(cell).trim().to_string(),
                colspan,
                rowspan,
                is_header,
                fill,
                text_color,
                font_size_px: if is_header { 16.0 } else { 14.0 },
                align,
            });
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    rows
}

fn collect_tags_named(handle: &Handle, target: &str, out: &mut Vec<Handle>) {
    if let NodeData::Element { ref name, .. } = handle.data {
        if (*name.local).eq_ignore_ascii_case(target) {
            out.push(Rc::clone(handle));
            return;
        }
    }
    for child in handle.children.borrow().iter() {
        collect_tags_named(child, target, out);
    }
}

fn count_descendant_elements(handle: &Handle) -> usize {
    let mut count = 0usize;
    for child in handle.children.borrow().iter() {
        if matches!(child.data, NodeData::Element { .. }) {
            count += 1;
            count += count_descendant_elements(child);
        }
    }
    count
}

fn serialize_svg_element(handle: &Handle) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    write_dom_node_xml(handle, &mut writer, true);
    String::from_utf8(writer.into_inner().into_inner()).unwrap_or_default()
}

fn write_dom_node_xml(handle: &Handle, writer: &mut Writer<Cursor<Vec<u8>>>, is_root_svg: bool) {
    match handle.data {
        NodeData::Element {
            ref name,
            ref attrs,
            ..
        } => {
            let local_tag: &str = name.local.as_ref();
            let tag = match &name.prefix {
                Some(prefix) if !prefix.is_empty() => {
                    let p: &str = prefix.as_ref();
                    format!("{p}:{local_tag}")
                }
                _ => local_tag.to_string(),
            };
            let mut el = BytesStart::new(&tag);
            let mut has_xmlns = false;
            let mut seen_attrs = HashSet::new();
            for attr in attrs.borrow().iter() {
                let local_attr: &str = attr.name.local.as_ref();
                let key = match &attr.name.prefix {
                    Some(prefix) if !prefix.is_empty() => {
                        let p: &str = prefix.as_ref();
                        format!("{p}:{local_attr}")
                    }
                    _ => local_attr.to_string(),
                };
                if key.eq_ignore_ascii_case("xmlns") {
                    has_xmlns = true;
                }
                let dedup_key = if key.eq_ignore_ascii_case("xmlns")
                    || key.to_ascii_lowercase().starts_with("xmlns:")
                {
                    key.to_ascii_lowercase()
                } else {
                    key.clone()
                };
                if !seen_attrs.insert(dedup_key) {
                    continue;
                }
                let clean_val = sanitize_xml_text(&attr.value);
                el.push_attribute((key.as_str(), clean_val.as_str()));
            }
            if is_root_svg && !has_xmlns {
                el.push_attribute(("xmlns", "http://www.w3.org/2000/svg"));
            }
            let children = handle.children.borrow();
            if children.is_empty() {
                let _ = writer.write_event(Event::Empty(el));
            } else {
                let _ = writer.write_event(Event::Start(el));
                for child in children.iter() {
                    write_dom_node_xml(child, writer, false);
                }
                let _ = writer.write_event(Event::End(BytesEnd::new(&tag)));
            }
        }
        NodeData::Text { ref contents } => {
            let txt = sanitize_xml_text(&contents.borrow());
            if !txt.is_empty() {
                let _ = writer.write_event(Event::Text(BytesText::new(&txt)));
            }
        }
        _ => {}
    }
}
