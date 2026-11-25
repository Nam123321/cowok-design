// @story 07.3
// @implements AC-01, AC-02
//! Intermediate Representation (IR), ECMA-376 unit conversions, XML 1.0 text sanitization,
//! Unicode NFC normalization, CSS color/shadow/gradient converters, and 2D table merge-grid
//! expansion for the Built-in Native Rust OOXML `.pptx` Exporter.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::oklch::{oklch_to_hex, OklchColor};

/// Canonical 16:9 Widescreen PowerPoint slide width in EMUs (13.333333 in * 914,400).
pub const SLIDE_W_EMU: i64 = 12_192_000;
/// Canonical 16:9 Widescreen PowerPoint slide height in EMUs (7.5 in * 914,400).
pub const SLIDE_H_EMU: i64 = 6_858_000;
/// Exact pixel-to-EMU scale factor for 1920x1080 slides (`12,192,000 / 1920 == 6,350`).
pub const EMU_PER_PX: i64 = 6_350;
pub const SLIDE_W_PX: f64 = 1920.0;
pub const SLIDE_H_PX: f64 = 1080.0;
/// Maximum slides allowed in a single deck to prevent XML/ZIP bomb amplification.
pub const MAX_SLIDES_PER_DECK: usize = 200;
/// Maximum native shapes emitted per slide before rasterizing excess groups.
pub const MAX_SHAPES_PER_SLIDE: usize = 250;
/// Maximum SVG child primitives before falling back to raster PNG @2x.
pub const MAX_SVG_NODES: usize = 300;
/// +4% horizontal safety buffer on text boxes to prevent Chrome vs PowerPoint metric wrap drift.
pub const TEXT_WIDTH_SAFETY_BUFFER_RATIO: f64 = 1.04;

pub const DEFAULT_LATIN_FONT: &str = "Inter";
pub const DEFAULT_EA_FONT: &str = "Noto Sans CJK JP";
pub const DEFAULT_CS_FONT: &str = "Inter";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PptxExportMode {
    #[default]
    #[serde(rename = "editable")]
    Editable,
    #[serde(rename = "visual_exact")]
    VisualExact,
}

impl PptxExportMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Editable => "editable",
            Self::VisualExact => "visual_exact",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PptxFidelityWarningCode {
    #[serde(rename = "BACKDROP_FILTER_RASTERIZED")]
    BackdropFilterRasterized,
    #[serde(rename = "CSS_ANIMATION_OMITTED")]
    CssAnimationOmitted,
    #[serde(rename = "HOVER_STATE_OMITTED")]
    HoverStateOmitted,
    #[serde(rename = "CANVAS_OR_3D_RASTERIZED")]
    CanvasOr3dRasterized,
    #[serde(rename = "COMPLEX_SVG_RASTERIZED")]
    ComplexSvgRasterized,
    #[serde(rename = "SHAPE_BUDGET_RASTERIZED")]
    ShapeBudgetRasterized,
    #[serde(rename = "CUSTOM_FONT_FALLBACK")]
    CustomFontFallback,
    #[serde(rename = "STATIC_LAYOUT_FALLBACK")]
    StaticLayoutFallback,
    #[serde(rename = "EXTERNAL_MEDIA_BLOCKED")]
    ExternalMediaBlocked,
    #[serde(rename = "VISUAL_EXACT_RASTER_UNAVAILABLE")]
    VisualExactRasterUnavailable,
    #[serde(rename = "INVALID_MEDIA_REJECTED")]
    InvalidMediaRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PptxFidelityWarningSeverity {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warning")]
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxFidelityWarning {
    pub slide_index: usize,
    pub code: PptxFidelityWarningCode,
    pub severity: PptxFidelityWarningSeverity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_label: Option<String>,
    pub fallback_applied: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PptxSlideTransition {
    #[default]
    None,
    Fade,
    Push,
    Wipe,
    Morph,
}

impl PptxSlideTransition {
    pub fn from_attr(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "fade" => Self::Fade,
            "push" | "slide" => Self::Push,
            "wipe" => Self::Wipe,
            "morph" | "auto-animate" => Self::Morph,
            _ => Self::None,
        }
    }
}

/// Normalized sRGB 6-hex color + ECMA-376 alpha (`0..=100_000`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PptxColor {
    pub hex: String,
    pub alpha_ooxml: u32,
}

impl PptxColor {
    pub fn opaque(hex6: &str) -> Self {
        let cleaned = normalize_hex6(hex6).unwrap_or_else(|| "0F172A".to_string());
        Self {
            hex: cleaned,
            alpha_ooxml: 100_000,
        }
    }

    pub fn with_alpha(hex6: &str, alpha_0_to_1: f64) -> Self {
        let cleaned = normalize_hex6(hex6).unwrap_or_else(|| "0F172A".to_string());
        let clamped = if alpha_0_to_1.is_finite() {
            alpha_0_to_1.clamp(0.0, 1.0)
        } else {
            1.0
        };
        Self {
            hex: cleaned,
            alpha_ooxml: (clamped * 100_000.0).round() as u32,
        }
    }

    pub fn is_transparent(&self) -> bool {
        self.alpha_ooxml == 0
    }

    pub fn to_rgb_bytes(&self) -> (u8, u8, u8) {
        let bytes = self.hex.as_bytes();
        if bytes.len() != 6 {
            return (255, 255, 255);
        }
        let r = u8::from_str_radix(&self.hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&self.hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&self.hex[4..6], 16).unwrap_or(255);
        (r, g, b)
    }
}

fn normalize_hex6(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches('#');
    if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(trimmed.to_ascii_uppercase())
    } else {
        None
    }
}

/// Parses CSS colors (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `oklch()`,
/// `transparent`, and common named colors) into ECMA-376 sRGB 6-hex + alpha (`0..=100_000`).
pub fn parse_css_color(raw: &str) -> Option<PptxColor> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    if lower == "transparent" || lower == "none" {
        return Some(PptxColor {
            hex: "000000".to_string(),
            alpha_ooxml: 0,
        });
    }
    if let Some(hex_body) = lower.strip_prefix('#') {
        let chars: Vec<char> = hex_body.chars().collect();
        if !chars.iter().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        return match chars.len() {
            3 => {
                let hex = format!("{0}{0}{1}{1}{2}{2}", chars[0], chars[1], chars[2])
                    .to_ascii_uppercase();
                Some(PptxColor {
                    hex,
                    alpha_ooxml: 100_000,
                })
            }
            4 => {
                let hex = format!("{0}{0}{1}{1}{2}{2}", chars[0], chars[1], chars[2])
                    .to_ascii_uppercase();
                let a_str = format!("{0}{0}", chars[3]);
                let a_byte = u8::from_str_radix(&a_str, 16).ok()?;
                Some(PptxColor::with_alpha(&hex, f64::from(a_byte) / 255.0))
            }
            6 => Some(PptxColor {
                hex: hex_body.to_ascii_uppercase(),
                alpha_ooxml: 100_000,
            }),
            8 => {
                let hex = hex_body[0..6].to_ascii_uppercase();
                let a_byte = u8::from_str_radix(&hex_body[6..8], 16).ok()?;
                Some(PptxColor::with_alpha(&hex, f64::from(a_byte) / 255.0))
            }
            _ => None,
        };
    }

    if (lower.starts_with("rgb(") || lower.starts_with("rgba(")) && lower.ends_with(')') {
        let open = lower.find('(')?;
        let inner = &lower[open + 1..lower.len() - 1];
        let (rgb_part, slash_alpha) = match inner.split_once('/') {
            Some((left, right)) => (left, Some(right.trim())),
            None => (inner, None),
        };
        let parts: Vec<&str> = rgb_part
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let r = parse_rgb_channel(parts[0])?;
        let g = parse_rgb_channel(parts[1])?;
        let b = parse_rgb_channel(parts[2])?;
        let alpha = if let Some(a_str) = slash_alpha.or_else(|| parts.get(3).copied()) {
            parse_alpha_token(a_str).unwrap_or(1.0)
        } else {
            1.0
        };
        let hex = format!("{r:02X}{g:02X}{b:02X}");
        return Some(PptxColor::with_alpha(&hex, alpha));
    }

    if lower.starts_with("oklch(") && lower.ends_with(')') {
        let inner = &lower[6..lower.len() - 1];
        let (lch_part, slash_alpha) = match inner.split_once('/') {
            Some((left, right)) => (left, Some(right.trim())),
            None => (inner, None),
        };
        let parts: Vec<&str> = lch_part
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let l = parse_percent_or_unit(parts[0])?;
        let c = parts[1].parse::<f64>().ok()?;
        let h = parts[2].trim_end_matches("deg").parse::<f64>().ok()?;
        if !l.is_finite() || !c.is_finite() || !h.is_finite() {
            return None;
        }
        let hex_with_hash = oklch_to_hex(OklchColor {
            l: l.clamp(0.0, 1.0),
            c: c.max(0.0),
            h: h.rem_euclid(360.0),
        });
        let alpha = slash_alpha.and_then(parse_alpha_token).unwrap_or(1.0);
        return Some(PptxColor::with_alpha(&hex_with_hash, alpha));
    }

    match lower.as_str() {
        "white" => Some(PptxColor::opaque("FFFFFF")),
        "black" => Some(PptxColor::opaque("000000")),
        "red" => Some(PptxColor::opaque("EF4444")),
        "green" => Some(PptxColor::opaque("22C55E")),
        "blue" => Some(PptxColor::opaque("3B82F6")),
        "yellow" => Some(PptxColor::opaque("EAB308")),
        "orange" => Some(PptxColor::opaque("F97316")),
        "purple" => Some(PptxColor::opaque("A855F7")),
        "gray" | "grey" => Some(PptxColor::opaque("64748B")),
        "slate" => Some(PptxColor::opaque("0F172A")),
        _ => None,
    }
}

fn parse_rgb_channel(tok: &str) -> Option<u8> {
    if let Some(pct) = tok.strip_suffix('%') {
        let v = pct.parse::<f64>().ok()?;
        if !v.is_finite() {
            return None;
        }
        Some((v.clamp(0.0, 100.0) * 2.55).round() as u8)
    } else {
        let v = tok.parse::<f64>().ok()?;
        if !v.is_finite() {
            return None;
        }
        Some(v.clamp(0.0, 255.0).round() as u8)
    }
}

fn parse_percent_or_unit(tok: &str) -> Option<f64> {
    if let Some(pct) = tok.strip_suffix('%') {
        let v = pct.parse::<f64>().ok()?;
        Some(v / 100.0)
    } else {
        tok.parse::<f64>().ok()
    }
}

fn parse_alpha_token(tok: &str) -> Option<f64> {
    let v = parse_percent_or_unit(tok.trim())?;
    if v.is_finite() {
        Some(v.clamp(0.0, 1.0))
    } else {
        None
    }
}

/// Sanitizes arbitrary text for XML 1.0 compliance + Unicode NFC normalization (preventing
/// Vietnamese combining diacritic splits on Keynote/Google Slides and PowerPoint Repair errors).
pub fn sanitize_xml_text(raw: &str) -> String {
    raw.nfc()
        .filter(|&ch| {
            let code = ch as u32;
            code == 0x09
                || code == 0x0A
                || code == 0x0D
                || (0x20..=0xD7FF).contains(&code)
                || (0xE000..=0xFFFD).contains(&code)
                || (0x10000..=0x10FFFF).contains(&code)
        })
        .collect()
}

/// Transliterates Vietnamese diacritics and strips unsafe path segments to produce a clean `.pptx` filename.
pub fn slugify_pptx_filename(raw: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in raw.trim().nfd() {
        let mapped = match ch {
            'đ' | 'Đ' => Some('d'),
            c if c.is_ascii_alphanumeric() => Some(c.to_ascii_lowercase()),
            c if ('\u{0300}'..='\u{036F}').contains(&c) => None, // strip combining marks
            _ => {
                if !last_dash && !out.is_empty() {
                    out.push('-');
                    last_dash = true;
                }
                None
            }
        };
        if let Some(m) = mapped {
            out.push(m);
            last_dash = false;
        }
    }
    let trimmed = out.trim_matches('-');
    let base = if trimmed.is_empty() {
        "presentation"
    } else if trimmed.len() > 64 {
        trimmed[..64].trim_end_matches('-')
    } else {
        trimmed
    };
    let final_base = if base.is_empty() {
        "presentation"
    } else {
        base
    };
    format!("{final_base}.pptx")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PptxRectEmu {
    pub x: i64,
    pub y: i64,
    pub cx: i64,
    pub cy: i64,
}

/// Converts a 1920x1080 CSS pixel box into strictly clamped non-negative ECMA-376 EMUs.
/// Rejects `NaN`, `Infinity`, zero/negative size, and elements completely off-canvas.
pub fn px_rect_to_emu(x: f64, y: f64, w: f64, h: f64) -> Option<PptxRectEmu> {
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || !h.is_finite() {
        return None;
    }
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let right = x + w;
    let bottom = y + h;
    if right <= 0.0 || bottom <= 0.0 || x >= SLIDE_W_PX || y >= SLIDE_H_PX {
        return None;
    }
    let clamped_x_px = x.clamp(0.0, SLIDE_W_PX - 1.0);
    let clamped_y_px = y.clamp(0.0, SLIDE_H_PX - 1.0);
    let clamped_r_px = right.clamp(clamped_x_px + 1.0, SLIDE_W_PX);
    let clamped_b_px = bottom.clamp(clamped_y_px + 1.0, SLIDE_H_PX);

    let x_emu = ((clamped_x_px * (EMU_PER_PX as f64)).round() as i64).clamp(0, SLIDE_W_EMU - 1);
    let y_emu = ((clamped_y_px * (EMU_PER_PX as f64)).round() as i64).clamp(0, SLIDE_H_EMU - 1);
    let cx_emu = (((clamped_r_px - clamped_x_px) * (EMU_PER_PX as f64)).round() as i64)
        .clamp(EMU_PER_PX, (SLIDE_W_EMU - x_emu).max(EMU_PER_PX));
    let cy_emu = (((clamped_b_px - clamped_y_px) * (EMU_PER_PX as f64)).round() as i64)
        .clamp(EMU_PER_PX, (SLIDE_H_EMU - y_emu).max(EMU_PER_PX));

    Some(PptxRectEmu {
        x: x_emu,
        y: y_emu,
        cx: cx_emu,
        cy: cy_emu,
    })
}

/// Applies the +4% text-box width safety buffer without overflowing past the slide edge or neighbor.
pub fn apply_text_width_buffer(rect: PptxRectEmu, max_right_emu: Option<i64>) -> PptxRectEmu {
    let buffered_cx = ((rect.cx as f64) * TEXT_WIDTH_SAFETY_BUFFER_RATIO).round() as i64;
    let limit_right = max_right_emu.unwrap_or(SLIDE_W_EMU).min(SLIDE_W_EMU);
    let max_cx = (limit_right - rect.x).max(rect.cx);
    PptxRectEmu {
        cx: buffered_cx.clamp(rect.cx, max_cx),
        ..rect
    }
}

/// Converts CSS `font-size` in pixels (96dpi) to OOXML `sz` (1/100 pt).
/// Example: `16px -> 12pt -> 1200`.
pub fn font_size_px_to_ooxml(font_size_px: f64) -> u32 {
    let safe_px = if font_size_px.is_finite() {
        font_size_px.clamp(6.0, 240.0)
    } else {
        16.0
    };
    (safe_px * 0.75 * 100.0).round() as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PptxPaddingEmu {
    pub left: i64,
    pub right: i64,
    pub top: i64,
    pub bottom: i64,
}

impl PptxPaddingEmu {
    pub fn from_px(left: f64, right: f64, top: f64, bottom: f64) -> Self {
        let to_emu = |v: f64| -> i64 {
            if v.is_finite() && v > 0.0 {
                (v.clamp(0.0, 200.0) * (EMU_PER_PX as f64)).round() as i64
            } else {
                0
            }
        };
        Self {
            left: to_emu(left),
            right: to_emu(right),
            top: to_emu(top),
            bottom: to_emu(bottom),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PptxShadow {
    pub dist_emu: i64,
    pub dir_ooxml: i64,
    pub blur_rad_emu: i64,
    pub color: PptxColor,
}

/// Converts Cartesian CSS `box-shadow(dx, dy, blur, color)` into Polar OOXML `<a:outerShdw>`.
pub fn css_box_shadow_to_ooxml(
    dx_px: f64,
    dy_px: f64,
    blur_px: f64,
    color: PptxColor,
) -> Option<PptxShadow> {
    if !dx_px.is_finite() || !dy_px.is_finite() || !blur_px.is_finite() || color.is_transparent() {
        return None;
    }
    let dist_emu = (dx_px.hypot(dy_px) * (EMU_PER_PX as f64))
        .round()
        .clamp(0.0, 20_000_000.0) as i64;
    let deg = (dy_px.atan2(dx_px).to_degrees() + 360.0) % 360.0;
    let dir_ooxml = (deg * 60_000.0).round().clamp(0.0, 21_599_999.0) as i64;
    let blur_rad_emu = (blur_px.max(0.0) * (EMU_PER_PX as f64))
        .round()
        .clamp(0.0, 20_000_000.0) as i64;
    Some(PptxShadow {
        dist_emu,
        dir_ooxml,
        blur_rad_emu,
        color,
    })
}

/// Parses a single CSS `box-shadow` value string into `PptxShadow`.
pub fn parse_css_box_shadow(raw: &str) -> Option<PptxShadow> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return None;
    }
    // Extract color token (rgba(...), rgb(...), oklch(...), or #hex)
    let mut color: Option<PptxColor> = None;
    let mut remainder = trimmed.to_string();
    for prefix in ["rgba(", "rgb(", "oklch("] {
        if let Some(start) = remainder.to_ascii_lowercase().find(prefix) {
            if let Some(rel_end) = remainder[start..].find(')') {
                let end = start + rel_end + 1;
                color = parse_css_color(&remainder[start..end]);
                remainder.replace_range(start..end, " ");
                break;
            }
        }
    }
    let mut lengths: Vec<f64> = Vec::new();
    for tok in remainder.split_whitespace() {
        if tok.eq_ignore_ascii_case("inset") {
            return None;
        }
        if color.is_none() && (tok.starts_with('#') || tok.chars().all(|c| c.is_ascii_alphabetic()))
        {
            if let Some(parsed) = parse_css_color(tok) {
                color = Some(parsed);
                continue;
            }
        }
        if let Some(px) = parse_css_px_token(tok) {
            lengths.push(px);
        }
    }
    if lengths.len() < 2 {
        return None;
    }
    let dx = lengths[0];
    let dy = lengths[1];
    let blur = lengths.get(2).copied().unwrap_or(0.0);
    let resolved_color = color.unwrap_or_else(|| PptxColor::with_alpha("000000", 0.25));
    css_box_shadow_to_ooxml(dx, dy, blur, resolved_color)
}

pub fn parse_css_px_token(tok: &str) -> Option<f64> {
    let t = tok.trim().to_ascii_lowercase();
    if t == "0" {
        return Some(0.0);
    }
    if let Some(num) = t.strip_suffix("px") {
        let v = num.parse::<f64>().ok()?;
        return v.is_finite().then_some(v);
    }
    if let Some(num) = t.strip_suffix("rem") {
        let v = num.parse::<f64>().ok()?;
        return v.is_finite().then_some(v * 16.0);
    }
    if let Some(num) = t.strip_suffix("pt") {
        let v = num.parse::<f64>().ok()?;
        return v.is_finite().then_some(v * (4.0 / 3.0));
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PptxGradientStop {
    /// OOXML stop position (`0..=100_000`).
    pub pos_ooxml: u32,
    pub color: PptxColor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PptxLinearGradient {
    /// OOXML `<a:lin ang="..."/>` angle (`0..21_600_000`, `0` = left-to-right clockwise).
    pub ang_ooxml: i64,
    pub stops: Vec<PptxGradientStop>,
}

/// Converts CSS `linear-gradient` angle (`0deg` = bottom-to-top) to OOXML angle (`0` = left-to-right).
pub fn css_linear_gradient_deg_to_ooxml(css_deg: f64) -> i64 {
    let safe = if css_deg.is_finite() { css_deg } else { 180.0 };
    let ooxml_deg = (safe - 90.0).rem_euclid(360.0);
    (ooxml_deg * 60_000.0).round().clamp(0.0, 21_599_999.0) as i64
}

/// Parses CSS `linear-gradient(...)` into `PptxLinearGradient` (up to 4 stops; complex gradients
/// can still be flagged by the layout extractor when appropriate).
pub fn parse_css_linear_gradient(raw: &str) -> Option<PptxLinearGradient> {
    let lower = raw.to_ascii_lowercase();
    let start = lower.find("linear-gradient(")?;
    let body_start = start + "linear-gradient(".len();
    let mut depth = 1usize;
    let mut body_end = None;
    for (idx, ch) in raw[body_start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    body_end = Some(body_start + idx);
                    break;
                }
            }
            _ => {}
        }
    }
    let inner = &raw[body_start..body_end?];
    let args = split_top_level_commas(inner);
    if args.len() < 2 {
        return None;
    }
    let mut deg = 180.0;
    let mut stop_start_idx = 0usize;
    let first = args[0].trim().to_ascii_lowercase();
    if let Some(d_str) = first.strip_suffix("deg") {
        if let Ok(d) = d_str.trim().parse::<f64>() {
            deg = d;
            stop_start_idx = 1;
        }
    } else if first.starts_with("to ") {
        deg = match first.as_str() {
            "to top" => 0.0,
            "to top right" | "to right top" => 45.0,
            "to right" => 90.0,
            "to bottom right" | "to right bottom" => 135.0,
            "to bottom" => 180.0,
            "to bottom left" | "to left bottom" => 225.0,
            "to left" => 270.0,
            "to top left" | "to left top" => 315.0,
            _ => 180.0,
        };
        stop_start_idx = 1;
    }
    let raw_stops = &args[stop_start_idx..];
    if raw_stops.len() < 2 {
        return None;
    }
    let mut stops = Vec::with_capacity(raw_stops.len());
    let last_idx = raw_stops.len().saturating_sub(1).max(1);
    for (i, item) in raw_stops.iter().enumerate() {
        let trimmed = item.trim();
        let (color_str, explicit_pos) = split_gradient_stop(trimmed);
        let color = parse_css_color(color_str)?;
        let pos_ooxml = explicit_pos
            .unwrap_or_else(|| ((i as f64 / last_idx as f64) * 100_000.0).round() as u32);
        stops.push(PptxGradientStop {
            pos_ooxml: pos_ooxml.min(100_000),
            color,
        });
    }
    Some(PptxLinearGradient {
        ang_ooxml: css_linear_gradient_deg_to_ooxml(deg),
        stops,
    })
}

fn split_top_level_commas(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in input.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

fn split_gradient_stop(raw: &str) -> (&str, Option<u32>) {
    if let Some((head, tail)) = raw.rsplit_once(' ') {
        if let Some(pct_str) = tail.trim().strip_suffix('%') {
            if let Ok(pct) = pct_str.parse::<f64>() {
                if pct.is_finite() {
                    return (
                        head.trim(),
                        Some((pct.clamp(0.0, 100.0) * 1000.0).round() as u32),
                    );
                }
            }
        }
    }
    (raw, None)
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PptxFill {
    #[default]
    None,
    Solid(PptxColor),
    LinearGradient(PptxLinearGradient),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PptxShapeGeom {
    #[default]
    Rect,
    RoundRect {
        /// OOXML corner radius adjustment (`0..=50_000`).
        adj: u32,
    },
    Ellipse,
}

/// Converts CSS `border-radius` into `<a:prstGeom>` (`ellipse` for 50% circular avatars, `roundRect`
/// with proportional `adj`, or `rect`).
pub fn border_radius_to_geom(
    radius_px: f64,
    width_px: f64,
    height_px: f64,
    is_circle_percent: bool,
) -> PptxShapeGeom {
    if is_circle_percent {
        return PptxShapeGeom::Ellipse;
    }
    if !radius_px.is_finite() || radius_px <= 0.5 {
        return PptxShapeGeom::Rect;
    }
    let min_dim = width_px.min(height_px).max(1.0);
    if radius_px >= (min_dim * 0.49) {
        return PptxShapeGeom::Ellipse;
    }
    let ratio = (radius_px / min_dim).clamp(0.0, 0.5);
    let adj = (ratio * 100_000.0).round() as u32;
    PptxShapeGeom::RoundRect {
        adj: adj.clamp(1_000, 50_000),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PptxTextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

impl PptxTextAlign {
    pub fn as_ooxml(self) -> &'static str {
        match self {
            Self::Left => "l",
            Self::Center => "ctr",
            Self::Right => "r",
            Self::Justify => "just",
        }
    }

    pub fn from_css(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "center" => Self::Center,
            "right" | "end" => Self::Right,
            "justify" => Self::Justify,
            _ => Self::Left,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PptxTextRun {
    pub text: String,
    pub font_latin: String,
    pub font_ea: String,
    pub font_cs: String,
    pub font_size_hundredths_pt: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: PptxColor,
    pub lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperlink: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PptxParagraph {
    pub align: PptxTextAlign,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing_pct: Option<u32>,
    pub runs: Vec<PptxTextRun>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PptxMediaFormat {
    Png,
    Jpeg,
    Svg,
}

impl PptxMediaFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Svg => "svg",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Svg => "image/svg+xml",
        }
    }
}

/// Validates the 8-byte PNG signature (`\x89PNG\r\n\x1a\n`).
pub fn is_valid_png_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n"
}

/// Validates the 3-byte JPEG SOI+marker signature (`\xFF\xD8\xFF`).
pub fn is_valid_jpeg_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && &bytes[0..3] == b"\xFF\xD8\xFF"
}

/// Computes the raw 32-byte SHA-256 digest of a byte slice.
pub fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// Deduplicated media asset stored once in `PptxDeckIr::media_pool` and referenced by index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PptxMediaAsset {
    pub sha256: [u8; 32],
    pub format: PptxMediaFormat,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PptxTableRawCell {
    pub text: String,
    pub colspan: usize,
    pub rowspan: usize,
    pub is_header: bool,
    pub fill: Option<PptxColor>,
    pub text_color: PptxColor,
    pub font_size_px: f64,
    pub align: PptxTextAlign,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PptxTableCellKind {
    Origin {
        text: String,
        grid_span: usize,
        row_span: usize,
        is_header: bool,
        fill: Option<PptxColor>,
        text_color: PptxColor,
        font_size_px: f64,
        align: PptxTextAlign,
    },
    HMerge,
    VMerge {
        grid_span: usize,
    },
    HvMerge,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PptxTableGrid {
    pub col_widths_emu: Vec<i64>,
    pub row_heights_emu: Vec<i64>,
    pub rows: Vec<Vec<PptxTableCellKind>>,
}

impl PptxTableGrid {
    /// Expands ragged HTML table rows with `colspan` / `rowspan` into a rectangular 2D ECMA-376
    /// `<a:tbl>` matrix containing mandatory `<a:tc hMerge="1">` / `<a:tc vMerge="1">` dummy cells.
    /// Clamps overlapping spans against already-occupied slots and row bounds so malformed tables
    /// never overwrite existing merges or corrupt OOXML output.
    pub fn from_raw_rows(raw_rows: &[Vec<PptxTableRawCell>], rect: PptxRectEmu) -> Self {
        if raw_rows.is_empty() {
            return Self {
                col_widths_emu: vec![rect.cx.max(EMU_PER_PX)],
                row_heights_emu: vec![rect.cy.max(EMU_PER_PX)],
                rows: vec![vec![PptxTableCellKind::Origin {
                    text: String::new(),
                    grid_span: 1,
                    row_span: 1,
                    is_header: false,
                    fill: None,
                    text_color: PptxColor::opaque("0F172A"),
                    font_size_px: 14.0,
                    align: PptxTextAlign::Left,
                }]],
            };
        }

        let total_raw_rows = raw_rows.len();
        let mut grid: Vec<Vec<Option<PptxTableCellKind>>> = vec![Vec::new(); total_raw_rows];

        for (r_idx, raw_row) in raw_rows.iter().enumerate() {
            let mut col = 0usize;
            for cell in raw_row {
                while col < grid[r_idx].len() && grid[r_idx][col].is_some() {
                    col += 1;
                }
                let req_colspan = cell.colspan.clamp(1, 32);
                let max_rowspan_bound = (total_raw_rows - r_idx).max(1);
                let req_rowspan = cell.rowspan.clamp(1, 64).min(max_rowspan_bound);

                // Clamp colspan against already-occupied slots on the current row so we never
                // overwrite an existing VMerge/HMerge slot.
                let mut effective_colspan = 1usize;
                while effective_colspan < req_colspan {
                    let target_col = col + effective_colspan;
                    if target_col < grid[r_idx].len() && grid[r_idx][target_col].is_some() {
                        break;
                    }
                    effective_colspan += 1;
                }

                // Clamp rowspan against already-occupied slots in any covered column below.
                let mut effective_rowspan = 1usize;
                'row_check: while effective_rowspan < req_rowspan {
                    let target_row = r_idx + effective_rowspan;
                    for dc in 0..effective_colspan {
                        let target_col = col + dc;
                        if target_col < grid[target_row].len()
                            && grid[target_row][target_col].is_some()
                        {
                            break 'row_check;
                        }
                    }
                    effective_rowspan += 1;
                }

                let needed_rows = r_idx + effective_rowspan;
                for row_vec in grid.iter_mut().take(needed_rows).skip(r_idx) {
                    if row_vec.len() < col + effective_colspan {
                        row_vec.resize(col + effective_colspan, None);
                    }
                }

                grid[r_idx][col] = Some(PptxTableCellKind::Origin {
                    text: sanitize_xml_text(&cell.text),
                    grid_span: effective_colspan,
                    row_span: effective_rowspan,
                    is_header: cell.is_header,
                    fill: cell.fill.clone(),
                    text_color: cell.text_color.clone(),
                    font_size_px: cell.font_size_px,
                    align: cell.align,
                });

                for dr in 0..effective_rowspan {
                    for dc in 0..effective_colspan {
                        if dr == 0 && dc == 0 {
                            continue;
                        }
                        let kind = match (dr > 0, dc > 0) {
                            (false, true) => PptxTableCellKind::HMerge,
                            (true, false) => PptxTableCellKind::VMerge {
                                grid_span: effective_colspan,
                            },
                            (true, true) => PptxTableCellKind::HvMerge,
                            (false, false) => unreachable!(),
                        };
                        grid[r_idx + dr][col + dc] = Some(kind);
                    }
                }
                col += effective_colspan;
            }
        }

        let max_cols = grid.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let num_rows = grid.len().max(1);

        let mut rows: Vec<Vec<PptxTableCellKind>> = Vec::with_capacity(num_rows);
        for row_opt in grid {
            let mut row_out = Vec::with_capacity(max_cols);
            for c in 0..max_cols {
                let cell = row_opt.get(c).and_then(|v| v.clone()).unwrap_or_else(|| {
                    PptxTableCellKind::Origin {
                        text: String::new(),
                        grid_span: 1,
                        row_span: 1,
                        is_header: false,
                        fill: None,
                        text_color: PptxColor::opaque("0F172A"),
                        font_size_px: 14.0,
                        align: PptxTextAlign::Left,
                    }
                });
                row_out.push(cell);
            }
            rows.push(row_out);
        }

        let base_col_w = (rect.cx / (max_cols as i64)).max(EMU_PER_PX);
        let col_widths_emu = vec![base_col_w; max_cols];
        let base_row_h = (rect.cy / (num_rows as i64)).max(EMU_PER_PX * 32);
        let row_heights_emu = vec![base_row_h; num_rows];

        Self {
            col_widths_emu,
            row_heights_emu,
            rows,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PptxNode {
    TextBox {
        id: u32,
        name: String,
        rect: PptxRectEmu,
        fill: PptxFill,
        border_color: Option<PptxColor>,
        border_width_emu: i64,
        geom: PptxShapeGeom,
        shadow: Option<PptxShadow>,
        padding: PptxPaddingEmu,
        paragraphs: Vec<PptxParagraph>,
        auto_fit: bool,
    },
    Shape {
        id: u32,
        name: String,
        rect: PptxRectEmu,
        fill: PptxFill,
        border_color: Option<PptxColor>,
        border_width_emu: i64,
        geom: PptxShapeGeom,
        shadow: Option<PptxShadow>,
    },
    Table {
        id: u32,
        name: String,
        rect: PptxRectEmu,
        grid: PptxTableGrid,
        lang: String,
        font_latin: String,
        font_ea: String,
    },
    Image {
        id: u32,
        name: String,
        rect: PptxRectEmu,
        geom: PptxShapeGeom,
        shadow: Option<PptxShadow>,
        media_index: usize,
        alt_text: String,
    },
    DualSvg {
        id: u32,
        name: String,
        rect: PptxRectEmu,
        geom: PptxShapeGeom,
        shadow: Option<PptxShadow>,
        svg_media_index: usize,
        png_fallback_media_index: usize,
        alt_text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PptxSlideIr {
    pub index: usize,
    pub background: PptxFill,
    pub transition: PptxSlideTransition,
    pub nodes: Vec<PptxNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PptxDeckIr {
    pub title: String,
    pub locale: String,
    pub export_mode: PptxExportMode,
    pub slides: Vec<PptxSlideIr>,
    #[serde(default)]
    pub media_pool: Vec<PptxMediaAsset>,
    pub fidelity_warnings: Vec<PptxFidelityWarning>,
    pub used_static_fallback: bool,
    pub engine: String,
}

/// Generates a minimal, 100% valid solid-color RGBA PNG without external image crate dependencies.
/// Used for deterministic Dual-Blip PNG fallback surrogates and static-fallback raster blocks.
pub fn encode_solid_png(width: u32, height: u32, color: &PptxColor) -> Vec<u8> {
    let w = width.clamp(1, 1920);
    let h = height.clamp(1, 1080);
    let (r, g, b) = color.to_rgb_bytes();
    let a = ((f64::from(color.alpha_ooxml) / 100_000.0) * 255.0).round() as u8;

    let row_len = 1 + (w as usize) * 4;
    let mut row = Vec::with_capacity(row_len);
    row.push(0u8); // Filter type 0 (None)
    for _ in 0..w {
        row.extend_from_slice(&[r, g, b, a]);
    }
    let mut raw_scanlines = Vec::with_capacity(row_len * (h as usize));
    for _ in 0..h {
        raw_scanlines.extend_from_slice(&row);
    }

    let zlib_data = wrap_uncompressed_zlib(&raw_scanlines);

    let mut png = Vec::with_capacity(64 + zlib_data.len());
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]); // PNG magic

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut png, b"IHDR", &ihdr);
    write_png_chunk(&mut png, b"IDAT", &zlib_data);
    write_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn wrap_uncompressed_zlib(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 16);
    out.push(0x78);
    out.push(0x01);
    let mut offset = 0usize;
    let max_block = 65_535usize;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let len = remaining.min(max_block);
        let is_final = offset + len >= data.len();
        out.push(if is_final { 0x01 } else { 0x00 });
        let len_u16 = len as u16;
        out.extend_from_slice(&len_u16.to_le_bytes());
        out.extend_from_slice(&(!len_u16).to_le_bytes());
        out.extend_from_slice(&data[offset..offset + len]);
        offset += len;
    }
    if data.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    }
    let adler = adler32(data);
    out.extend_from_slice(&adler.to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + u32::from(byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn write_png_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(payload);
    let mut crc_buf = Vec::with_capacity(4 + payload.len());
    crc_buf.extend_from_slice(chunk_type);
    crc_buf.extend_from_slice(payload);
    let crc = crc32_ieee(&crc_buf);
    out.extend_from_slice(&crc.to_be_bytes());
}
