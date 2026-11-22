// @story 05.4
// @implements AC-02
use serde::{Deserialize, Serialize};

use crate::patcher::HtmlForgeError;

pub const WCAG_AA_MIN_CONTRAST: f64 = 4.5;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OklchColor {
    pub l: f64,
    pub c: f64,
    pub h: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OklchContrastReport {
    pub original_fg_hex: String,
    pub bg_hex: String,
    pub adjusted_fg_hex: String,
    pub original_ratio: f64,
    pub adjusted_ratio: f64,
    pub min_ratio: f64,
    pub clamped: bool,
    pub warning: Option<String>,
}

fn parse_hex_rgb(hex: &str) -> Result<(u8, u8, u8), HtmlForgeError> {
    let stripped = hex.trim().strip_prefix('#').ok_or_else(|| {
        HtmlForgeError::InvalidPayload(format!("Expected #RRGGBB or #RGB hex color, got '{hex}'"))
    })?;
    if !stripped.is_ascii() || !stripped.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(HtmlForgeError::InvalidPayload(format!(
            "Invalid non-ASCII or non-hex color '{hex}'"
        )));
    }
    match stripped.len() {
        3 => {
            let expand = |idx: usize| -> Result<u8, HtmlForgeError> {
                let d = u8::from_str_radix(&stripped[idx..idx + 1], 16).map_err(|_| {
                    HtmlForgeError::InvalidPayload(format!("Invalid hex color '{hex}'"))
                })?;
                Ok((d << 4) | d)
            };
            Ok((expand(0)?, expand(1)?, expand(2)?))
        }
        6 => {
            let parse = |start: usize| -> Result<u8, HtmlForgeError> {
                u8::from_str_radix(&stripped[start..start + 2], 16).map_err(|_| {
                    HtmlForgeError::InvalidPayload(format!("Invalid hex color '{hex}'"))
                })
            };
            Ok((parse(0)?, parse(2)?, parse(4)?))
        }
        _ => Err(HtmlForgeError::InvalidPayload(format!(
            "Unsupported hex color length '{hex}'"
        ))),
    }
}

fn format_hex_rgb(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn srgb_to_linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb_u8(linear: f64) -> u8 {
    let clamped = linear.clamp(0.0, 1.0);
    let srgb = if clamped <= 0.003_130_8 {
        12.92 * clamped
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    };
    (srgb.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn relative_luminance(rgb: (u8, u8, u8)) -> f64 {
    0.2126 * srgb_to_linear(rgb.0) + 0.7152 * srgb_to_linear(rgb.1) + 0.0722 * srgb_to_linear(rgb.2)
}

fn contrast_ratio_rgb(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

pub fn contrast_ratio_hex(fg_hex: &str, bg_hex: &str) -> Result<f64, HtmlForgeError> {
    let fg = parse_hex_rgb(fg_hex)?;
    let bg = parse_hex_rgb(bg_hex)?;
    Ok(contrast_ratio_rgb(fg, bg))
}

pub fn hex_to_oklch(hex: &str) -> Result<OklchColor, HtmlForgeError> {
    let (r8, g8, b8) = parse_hex_rgb(hex)?;
    let r = srgb_to_linear(r8);
    let g = srgb_to_linear(g8);
    let b = srgb_to_linear(b8);

    let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
    let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
    let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    let ok_l = 0.210_454_255_3 * l_ + 0.793_617_785_0 * m_ - 0.004_072_046_8 * s_;
    let ok_a = 1.977_998_495_1 * l_ - 2.428_592_205_0 * m_ + 0.450_593_709_9 * s_;
    let ok_b = 0.025_904_037_1 * l_ + 0.782_771_766_2 * m_ - 0.808_675_766_0 * s_;

    let c = (ok_a * ok_a + ok_b * ok_b).sqrt();
    let h = ok_b.atan2(ok_a).to_degrees().rem_euclid(360.0);
    Ok(OklchColor { l: ok_l, c, h })
}

fn oklch_to_rgb(color: OklchColor) -> (u8, u8, u8) {
    let h_rad = color.h.to_radians();
    let ok_a = color.c * h_rad.cos();
    let ok_b = color.c * h_rad.sin();

    let l_ = color.l + 0.396_337_777_4 * ok_a + 0.215_803_757_3 * ok_b;
    let m_ = color.l - 0.105_561_345_8 * ok_a - 0.063_854_172_8 * ok_b;
    let s_ = color.l - 0.089_484_177_5 * ok_a - 1.291_485_548_0 * ok_b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    let r = 4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s;
    let g = -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s;
    let b = -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s;

    (
        linear_to_srgb_u8(r),
        linear_to_srgb_u8(g),
        linear_to_srgb_u8(b),
    )
}

pub fn oklch_to_hex(color: OklchColor) -> String {
    let (r, g, b) = oklch_to_rgb(color);
    format_hex_rgb(r, g, b)
}

/// Evaluates foreground vs background WCAG contrast and clamps OKLCH lightness `L`
/// to guarantee `>= min_ratio` (WCAG AA `4.5:1` by default) while preserving brand hue.
pub fn evaluate_oklch_contrast_guard(
    fg_hex: &str,
    bg_hex: &str,
    min_ratio: f64,
) -> Result<OklchContrastReport, HtmlForgeError> {
    let target_ratio = if min_ratio.is_finite() && min_ratio > 1.0 {
        min_ratio
    } else {
        WCAG_AA_MIN_CONTRAST
    };
    let fg_rgb = parse_hex_rgb(fg_hex)?;
    let bg_rgb = parse_hex_rgb(bg_hex)?;
    let original_fg_hex = format_hex_rgb(fg_rgb.0, fg_rgb.1, fg_rgb.2);
    let normalized_bg_hex = format_hex_rgb(bg_rgb.0, bg_rgb.1, bg_rgb.2);
    let original_ratio = contrast_ratio_rgb(fg_rgb, bg_rgb);

    if original_ratio >= target_ratio {
        return Ok(OklchContrastReport {
            original_fg_hex: original_fg_hex.clone(),
            bg_hex: normalized_bg_hex,
            adjusted_fg_hex: original_fg_hex,
            original_ratio,
            adjusted_ratio: original_ratio,
            min_ratio: target_ratio,
            clamped: false,
            warning: None,
        });
    }

    let fg_oklch = hex_to_oklch(&original_fg_hex)?;
    let bg_lum = relative_luminance(bg_rgb);
    let should_darken = bg_lum >= 0.179;

    let mut best_rgb = if should_darken {
        (0u8, 0u8, 0u8)
    } else {
        (255u8, 255u8, 255u8)
    };
    let mut best_ratio = contrast_ratio_rgb(best_rgb, bg_rgb);

    let mut lo = if should_darken { 0.0 } else { fg_oklch.l };
    let mut hi = if should_darken { fg_oklch.l } else { 1.0 };

    for _ in 0..24 {
        let mid = (lo + hi) * 0.5;
        let chroma_scale = (1.0 - (mid - fg_oklch.l).abs() * 0.6).clamp(0.25, 1.0);
        let candidate = oklch_to_rgb(OklchColor {
            l: mid,
            c: fg_oklch.c * chroma_scale,
            h: fg_oklch.h,
        });
        let ratio = contrast_ratio_rgb(candidate, bg_rgb);
        if ratio >= target_ratio {
            best_rgb = candidate;
            best_ratio = ratio;
            if should_darken {
                lo = mid;
            } else {
                hi = mid;
            }
        } else if should_darken {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    let adjusted_fg_hex = format_hex_rgb(best_rgb.0, best_rgb.1, best_rgb.2);
    Ok(OklchContrastReport {
        original_fg_hex: original_fg_hex.clone(),
        bg_hex: normalized_bg_hex.clone(),
        adjusted_fg_hex: adjusted_fg_hex.clone(),
        original_ratio,
        adjusted_ratio: best_ratio,
        min_ratio: target_ratio,
        clamped: true,
        warning: Some(format!(
            "OKLCH_WCAG_AA_CLAMPED: Adjusted {original_fg_hex} -> {adjusted_fg_hex} on {normalized_bg_hex} ({original_ratio:.2}:1 -> {best_ratio:.2}:1, min {target_ratio:.1}:1)"
        )),
    })
}
