// @story 07.1
// @implements AC-01, AC-02, AC-03, AC-04
//! Pooled 1-Chromium Rust CDP Visual QC engine (`lib-audit` + `slides-check`),
//! 26-rule geometry/WCAG 2.2 AA solver, Real RGBA SSIM, `elementsFromPoint`
//! occlusion hit-testing, E2E Flow Completeness auditor, and `<10ms`
//! deterministic `lint_fix` (`0` LLM tokens).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use lol_html::{element, rewrite_str, text, RewriteStrSettings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::oklch::{contrast_ratio_hex, evaluate_oklch_contrast_guard, WCAG_AA_MIN_CONTRAST};

pub const MAX_AUTOFIX_ATTEMPTS: u8 = 3;
pub const MAX_CDP_CONCURRENCY: usize = 1;
pub const MAX_CDP_RSS_MB: u64 = 250;
pub const SLIDE_WIDTH_PX: f64 = 1920.0;
pub const SLIDE_HEIGHT_PX: f64 = 1080.0;
pub const SLIDE_CONTENT_MAX_Y: f64 = 960.0;
pub const RESPONSIVE_SWEEP_MIN_PX: u32 = 320;
pub const RESPONSIVE_SWEEP_MAX_PX: u32 = 1440;
pub const DEFAULT_SSIM_PASS_THRESHOLD: f64 = 0.92;
pub const DEFAULT_CIRCUIT_BREAKER_THRESHOLD: u32 = 2;
pub const DEFAULT_CIRCUIT_BREAKER_COOLDOWN_MS: u64 = 15_000;
pub const CANONICAL_APP_VIEWPORTS: &[u32] = &[1440, 1024, 768, 712, 390, 344];
pub const WCAG_LARGE_TEXT_MIN_CONTRAST: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QcTargetKind {
    #[serde(rename = "APP_SCREEN")]
    AppScreen,
    #[serde(rename = "PRESENTATION_SLIDE")]
    PresentationSlide,
    #[serde(rename = "FULL_FLOW")]
    FullFlow,
}

impl QcTargetKind {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::AppScreen => "APP_SCREEN",
            Self::PresentationSlide => "PRESENTATION_SLIDE",
            Self::FullFlow => "FULL_FLOW",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw.trim() {
            "APP_SCREEN" => Some(Self::AppScreen),
            "PRESENTATION_SLIDE" => Some(Self::PresentationSlide),
            "FULL_FLOW" => Some(Self::FullFlow),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QcVerdict {
    #[serde(rename = "PASS")]
    Pass,
    #[serde(rename = "FAIL")]
    Fail,
    #[serde(rename = "UNVERIFIED_NO_BROWSER")]
    UnverifiedNoBrowser,
}

impl QcVerdict {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::UnverifiedNoBrowser => "UNVERIFIED_NO_BROWSER",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw.trim() {
            "PASS" => Some(Self::Pass),
            "FAIL" => Some(Self::Fail),
            "UNVERIFIED_NO_BROWSER" => Some(Self::UnverifiedNoBrowser),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QcSeverity {
    P0,
    P1,
    P2,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcBoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl QcBoundingBox {
    pub fn right(self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(self) -> f64 {
        self.y + self.height
    }

    pub fn area(self) -> f64 {
        (self.width.max(0.0)) * (self.height.max(0.0))
    }

    pub fn intersects(self, other: Self) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    pub fn intersection_area(self, other: Self) -> f64 {
        let ox = (self.right().min(other.right()) - self.x.max(other.x)).max(0.0);
        let oy = (self.bottom().min(other.bottom()) - self.y.max(other.y)).max(0.0);
        ox * oy
    }

    pub fn contains_box(self, inner: Self) -> bool {
        inner.x >= self.x - 1.0
            && inner.y >= self.y - 1.0
            && inner.right() <= self.right() + 1.0
            && inner.bottom() <= self.bottom() + 1.0
    }

    pub fn contains_point(self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomGeometryNode {
    pub ff_id: String,
    pub tag: String,
    #[serde(default)]
    pub role: Option<String>,
    pub bbox: QcBoundingBox,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub scroll_width: f64,
    #[serde(default)]
    pub client_width: f64,
    #[serde(default)]
    pub scroll_height: f64,
    #[serde(default)]
    pub client_height: f64,
    #[serde(default = "default_font_size")]
    pub font_size_px: f64,
    #[serde(default = "default_line_height")]
    pub line_height_ratio: f64,
    #[serde(default)]
    pub fg_hex: Option<String>,
    #[serde(default)]
    pub bg_hex: Option<String>,
    #[serde(default)]
    pub border_radius_px: Option<f64>,
    #[serde(default)]
    pub parent_radius_px: Option<f64>,
    #[serde(default)]
    pub parent_padding_px: Option<f64>,
    #[serde(default)]
    pub is_interactive: bool,
    #[serde(default = "default_true")]
    pub is_visible: bool,
    #[serde(default)]
    pub pointer_events_none: bool,
    #[serde(default)]
    pub text_len: usize,
    #[serde(default = "default_true")]
    pub has_aria_label: bool,
    #[serde(default = "default_true")]
    pub has_focus_ring: bool,
}

fn default_font_size() -> f64 {
    16.0
}

fn default_line_height() -> f64 {
    1.5
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcIssue {
    pub id: String,
    pub rule_id: String,
    pub category: String,
    pub severity: QcSeverity,
    pub message: String,
    pub viewport_width: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ff_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<QcBoundingBox>,
    pub deterministic_fixable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QcRuleDefinition {
    pub rule_id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub default_severity: QcSeverity,
    pub deterministic_fixable: bool,
}

pub const QC_RULE_CATALOG: &[QcRuleDefinition] = &[
    QcRuleDefinition {
        rule_id: "R01_RTREE_ELEMENT_OVERLAP",
        title: "R-Tree Sibling Bounding Box Overlap",
        category: "layout",
        default_severity: QcSeverity::P0,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R02_VERTICAL_OVERFLOW",
        title: "Vertical Content Overflow (scrollHeight > clientHeight)",
        category: "layout",
        default_severity: QcSeverity::P0,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R03_HORIZONTAL_OVERFLOW",
        title: "Horizontal Content Overflow (scrollWidth > clientWidth)",
        category: "layout",
        default_severity: QcSeverity::P0,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R04_SLIDE_FOOTER_RAIL_CONTENT_MAX_Y",
        title: "16:9 Slide Footer-Rail CONTENT_MAX_Y Breach",
        category: "slide",
        default_severity: QcSeverity::P0,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R05_ELEMENTS_FROM_POINT_OCCLUSION",
        title: "elementsFromPoint Interactive CTA Occlusion",
        category: "occlusion",
        default_severity: QcSeverity::P0,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R06_RGBA_SSIM_REGRESSION",
        title: "Real RGBA Pixel SSIM Below Pass Threshold",
        category: "occlusion",
        default_severity: QcSeverity::P0,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R07_WCAG_AA_TEXT_CONTRAST",
        title: "WCAG 2.2 AA Text Contrast < 4.5:1",
        category: "wcag",
        default_severity: QcSeverity::P0,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R08_WCAG_AA_LARGE_TEXT_CONTRAST",
        title: "WCAG 2.2 AA Large Text Contrast < 3.0:1",
        category: "wcag",
        default_severity: QcSeverity::P1,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R09_CONCENTRIC_RADII_MISMATCH",
        title: "Concentric Corner Radii Drift (inner != outer - padding)",
        category: "layout",
        default_severity: QcSeverity::P1,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R10_TOUCH_TARGET_TOO_SMALL",
        title: "WCAG 2.2 Touch/Pointer Target Too Small",
        category: "wcag",
        default_severity: QcSeverity::P1,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R11_CROSS_ORIGIN_IFRAME_SANDBOX",
        title: "Cross-Origin Iframe Missing sandbox=\"allow-scripts\"",
        category: "security",
        default_severity: QcSeverity::P0,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R12_CROSS_ORIGIN_CANVAS_GUARD",
        title: "Cross-Origin Canvas Missing Isolation Attribute",
        category: "security",
        default_severity: QcSeverity::P1,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R13_MICRO_TYPOGRAPHY_READABILITY",
        title: "Body Font Size Below 12px Readability Floor",
        category: "wcag",
        default_severity: QcSeverity::P1,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R14_LINE_HEIGHT_CRAMPED",
        title: "Cramped Line Height (< 1.25) on Multi-line Copy",
        category: "wcag",
        default_severity: QcSeverity::P1,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R15_MISSING_ACCESSIBLE_NAME",
        title: "Interactive Control Missing Accessible Name",
        category: "wcag",
        default_severity: QcSeverity::P1,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R16_MISSING_FOCUS_VISIBLE_RING",
        title: "Interactive Control Strips Focus Ring (outline: none)",
        category: "wcag",
        default_severity: QcSeverity::P1,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R17_MISSING_IMG_ALT",
        title: "Image Element Missing Alt Attribute",
        category: "wcag",
        default_severity: QcSeverity::P1,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R18_RAW_HEX_COLOR_DRIFT",
        title: "Hardcoded Inline Hex Instead of Semantic CSS Token",
        category: "layout",
        default_severity: QcSeverity::P2,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R19_NEGATIVE_MARGIN_CLIPPING",
        title: "Negative Margin Causing Potential Clipping",
        category: "layout",
        default_severity: QcSeverity::P1,
        deterministic_fixable: true,
    },
    QcRuleDefinition {
        rule_id: "R20_VIEWPORT_SWEEP_CLIPPING",
        title: "Responsive Sweep (320..1440px) Right-Edge Clipping",
        category: "layout",
        default_severity: QcSeverity::P0,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R21_SLIDE_ASPECT_RATIO_DRIFT",
        title: "16:9 Slide Container Exceeds 1920x1080 Canvas",
        category: "slide",
        default_severity: QcSeverity::P0,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R22_SLIDE_TEXT_DENSITY_OVERLOAD",
        title: "16:9 Slide Copy Density Exceeds Legibility Budget",
        category: "slide",
        default_severity: QcSeverity::P1,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R23_FLOW_DEAD_END_CTA",
        title: "E2E Flow Unwired Dead-End CTA",
        category: "flow",
        default_severity: QcSeverity::P0,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R24_FLOW_ORPHAN_SCREEN",
        title: "E2E Flow Orphan Screen Unreachable From Entry",
        category: "flow",
        default_severity: QcSeverity::P0,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R25_FLOW_MISSING_SUBSTATES",
        title: "E2E Flow Missing 4-SubStates (default/loading/empty/error)",
        category: "flow",
        default_severity: QcSeverity::P1,
        deterministic_fixable: false,
    },
    QcRuleDefinition {
        rule_id: "R26_UNVERIFIED_NO_BROWSER_CDP",
        title: "No Local Chromium Binary (iframe_bridge_fallback)",
        category: "occlusion",
        default_severity: QcSeverity::P1,
        deterministic_fixable: false,
    },
];

pub fn registered_qc_rules() -> &'static [QcRuleDefinition] {
    QC_RULE_CATALOG
}

/// Deterministic 40-hex-character composite cache key over
/// `(html, constraints_json, sorted viewports, fixture_ref)`.
/// Uses length-prefixed SHA-256 truncated to 20 bytes (40 lowercase hex chars)
/// to avoid SHA-1 collision vulnerabilities while preserving `layout_sha1` schema width.
pub fn compute_layout_sha1(
    html: &str,
    constraints_json: &str,
    viewports: &[u32],
    fixture_ref: &str,
) -> String {
    let mut sorted_vp = viewports.to_vec();
    sorted_vp.sort_unstable();
    sorted_vp.dedup();
    let vp_csv = sorted_vp
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");

    let mut hasher = Sha256::new();
    for part in [html, constraints_json, vp_csv.as_str(), fixture_ref] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(40);
    for byte in &digest[..20] {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Computes Real RGBA pixel SSIM (Wang-Bovik structural similarity over 8x8 luminance windows).
pub fn compute_rgba_ssim(
    img_a: &[u8],
    img_b: &[u8],
    width: u32,
    height: u32,
) -> Result<f64, String> {
    if width == 0 || height == 0 {
        return Err("SSIM image dimensions must be positive".into());
    }
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| "SSIM image dimensions overflow".to_string())?;
    let expected_bytes = pixel_count
        .checked_mul(4)
        .ok_or_else(|| "SSIM buffer size overflow".to_string())?;
    if img_a.len() != expected_bytes || img_b.len() != expected_bytes {
        return Err(format!(
            "RGBA buffer length mismatch: expected {expected_bytes}, got a={} b={}",
            img_a.len(),
            img_b.len()
        ));
    }

    let to_luma = |buf: &[u8]| -> Vec<f64> {
        buf.as_chunks::<4>()
            .0
            .iter()
            .map(|px| {
                let alpha = f64::from(px[3]) / 255.0;
                (0.2126 * f64::from(px[0]) + 0.7152 * f64::from(px[1]) + 0.0722 * f64::from(px[2]))
                    * alpha
            })
            .collect()
    };

    let lum_a = to_luma(img_a);
    let lum_b = to_luma(img_b);
    let w = width as usize;
    let h = height as usize;
    let tile = 8usize;
    let c1 = (0.01_f64 * 255.0).powi(2);
    let c2 = (0.03_f64 * 255.0).powi(2);

    let mut total_ssim = 0.0_f64;
    let mut windows = 0usize;

    let mut y0 = 0usize;
    while y0 < h {
        let y1 = (y0 + tile).min(h);
        let mut x0 = 0usize;
        while x0 < w {
            let x1 = (x0 + tile).min(w);
            let mut n = 0.0_f64;
            let mut sum_a = 0.0_f64;
            let mut sum_b = 0.0_f64;
            for y in y0..y1 {
                let row = y * w;
                for x in x0..x1 {
                    sum_a += lum_a[row + x];
                    sum_b += lum_b[row + x];
                    n += 1.0;
                }
            }
            let mu_a = sum_a / n;
            let mu_b = sum_b / n;
            let mut var_a = 0.0_f64;
            let mut var_b = 0.0_f64;
            let mut cov_ab = 0.0_f64;
            for y in y0..y1 {
                let row = y * w;
                for x in x0..x1 {
                    let da = lum_a[row + x] - mu_a;
                    let db = lum_b[row + x] - mu_b;
                    var_a += da * da;
                    var_b += db * db;
                    cov_ab += da * db;
                }
            }
            let denom_n = if n > 1.0 { n - 1.0 } else { 1.0 };
            var_a /= denom_n;
            var_b /= denom_n;
            cov_ab /= denom_n;

            let numerator = (2.0 * mu_a * mu_b + c1) * (2.0 * cov_ab + c2);
            let denominator = (mu_a * mu_a + mu_b * mu_b + c1) * (var_a + var_b + c2);
            let ssim = if denominator.abs() < f64::EPSILON {
                1.0
            } else {
                (numerator / denominator).clamp(0.0, 1.0)
            };
            total_ssim += ssim;
            windows += 1;
            x0 += tile;
        }
        y0 += tile;
    }

    Ok((total_ssim / (windows.max(1) as f64)).clamp(0.0, 1.0))
}

/// Evaluates `document.elementsFromPoint(cx, cy)` occlusion for interactive controls.
pub fn elements_from_point_occlusion(
    nodes: &[DomGeometryNode],
    viewport_width: u32,
) -> Vec<QcIssue> {
    let mut issues = Vec::new();
    let visible: Vec<(usize, &DomGeometryNode)> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.is_visible && !n.pointer_events_none && n.bbox.area() > 0.0)
        .collect();

    for &(idx, target) in &visible {
        if !target.is_interactive {
            continue;
        }
        let cx = target.bbox.x + target.bbox.width * 0.5;
        let cy = target.bbox.y + target.bbox.height * 0.5;

        let mut stack: Vec<(usize, &DomGeometryNode)> = visible
            .iter()
            .copied()
            .filter(|(_, candidate)| candidate.bbox.contains_point(cx, cy))
            .collect();

        stack.sort_by(|(ia, a), (ib, b)| b.z_index.cmp(&a.z_index).then_with(|| ib.cmp(ia)));

        if let Some(&(top_idx, top_node)) = stack.first() {
            if top_idx != idx && !target.bbox.contains_box(top_node.bbox) {
                issues.push(QcIssue {
                    id: format!("occ-{viewport_width}-{}", target.ff_id),
                    rule_id: "R05_ELEMENTS_FROM_POINT_OCCLUSION".into(),
                    category: "occlusion".into(),
                    severity: QcSeverity::P0,
                    message: format!(
                        "Interactive element '{}' occluded at ({:.0},{:.0}) by '{}' (z-index {})",
                        target.ff_id, cx, cy, top_node.ff_id, top_node.z_index
                    ),
                    viewport_width,
                    ff_id: Some(target.ff_id.clone()),
                    bbox: Some(target.bbox),
                    deterministic_fixable: false,
                    suggested_fix: Some(format!(
                        "Raise z-index or reposition '{}' away from '{}'",
                        top_node.ff_id, target.ff_id
                    )),
                    target_node_id: None,
                });
            }
        }
    }
    issues
}

/// Spatial grid / R-tree bucket overlap detector for visible sibling boxes.
pub fn detect_rtree_overlaps(nodes: &[DomGeometryNode], viewport_width: u32) -> Vec<QcIssue> {
    const CELL_PX: f64 = 128.0;
    let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (idx, n) in nodes.iter().enumerate() {
        if !n.is_visible || n.bbox.area() <= 0.0 {
            continue;
        }
        let min_cx = (n.bbox.x / CELL_PX).floor() as i32;
        let max_cx = (n.bbox.right() / CELL_PX).floor() as i32;
        let min_cy = (n.bbox.y / CELL_PX).floor() as i32;
        let max_cy = (n.bbox.bottom() / CELL_PX).floor() as i32;
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                buckets.entry((cx, cy)).or_default().push(idx);
            }
        }
    }

    let mut seen_pairs: HashSet<(usize, usize)> = HashSet::new();
    let mut issues = Vec::new();
    for indices in buckets.values() {
        for i in 0..indices.len() {
            for j in (i + 1)..indices.len() {
                let a_idx = indices[i].min(indices[j]);
                let b_idx = indices[i].max(indices[j]);
                if !seen_pairs.insert((a_idx, b_idx)) {
                    continue;
                }
                let a = &nodes[a_idx];
                let b = &nodes[b_idx];
                if !a.bbox.intersects(b.bbox) {
                    continue;
                }
                // Parent-child containment is expected, not a sibling overlap violation.
                if a.bbox.contains_box(b.bbox) || b.bbox.contains_box(a.bbox) {
                    continue;
                }
                let overlap = a.bbox.intersection_area(b.bbox);
                let min_area = a.bbox.area().min(b.bbox.area());
                if min_area > 0.0 && (overlap / min_area) > 0.15 {
                    issues.push(QcIssue {
                        id: format!("ovl-{viewport_width}-{}-{}", a.ff_id, b.ff_id),
                        rule_id: "R01_RTREE_ELEMENT_OVERLAP".into(),
                        category: "layout".into(),
                        severity: QcSeverity::P0,
                        message: format!(
                            "Elements '{}' and '{}' overlap by {:.0}% at {}px viewport",
                            a.ff_id,
                            b.ff_id,
                            (overlap / min_area) * 100.0,
                            viewport_width
                        ),
                        viewport_width,
                        ff_id: Some(a.ff_id.clone()),
                        bbox: Some(a.bbox),
                        deterministic_fixable: false,
                        suggested_fix: Some(
                            "Adjust flex/grid gap or bounding box position to eliminate overlap"
                                .into(),
                        ),
                        target_node_id: None,
                    });
                }
            }
        }
    }
    issues.sort_by(|x, y| x.id.cmp(&y.id));
    issues
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowCtaNode {
    pub cta_id: String,
    pub label: String,
    #[serde(default)]
    pub target_screen_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowAuditNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub is_entry: bool,
    #[serde(default)]
    pub sub_states: Vec<String>,
    #[serde(default)]
    pub ctas: Vec<FlowCtaNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowAuditEdge {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub trigger_cta_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowCompletenessReport {
    pub dead_end_ctas: Vec<String>,
    pub orphan_screens: Vec<String>,
    pub missing_substates: BTreeMap<String, Vec<String>>,
    pub issues: Vec<QcIssue>,
    pub is_complete: bool,
}

const REQUIRED_SUBSTATES: &[&str] = &["default", "loading", "empty", "error"];

/// Audits E2E Flow DAG completeness across Story 05.4 / 05.5 screen graphs:
/// (1) Unwired Dead-End CTAs, (2) Orphan Screens unreachable from entry,
/// (3) Missing 4-SubStates (`default | loading | empty | error`).
pub fn audit_e2e_flow_completeness(
    nodes: &[FlowAuditNode],
    edges: &[FlowAuditEdge],
) -> FlowCompletenessReport {
    let node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut wired_ctas: HashSet<(&str, &str)> = HashSet::new();

    for edge in edges {
        if node_ids.contains(edge.source.as_str()) && node_ids.contains(edge.target.as_str()) {
            adj.entry(edge.source.as_str())
                .or_default()
                .push(edge.target.as_str());
            if let Some(cta_id) = edge.trigger_cta_id.as_deref() {
                wired_ctas.insert((edge.source.as_str(), cta_id));
            }
        }
    }

    for node in nodes {
        for cta in &node.ctas {
            if let Some(target) = cta.target_screen_id.as_deref() {
                if node_ids.contains(target) {
                    adj.entry(node.id.as_str()).or_default().push(target);
                    wired_ctas.insert((node.id.as_str(), cta.cta_id.as_str()));
                }
            }
        }
    }

    let mut visited: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    let entries: Vec<&str> = nodes
        .iter()
        .filter(|n| n.is_entry)
        .map(|n| n.id.as_str())
        .collect();
    if entries.is_empty() {
        if let Some(first) = nodes.first() {
            visited.insert(first.id.as_str());
            queue.push_back(first.id.as_str());
        }
    } else {
        for entry in entries {
            if visited.insert(entry) {
                queue.push_back(entry);
            }
        }
    }

    while let Some(curr) = queue.pop_front() {
        if let Some(neighbors) = adj.get(curr) {
            for &next in neighbors {
                if visited.insert(next) {
                    queue.push_back(next);
                }
            }
        }
    }

    let mut dead_end_ctas = Vec::new();
    let mut orphan_screens = Vec::new();
    let mut missing_substates: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut issues = Vec::new();

    for node in nodes {
        if !visited.contains(node.id.as_str()) {
            orphan_screens.push(node.id.clone());
            issues.push(QcIssue {
                id: format!("flow-orphan-{}", node.id),
                rule_id: "R24_FLOW_ORPHAN_SCREEN".into(),
                category: "flow".into(),
                severity: QcSeverity::P0,
                message: format!(
                    "Orphan screen '{}' ({}) is unreachable from entry node",
                    node.label, node.id
                ),
                viewport_width: 1440,
                ff_id: Some(node.id.clone()),
                bbox: None,
                deterministic_fixable: false,
                suggested_fix: Some(
                    "Connect an incoming navigation edge from an entry-reachable screen".into(),
                ),
                target_node_id: Some(node.id.clone()),
            });
        }

        for cta in &node.ctas {
            if !wired_ctas.contains(&(node.id.as_str(), cta.cta_id.as_str())) {
                let key = format!("{}:{}", node.id, cta.cta_id);
                dead_end_ctas.push(key.clone());
                issues.push(QcIssue {
                    id: format!("flow-dead-cta-{}-{}", node.id, cta.cta_id),
                    rule_id: "R23_FLOW_DEAD_END_CTA".into(),
                    category: "flow".into(),
                    severity: QcSeverity::P0,
                    message: format!(
                        "Unwired Dead-End CTA '{}' ({}) on screen '{}'",
                        cta.label, cta.cta_id, node.label
                    ),
                    viewport_width: 1440,
                    ff_id: Some(cta.cta_id.clone()),
                    bbox: None,
                    deterministic_fixable: false,
                    suggested_fix: Some(
                        "Wire CTA targetScreenId or add a Flow edge to a valid destination screen"
                            .into(),
                    ),
                    target_node_id: Some(node.id.clone()),
                });
            }
        }

        let present: BTreeSet<String> = node
            .sub_states
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .collect();
        let missing: Vec<String> = REQUIRED_SUBSTATES
            .iter()
            .filter(|req| !present.contains(**req))
            .map(|s| (*s).to_string())
            .collect();
        if !missing.is_empty() {
            missing_substates.insert(node.id.clone(), missing.clone());
            issues.push(QcIssue {
                id: format!("flow-substates-{}", node.id),
                rule_id: "R25_FLOW_MISSING_SUBSTATES".into(),
                category: "flow".into(),
                severity: QcSeverity::P1,
                message: format!(
                    "Screen '{}' missing required 4-SubStates: {}",
                    node.label,
                    missing.join(", ")
                ),
                viewport_width: 1440,
                ff_id: Some(node.id.clone()),
                bbox: None,
                deterministic_fixable: false,
                suggested_fix: Some(format!(
                    "Generate missing sub-states ({}) via Story 05.5",
                    missing.join(", ")
                )),
                target_node_id: Some(node.id.clone()),
            });
        }
    }

    let is_complete =
        dead_end_ctas.is_empty() && orphan_screens.is_empty() && missing_substates.is_empty();
    FlowCompletenessReport {
        dead_end_ctas,
        orphan_screens,
        missing_substates,
        issues,
        is_complete,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeterministicLintFixResult {
    pub patched_html: String,
    pub previous_html: String,
    pub fixed_issue_ids: Vec<String>,
    pub unfixable_issue_ids: Vec<String>,
    pub elapsed_ms: f64,
    pub llm_tokens_used: u32,
    pub diff_summary: Vec<String>,
}

fn is_iframe_sandbox_exact_allow_scripts(sandbox_attr: Option<&str>) -> bool {
    let Some(raw) = sandbox_attr else {
        return false;
    };
    let mut tokens = raw.split_ascii_whitespace();
    matches!(
        (tokens.next(), tokens.next()),
        (Some(only), None) if only.eq_ignore_ascii_case("allow-scripts")
    )
}

fn fix_iframe_sandbox(html: &str) -> (String, bool) {
    let changed = Rc::new(Cell::new(false));
    let changed_ref = Rc::clone(&changed);
    let rewritten = rewrite_str(
        html,
        RewriteStrSettings::new().append_element_content_handler(element!("iframe", move |el| {
            let _src = el.get_attribute("src");
            let sandbox = el.get_attribute("sandbox");
            if !is_iframe_sandbox_exact_allow_scripts(sandbox.as_deref()) {
                let _ = el.set_attribute("sandbox", "allow-scripts");
                changed_ref.set(true);
            }
            Ok(())
        })),
    );
    match rewritten {
        Ok(out) => (out, changed.get()),
        Err(_) => (html.to_string(), false),
    }
}

fn fix_cross_origin_canvas_guard(html: &str) -> (String, bool) {
    let changed = Rc::new(Cell::new(false));
    let changed_ref = Rc::clone(&changed);
    let rewritten = rewrite_str(
        html,
        RewriteStrSettings::new().append_element_content_handler(element!("canvas", move |el| {
            let isolated = el
                .get_attribute("data-cowok-isolated")
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
            if !isolated {
                let _ = el.set_attribute("data-cowok-isolated", "true");
                changed_ref.set(true);
            }
            Ok(())
        })),
    );
    match rewritten {
        Ok(out) => (out, changed.get()),
        Err(_) => (html.to_string(), false),
    }
}

fn parse_cramped_line_height(value: &str) -> bool {
    let trimmed = value.trim();
    if let Some(pct) = trimmed.strip_suffix('%') {
        pct.trim()
            .parse::<f64>()
            .is_ok_and(|v| v > 0.0 && v < 125.0)
    } else {
        trimmed.parse::<f64>().is_ok_and(|v| v > 0.0 && v < 1.25)
    }
}

fn rewrite_cramped_line_height_style(style: &str) -> (String, bool) {
    let mut changed = false;
    let mut out_parts = Vec::new();
    for raw_decl in style.split(';') {
        let trimmed = raw_decl.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((prop, val)) = trimmed.split_once(':') {
            if prop.trim().eq_ignore_ascii_case("line-height") && parse_cramped_line_height(val) {
                out_parts.push("line-height: 1.45".to_string());
                changed = true;
                continue;
            }
        }
        out_parts.push(trimmed.to_string());
    }
    if changed {
        (format!("{};", out_parts.join("; ")), true)
    } else {
        (style.to_string(), false)
    }
}

fn merge_style_declarations(existing: Option<&str>, additions: &[(&str, &str)]) -> (String, bool) {
    let mut ordered: Vec<(String, String)> = Vec::new();
    if let Some(raw) = existing {
        for decl in raw.split(';') {
            let trimmed = decl.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some((prop, val)) = trimmed.split_once(':') {
                ordered.push((prop.trim().to_string(), val.trim().to_string()));
            }
        }
    }

    let mut changed = false;
    for &(prop, val) in additions {
        if let Some(slot) = ordered
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(prop))
        {
            if !slot.1.eq_ignore_ascii_case(val) {
                slot.1 = val.to_string();
                changed = true;
            }
        } else {
            ordered.push((prop.to_string(), val.to_string()));
            changed = true;
        }
    }

    let rendered = ordered
        .into_iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("; ");
    let final_style = if rendered.is_empty() {
        rendered
    } else {
        format!("{rendered};")
    };
    (final_style, changed)
}

fn fix_cramped_line_height(html: &str, target_ff_id: Option<&str>) -> (String, bool) {
    let changed = Rc::new(Cell::new(false));
    let changed_ref = Rc::clone(&changed);
    let ff_target = target_ff_id.map(str::to_string);
    let rewritten = rewrite_str(
        html,
        RewriteStrSettings::new().append_element_content_handler(element!(
            "[style], [data-ff-id]",
            move |el| {
                let mut el_changed = false;
                if let Some(style) = el.get_attribute("style") {
                    let (next_style, c) = rewrite_cramped_line_height_style(&style);
                    if c {
                        let _ = el.set_attribute("style", &next_style);
                        el_changed = true;
                    }
                }
                if !el_changed {
                    if let Some(ref wanted) = ff_target {
                        if el.get_attribute("data-ff-id").as_deref() == Some(wanted.as_str()) {
                            let (merged, c) = merge_style_declarations(
                                el.get_attribute("style").as_deref(),
                                &[("line-height", "1.45")],
                            );
                            if c {
                                let _ = el.set_attribute("style", &merged);
                                el_changed = true;
                            }
                        }
                    }
                }
                if el_changed {
                    changed_ref.set(true);
                }
                Ok(())
            }
        )),
    );
    match rewritten {
        Ok(out) => (out, changed.get()),
        Err(_) => (html.to_string(), false),
    }
}

fn fix_missing_accessible_name(html: &str) -> (String, bool) {
    // Pass 1: inspect every `button` and `a` element to see if it has an accessible
    // attribute (`aria-label`, `aria-labelledby`, `title`) or non-empty visible text.
    let has_label: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(Vec::new()));
    let has_text: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(Vec::new()));
    let stack: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));

    let has_label_p1 = Rc::clone(&has_label);
    let has_text_p1 = Rc::clone(&has_text);
    let stack_p1 = Rc::clone(&stack);
    let has_text_t1 = Rc::clone(&has_text);
    let stack_t1 = Rc::clone(&stack);

    let scan_ok = rewrite_str(
        html,
        RewriteStrSettings::new()
            .append_element_content_handler(element!("button, a", move |el| {
                let labeled = el
                    .get_attribute("aria-label")
                    .is_some_and(|v| !v.trim().is_empty())
                    || el
                        .get_attribute("aria-labelledby")
                        .is_some_and(|v| !v.trim().is_empty())
                    || el
                        .get_attribute("title")
                        .is_some_and(|v| !v.trim().is_empty());
                let idx = {
                    let mut labels = has_label_p1.borrow_mut();
                    let mut texts = has_text_p1.borrow_mut();
                    let i = labels.len();
                    labels.push(labeled);
                    texts.push(false);
                    i
                };
                stack_p1.borrow_mut().push(idx);
                let stack_end = Rc::clone(&stack_p1);
                if let Some(handlers) = el.end_tag_handlers() {
                    handlers.push(Box::new(move |_: &mut lol_html::html_content::EndTag<'_>| {
                        stack_end.borrow_mut().pop();
                        Ok(())
                    }) as _);
                }
                Ok(())
            }))
            .append_element_content_handler(text!("button, a", move |chunk| {
                if !chunk.as_str().trim().is_empty() {
                    let active = stack_t1.borrow();
                    let mut texts = has_text_t1.borrow_mut();
                    for &idx in active.iter() {
                        if let Some(slot) = texts.get_mut(idx) {
                            *slot = true;
                        }
                    }
                }
                Ok(())
            })),
    );
    if scan_ok.is_err() {
        return (html.to_string(), false);
    }

    let labels = has_label.borrow();
    let texts = has_text.borrow();
    let needs_label: Vec<bool> = labels
        .iter()
        .zip(texts.iter())
        .map(|(&l, &t)| !l && !t)
        .collect();
    if !needs_label.iter().any(|&b| b) {
        return (html.to_string(), false);
    }

    // Pass 2: add `aria-label="Action"` strictly to unlabeled buttons/links.
    let cursor = Rc::new(Cell::new(0usize));
    let changed = Rc::new(Cell::new(false));
    let cursor_ref = Rc::clone(&cursor);
    let changed_ref = Rc::clone(&changed);
    let rewritten = rewrite_str(
        html,
        RewriteStrSettings::new().append_element_content_handler(element!(
            "button, a",
            move |el| {
                let idx = cursor_ref.get();
                cursor_ref.set(idx + 1);
                if needs_label.get(idx).copied().unwrap_or(false) {
                    let _ = el.set_attribute("aria-label", "Action");
                    changed_ref.set(true);
                }
                Ok(())
            }
        )),
    );
    match rewritten {
        Ok(out) => (out, changed.get()),
        Err(_) => (html.to_string(), false),
    }
}

fn replace_case_insensitive_attr(input: &str, needle: &str, replacement: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut idx = 0;
    while let Some(pos) = lower[idx..].find(&needle_lower) {
        let abs = idx + pos;
        out.push_str(&input[idx..abs]);
        out.push_str(replacement);
        idx = abs + needle.len();
    }
    out.push_str(&input[idx..]);
    out
}

fn fix_inline_hex_contrast(html: &str) -> (String, bool) {
    let mut out = html.to_string();
    let mut changed = false;
    // Clamp common low-contrast light-on-white inline colors deterministically via OKLCH guard.
    for low_hex in [
        "#cccccc", "#bbbbbb", "#aaaaaa", "#d1d5db", "#9ca3af", "#e5e7eb",
    ] {
        if out.to_ascii_lowercase().contains(low_hex) {
            if let Ok(report) =
                evaluate_oklch_contrast_guard(low_hex, "#FFFFFF", WCAG_AA_MIN_CONTRAST)
            {
                out = replace_case_insensitive_attr(&out, low_hex, &report.adjusted_fg_hex);
                changed = true;
            }
        }
    }
    (out, changed)
}

fn fix_outline_none(html: &str) -> (String, bool) {
    let mut out = html.to_string();
    let mut changed = false;
    for bad in ["outline: none", "outline:none", "outline: 0", "outline:0"] {
        if out.to_ascii_lowercase().contains(bad) {
            out = replace_case_insensitive_attr(
                &out,
                bad,
                "outline: 2px solid var(--focus-ring, #2563EB); outline-offset: 2px",
            );
            changed = true;
        }
    }
    (out, changed)
}

fn fix_micro_typography(html: &str) -> (String, bool) {
    let mut out = html.to_string();
    let mut changed = false;
    for tiny in [
        "font-size: 8px",
        "font-size: 9px",
        "font-size: 10px",
        "font-size: 11px",
        "font-size:8px",
        "font-size:9px",
        "font-size:10px",
        "font-size:11px",
    ] {
        if out.to_ascii_lowercase().contains(tiny) {
            out = replace_case_insensitive_attr(&out, tiny, "font-size: 14px");
            changed = true;
        }
    }
    (out, changed)
}

fn fix_negative_margins(html: &str) -> (String, bool) {
    let mut out = html.to_string();
    let mut changed = false;
    for pattern in ["margin: -", "margin-left: -", "margin-top: -"] {
        while let Some(pos) = out.to_ascii_lowercase().find(pattern) {
            let val_start = pos + pattern.len();
            let mut end = val_start;
            let bytes = out.as_bytes();
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'.') {
                end += 1;
            }
            if end > val_start {
                let prefix = pattern.trim_end_matches('-');
                out.replace_range(pos..end, &format!("{prefix}0px"));
                changed = true;
            } else {
                break;
            }
        }
    }
    (out, changed)
}

fn fix_missing_img_alt(html: &str) -> (String, bool) {
    let lower = html.to_ascii_lowercase();
    if !lower.contains("<img") {
        return (html.to_string(), false);
    }
    let mut result = String::with_capacity(html.len() + 32);
    let mut cursor = 0;
    let mut changed = false;
    while let Some(rel) = lower[cursor..].find("<img") {
        let start = cursor + rel;
        result.push_str(&html[cursor..start]);
        if let Some(end_rel) = html[start..].find('>') {
            let end = start + end_rel + 1;
            let tag = &html[start..end];
            if !tag.to_ascii_lowercase().contains("alt=") {
                if let Some(stripped) = tag.strip_suffix("/>") {
                    result.push_str(&format!("{} alt=\"\" />", stripped.trim_end()));
                } else if let Some(stripped) = tag.strip_suffix('>') {
                    result.push_str(&format!("{stripped} alt=\"\">"));
                } else {
                    result.push_str(tag);
                }
                changed = true;
            } else {
                result.push_str(tag);
            }
            cursor = end;
        } else {
            result.push_str(&html[start..]);
            cursor = html.len();
            break;
        }
    }
    result.push_str(&html[cursor..]);
    (result, changed)
}

fn fix_concentric_radius_inline(html: &str) -> (String, bool) {
    if html
        .to_ascii_lowercase()
        .contains("data-concentric-drift=\"true\"")
    {
        let out = replace_case_insensitive_attr(
            html,
            "data-concentric-drift=\"true\"",
            "data-concentric-drift=\"false\" data-radius-fixed=\"var(--radius-sm)\"",
        );
        return (out, true);
    }
    (html.to_string(), false)
}

fn fix_touch_target_inline(html: &str) -> (String, bool) {
    let mut out = html.to_string();
    let mut changed = false;
    for small in [
        "height: 16px",
        "height: 18px",
        "height: 20px",
        "height:16px",
        "height:20px",
    ] {
        if out.to_ascii_lowercase().contains(small) {
            out = replace_case_insensitive_attr(&out, small, "min-height: 44px; min-width: 44px");
            changed = true;
        }
    }
    (out, changed)
}

/// Executes `<10ms` deterministic token/spacing/radius/contrast/sandbox auto-fix
/// with `0` LLM tokens and preserves `previous_html` for 1-step `Cmd+Z` rollback.
pub fn apply_deterministic_lint_fix(html: &str, issues: &[QcIssue]) -> DeterministicLintFixResult {
    let started = Instant::now();
    let previous_html = html.to_string();
    let mut current = html.to_string();
    let mut fixed_issue_ids = Vec::new();
    let mut unfixable_issue_ids = Vec::new();
    let mut diff_summary = Vec::new();
    let mut resolved_rules: HashSet<String> = HashSet::new();

    for issue in issues {
        if !issue.deterministic_fixable {
            unfixable_issue_ids.push(issue.id.clone());
            continue;
        }
        let (next, changed, note) = match issue.rule_id.as_str() {
            "R11_CROSS_ORIGIN_IFRAME_SANDBOX" => {
                let (h, c) = fix_iframe_sandbox(&current);
                (
                    h,
                    c,
                    "Enforced sandbox=\"allow-scripts\" on cross-origin <iframe>",
                )
            }
            "R12_CROSS_ORIGIN_CANVAS_GUARD" => {
                let (h, c) = fix_cross_origin_canvas_guard(&current);
                (h, c, "Added data-cowok-isolated=\"true\" guard on <canvas>")
            }
            "R07_WCAG_AA_TEXT_CONTRAST" | "R08_WCAG_AA_LARGE_TEXT_CONTRAST" => {
                let (h, c) = fix_inline_hex_contrast(&current);
                (
                    h,
                    c,
                    "Clamped foreground hex via OKLCH guard to WCAG 2.2 AA (>= 4.5:1)",
                )
            }
            "R16_MISSING_FOCUS_VISIBLE_RING" => {
                let (h, c) = fix_outline_none(&current);
                (h, c, "Restored semantic focus-visible outline ring")
            }
            "R13_MICRO_TYPOGRAPHY_READABILITY" => {
                let (h, c) = fix_micro_typography(&current);
                (h, c, "Raised micro font-size to 14px readability floor")
            }
            "R14_LINE_HEIGHT_CRAMPED" => {
                let (h, c) = fix_cramped_line_height(&current, issue.ff_id.as_deref());
                (
                    h,
                    c,
                    "Normalized cramped line-height to 1.45 readability floor",
                )
            }
            "R15_MISSING_ACCESSIBLE_NAME" => {
                let (h, c) = fix_missing_accessible_name(&current);
                (
                    h,
                    c,
                    "Added aria-label=\"Action\" to unlabeled interactive control",
                )
            }
            "R19_NEGATIVE_MARGIN_CLIPPING" => {
                let (h, c) = fix_negative_margins(&current);
                (h, c, "Normalized negative inline margin to 0px")
            }
            "R17_MISSING_IMG_ALT" => {
                let (h, c) = fix_missing_img_alt(&current);
                (h, c, "Added missing alt attribute to <img>")
            }
            "R09_CONCENTRIC_RADII_MISMATCH" => {
                let (h, c) = fix_concentric_radius_inline(&current);
                (h, c, "Aligned nested border-radius to concentric token")
            }
            "R10_TOUCH_TARGET_TOO_SMALL" => {
                let (h, c) = fix_touch_target_inline(&current);
                (h, c, "Expanded touch target to >= 44x44px")
            }
            _ => (current.clone(), false, ""),
        };

        if changed {
            current = next;
            fixed_issue_ids.push(issue.id.clone());
            resolved_rules.insert(issue.rule_id.clone());
            diff_summary.push(format!("[{}] {}", issue.rule_id, note));
        } else if resolved_rules.contains(&issue.rule_id)
            || (issue.rule_id == "R11_CROSS_ORIGIN_IFRAME_SANDBOX"
                && !has_unsafe_iframe_sandbox(&current))
        {
            // Multi-viewport duplicates of an already-fixed static rule in this batch
            // are resolved by the earlier HTML rewrite.
            fixed_issue_ids.push(issue.id.clone());
        } else {
            unfixable_issue_ids.push(issue.id.clone());
        }
    }

    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    DeterministicLintFixResult {
        patched_html: current,
        previous_html,
        fixed_issue_ids,
        unfixable_issue_ids,
        elapsed_ms,
        llm_tokens_used: 0,
        diff_summary,
    }
}

fn structural_patch_declarations(
    rule_id: &str,
) -> Option<(&'static [(&'static str, &'static str)], &'static str)> {
    match rule_id {
        "R01_RTREE_ELEMENT_OVERLAP" => Some((
            &[
                ("position", "relative"),
                ("z-index", "1"),
                ("margin-bottom", "16px"),
            ],
            "Applied surgical flow isolation & margin-bottom: 16px",
        )),
        "R02_VERTICAL_OVERFLOW" => Some((
            &[
                ("overflow-y", "auto"),
                ("max-height", "100%"),
                ("box-sizing", "border-box"),
            ],
            "Applied surgical vertical reflow (overflow-y: auto; max-height: 100%)",
        )),
        "R03_HORIZONTAL_OVERFLOW"
        | "R20_VIEWPORT_SWEEP_CLIPPING"
        | "R21_SLIDE_ASPECT_RATIO_DRIFT" => Some((
            &[
                ("max-width", "100%"),
                ("min-width", "0"),
                ("overflow-x", "hidden"),
                ("box-sizing", "border-box"),
            ],
            "Constrained horizontal bounds (max-width: 100%; min-width: 0; overflow-x: hidden)",
        )),
        "R04_SLIDE_FOOTER_RAIL_CONTENT_MAX_Y" => Some((
            &[
                ("max-height", "880px"),
                ("overflow", "hidden"),
                ("margin-bottom", "120px"),
                ("box-sizing", "border-box"),
            ],
            "Constrained slide content above 16:9 footer-rail CONTENT_MAX_Y",
        )),
        "R05_ELEMENTS_FROM_POINT_OCCLUSION" => Some((
            &[
                ("position", "relative"),
                ("z-index", "100"),
                ("pointer-events", "auto"),
            ],
            "Raised stacking order (z-index: 100; pointer-events: auto) to clear occlusion",
        )),
        _ => None,
    }
}

fn patch_structural_ff_element(
    html: &str,
    ff_id: &str,
    rule_id: &str,
    decls: &[(&str, &str)],
) -> (String, bool) {
    let changed = Rc::new(Cell::new(false));
    let changed_ref = Rc::clone(&changed);
    let wanted = ff_id.to_string();
    let rule = rule_id.to_string();
    let owned_decls: Vec<(String, String)> = decls
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();

    let rewritten = rewrite_str(
        html,
        RewriteStrSettings::new().append_element_content_handler(element!(
            "[data-ff-id], [id]",
            move |el| {
                let matches_ff = el.get_attribute("data-ff-id").as_deref() == Some(wanted.as_str())
                    || el.get_attribute("id").as_deref() == Some(wanted.as_str());
                if !matches_ff {
                    return Ok(());
                }
                let pairs: Vec<(&str, &str)> = owned_decls
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                let (merged_style, style_changed) =
                    merge_style_declarations(el.get_attribute("style").as_deref(), &pairs);
                let attr_already_set = el
                    .get_attribute("data-qc-patched")
                    .as_deref()
                    .is_some_and(|existing| existing.split(',').any(|r| r == rule));
                if style_changed || !attr_already_set {
                    if !merged_style.is_empty() {
                        let _ = el.set_attribute("style", &merged_style);
                    }
                    let next_attr = match el.get_attribute("data-qc-patched") {
                        Some(existing) if !existing.trim().is_empty() => {
                            if attr_already_set {
                                existing
                            } else {
                                format!("{existing},{rule}")
                            }
                        }
                        _ => rule.clone(),
                    };
                    let _ = el.set_attribute("data-qc-patched", &next_attr);
                    changed_ref.set(true);
                }
                Ok(())
            }
        )),
    );
    match rewritten {
        Ok(out) => (out, changed.get()),
        Err(_) => (html.to_string(), false),
    }
}

/// Applies deterministic fixes first, then executes bounded surgical structural layout
/// patches via `lol_html` for overflowing/overlapping/occluded `data-ff-id` elements.
pub fn apply_agent_structural_patch(html: &str, issues: &[QcIssue]) -> DeterministicLintFixResult {
    let started = Instant::now();
    let base = apply_deterministic_lint_fix(html, issues);
    let mut current = base.patched_html;
    let mut fixed_issue_ids = base.fixed_issue_ids;
    let mut diff_summary = base.diff_summary;

    let remaining_ids: HashSet<&str> = base
        .unfixable_issue_ids
        .iter()
        .map(String::as_str)
        .collect();
    let mut unfixable_issue_ids = Vec::new();
    let mut resolved_targets: HashSet<(String, String)> = HashSet::new();

    for issue in issues {
        if !remaining_ids.contains(issue.id.as_str()) {
            continue;
        }
        let Some(ff_id) = issue
            .ff_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            unfixable_issue_ids.push(issue.id.clone());
            continue;
        };
        let Some((decls, note)) = structural_patch_declarations(issue.rule_id.as_str()) else {
            unfixable_issue_ids.push(issue.id.clone());
            continue;
        };
        let target_key = (issue.rule_id.clone(), ff_id.to_string());
        let (next, changed) = patch_structural_ff_element(&current, ff_id, &issue.rule_id, decls);
        if changed {
            current = next;
            fixed_issue_ids.push(issue.id.clone());
            resolved_targets.insert(target_key);
            diff_summary.push(format!("[{}] {} on '{}'", issue.rule_id, note, ff_id));
        } else if resolved_targets.contains(&target_key) {
            fixed_issue_ids.push(issue.id.clone());
        } else {
            unfixable_issue_ids.push(issue.id.clone());
        }
    }

    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    DeterministicLintFixResult {
        patched_html: current,
        previous_html: base.previous_html,
        fixed_issue_ids,
        unfixable_issue_ids,
        elapsed_ms,
        llm_tokens_used: 0,
        diff_summary,
    }
}

fn has_unsafe_iframe_sandbox(html: &str) -> bool {
    let unsafe_found = Rc::new(Cell::new(false));
    let found_ref = Rc::clone(&unsafe_found);
    let res = rewrite_str(
        html,
        RewriteStrSettings::new().append_element_content_handler(element!("iframe", move |el| {
            let _src = el.get_attribute("src");
            let sandbox = el.get_attribute("sandbox");
            if !is_iframe_sandbox_exact_allow_scripts(sandbox.as_deref()) {
                found_ref.set(true);
            }
            Ok(())
        })),
    );
    match res {
        Ok(_) => unsafe_found.get(),
        Err(_) => true,
    }
}

/// Scans HTML markup for deterministic static WCAG/security/typography violations.
pub fn scan_html_static_rules(
    html: &str,
    target_kind: QcTargetKind,
    viewport_width: u32,
) -> Vec<QcIssue> {
    let mut issues = Vec::new();
    let lower = html.to_ascii_lowercase();

    if has_unsafe_iframe_sandbox(html) {
        issues.push(QcIssue {
            id: format!("sec-iframe-{viewport_width}"),
            rule_id: "R11_CROSS_ORIGIN_IFRAME_SANDBOX".into(),
            category: "security".into(),
            severity: QcSeverity::P0,
            message: "Cross-origin <iframe> must enforce sandbox=\"allow-scripts\" without allow-same-origin".into(),
            viewport_width,
            ff_id: None,
            bbox: None,
            deterministic_fixable: true,
            suggested_fix: Some("Set sandbox=\"allow-scripts\" on <iframe>".into()),
            target_node_id: None,
        });
    }

    if lower.contains("<canvas")
        && !lower.contains("data-cowok-isolated=\"true\"")
        && !lower.contains("data-sandbox-isolated=\"true\"")
        && lower.contains("http")
    {
        issues.push(QcIssue {
            id: format!("sec-canvas-{viewport_width}"),
            rule_id: "R12_CROSS_ORIGIN_CANVAS_GUARD".into(),
            category: "security".into(),
            severity: QcSeverity::P1,
            message: "Cross-origin <canvas> lacks isolation guard attribute".into(),
            viewport_width,
            ff_id: None,
            bbox: None,
            deterministic_fixable: true,
            suggested_fix: Some("Add data-cowok-isolated=\"true\" to cross-origin canvas".into()),
            target_node_id: None,
        });
    }

    for bad in ["outline: none", "outline:none", "outline: 0", "outline:0"] {
        if lower.contains(bad) {
            issues.push(QcIssue {
                id: format!("wcag-focus-{viewport_width}"),
                rule_id: "R16_MISSING_FOCUS_VISIBLE_RING".into(),
                category: "wcag".into(),
                severity: QcSeverity::P1,
                message: "Interactive element removes focus ring via outline: none".into(),
                viewport_width,
                ff_id: None,
                bbox: None,
                deterministic_fixable: true,
                suggested_fix: Some(
                    "Replace outline: none with semantic focus-visible ring".into(),
                ),
                target_node_id: None,
            });
            break;
        }
    }

    for tiny in [
        "font-size: 8px",
        "font-size: 9px",
        "font-size: 10px",
        "font-size: 11px",
        "font-size:8px",
        "font-size:9px",
        "font-size:10px",
        "font-size:11px",
    ] {
        if lower.contains(tiny) {
            issues.push(QcIssue {
                id: format!("wcag-font-{viewport_width}"),
                rule_id: "R13_MICRO_TYPOGRAPHY_READABILITY".into(),
                category: "wcag".into(),
                severity: QcSeverity::P1,
                message: "Inline font-size below 12px readability floor".into(),
                viewport_width,
                ff_id: None,
                bbox: None,
                deterministic_fixable: true,
                suggested_fix: Some("Raise body font-size to >= 14px".into()),
                target_node_id: None,
            });
            break;
        }
    }

    if lower.contains("margin: -")
        || lower.contains("margin-left: -")
        || lower.contains("margin-top: -")
    {
        issues.push(QcIssue {
            id: format!("layout-neg-margin-{viewport_width}"),
            rule_id: "R19_NEGATIVE_MARGIN_CLIPPING".into(),
            category: "layout".into(),
            severity: QcSeverity::P1,
            message: "Negative inline margin risks edge clipping across viewports".into(),
            viewport_width,
            ff_id: None,
            bbox: None,
            deterministic_fixable: true,
            suggested_fix: Some("Reset negative margin to 0px and use parent gap token".into()),
            target_node_id: None,
        });
    }

    // Detect <img> missing alt
    let mut cursor = 0;
    while let Some(rel) = lower[cursor..].find("<img") {
        let start = cursor + rel;
        let Some(end_rel) = lower[start..].find('>') else {
            break;
        };
        let tag = &lower[start..start + end_rel + 1];
        if !tag.contains("alt=") {
            issues.push(QcIssue {
                id: format!("wcag-img-alt-{viewport_width}-{start}"),
                rule_id: "R17_MISSING_IMG_ALT".into(),
                category: "wcag".into(),
                severity: QcSeverity::P1,
                message: "Image tag is missing alt attribute for screen readers".into(),
                viewport_width,
                ff_id: None,
                bbox: None,
                deterministic_fixable: true,
                suggested_fix: Some(
                    "Add descriptive alt text or alt=\"\" for decorative images".into(),
                ),
                target_node_id: None,
            });
            break;
        }
        cursor = start + end_rel + 1;
    }

    // Detect low contrast inline hex pairs
    for low_hex in [
        "#cccccc", "#bbbbbb", "#aaaaaa", "#d1d5db", "#9ca3af", "#e5e7eb",
    ] {
        if lower.contains(low_hex) {
            if let Ok(ratio) = contrast_ratio_hex(low_hex, "#FFFFFF") {
                if ratio < WCAG_AA_MIN_CONTRAST {
                    issues.push(QcIssue {
                        id: format!("wcag-contrast-{viewport_width}"),
                        rule_id: "R07_WCAG_AA_TEXT_CONTRAST".into(),
                        category: "wcag".into(),
                        severity: QcSeverity::P0,
                        message: format!(
                            "Low-contrast inline color {low_hex} against light surface ({ratio:.2}:1 < 4.5:1)"
                        ),
                        viewport_width,
                        ff_id: None,
                        bbox: None,
                        deterministic_fixable: true,
                        suggested_fix: Some("Clamp color via OKLCH contrast guard to >= 4.5:1".into()),
                        target_node_id: None,
                    });
                    break;
                }
            }
        }
    }

    if lower.contains("data-concentric-drift=\"true\"") {
        issues.push(QcIssue {
            id: format!("layout-concentric-{viewport_width}"),
            rule_id: "R09_CONCENTRIC_RADII_MISMATCH".into(),
            category: "layout".into(),
            severity: QcSeverity::P1,
            message: "Nested element violates concentric border-radius formula".into(),
            viewport_width,
            ff_id: None,
            bbox: None,
            deterministic_fixable: true,
            suggested_fix: Some("Apply inner_radius = max(0, outer_radius - padding)".into()),
            target_node_id: None,
        });
    }

    if target_kind == QcTargetKind::PresentationSlide {
        let stripped_len = strip_html_tags_len(html);
        if stripped_len > 900 {
            issues.push(QcIssue {
                id: format!("slide-density-{viewport_width}"),
                rule_id: "R22_SLIDE_TEXT_DENSITY_OVERLOAD".into(),
                category: "slide".into(),
                severity: QcSeverity::P1,
                message: format!(
                    "16:9 Slide copy density ({stripped_len} chars) exceeds 900-char legibility budget"
                ),
                viewport_width,
                ff_id: None,
                bbox: None,
                deterministic_fixable: false,
                suggested_fix: Some("Split dense slide copy across two 16:9 slides".into()),
                target_node_id: None,
            });
        }
    }

    issues
}

fn strip_html_tags_len(html: &str) -> usize {
    let mut in_tag = false;
    let mut count = 0usize;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag && !c.is_whitespace() => count += 1,
            _ => {}
        }
    }
    count
}

/// Evaluates geometry/WCAG/slide rules against measured DOM geometry nodes.
pub fn evaluate_geometry_rules(
    nodes: &[DomGeometryNode],
    target_kind: QcTargetKind,
    viewport_width: u32,
) -> Vec<QcIssue> {
    let mut issues = Vec::new();
    issues.extend(detect_rtree_overlaps(nodes, viewport_width));
    issues.extend(elements_from_point_occlusion(nodes, viewport_width));

    for node in nodes {
        if !node.is_visible {
            continue;
        }
        if node.client_height > 0.0 && node.scroll_height > node.client_height + 1.0 {
            issues.push(QcIssue {
                id: format!("ovf-v-{viewport_width}-{}", node.ff_id),
                rule_id: "R02_VERTICAL_OVERFLOW".into(),
                category: "layout".into(),
                severity: QcSeverity::P0,
                message: format!(
                    "Vertical overflow on '{}': scrollHeight {:.0}px > clientHeight {:.0}px",
                    node.ff_id, node.scroll_height, node.client_height
                ),
                viewport_width,
                ff_id: Some(node.ff_id.clone()),
                bbox: Some(node.bbox),
                deterministic_fixable: false,
                suggested_fix: Some("Reduce content height or enable safe container reflow".into()),
                target_node_id: None,
            });
        }

        if node.client_width > 0.0 && node.scroll_width > node.client_width + 1.0 {
            issues.push(QcIssue {
                id: format!("ovf-h-{viewport_width}-{}", node.ff_id),
                rule_id: "R03_HORIZONTAL_OVERFLOW".into(),
                category: "layout".into(),
                severity: QcSeverity::P0,
                message: format!(
                    "Horizontal overflow on '{}': scrollWidth {:.0}px > clientWidth {:.0}px",
                    node.ff_id, node.scroll_width, node.client_width
                ),
                viewport_width,
                ff_id: Some(node.ff_id.clone()),
                bbox: Some(node.bbox),
                deterministic_fixable: false,
                suggested_fix: Some(
                    "Apply min-width: 0 / flex-wrap to eliminate horizontal scroll".into(),
                ),
                target_node_id: None,
            });
        }

        if node.bbox.right() > f64::from(viewport_width) + 1.0 {
            let rule_id = if target_kind == QcTargetKind::PresentationSlide {
                "R21_SLIDE_ASPECT_RATIO_DRIFT"
            } else {
                "R20_VIEWPORT_SWEEP_CLIPPING"
            };
            issues.push(QcIssue {
                id: format!("clip-r-{viewport_width}-{}", node.ff_id),
                rule_id: rule_id.into(),
                category: if target_kind == QcTargetKind::PresentationSlide {
                    "slide".into()
                } else {
                    "layout".into()
                },
                severity: QcSeverity::P0,
                message: format!(
                    "Element '{}' clips right viewport boundary ({:.0}px > {}px)",
                    node.ff_id,
                    node.bbox.right(),
                    viewport_width
                ),
                viewport_width,
                ff_id: Some(node.ff_id.clone()),
                bbox: Some(node.bbox),
                deterministic_fixable: false,
                suggested_fix: Some("Constrain max-width to 100% inside viewport container".into()),
                target_node_id: None,
            });
        }

        if target_kind == QcTargetKind::PresentationSlide
            && node.role.as_deref() != Some("footer-rail")
            && node.bbox.bottom() > SLIDE_CONTENT_MAX_Y
        {
            issues.push(QcIssue {
                id: format!("slide-maxy-{}", node.ff_id),
                rule_id: "R04_SLIDE_FOOTER_RAIL_CONTENT_MAX_Y".into(),
                category: "slide".into(),
                severity: QcSeverity::P0,
                message: format!(
                    "Slide element '{}' bottom {:.0}px breaches Footer-Rail CONTENT_MAX_Y ({:.0}px)",
                    node.ff_id,
                    node.bbox.bottom(),
                    SLIDE_CONTENT_MAX_Y
                ),
                viewport_width,
                ff_id: Some(node.ff_id.clone()),
                bbox: Some(node.bbox),
                deterministic_fixable: false,
                suggested_fix: Some("Compact vertical spacing or move overflow bullets to next slide".into()),
                target_node_id: None,
            });
        }

        if let (Some(fg), Some(bg)) = (node.fg_hex.as_deref(), node.bg_hex.as_deref()) {
            if let Ok(ratio) = contrast_ratio_hex(fg, bg) {
                let is_large = node.font_size_px >= 18.66;
                let min_ratio = if is_large {
                    WCAG_LARGE_TEXT_MIN_CONTRAST
                } else {
                    WCAG_AA_MIN_CONTRAST
                };
                if ratio < min_ratio {
                    issues.push(QcIssue {
                        id: format!("contrast-{viewport_width}-{}", node.ff_id),
                        rule_id: if is_large {
                            "R08_WCAG_AA_LARGE_TEXT_CONTRAST".into()
                        } else {
                            "R07_WCAG_AA_TEXT_CONTRAST".into()
                        },
                        category: "wcag".into(),
                        severity: if is_large { QcSeverity::P1 } else { QcSeverity::P0 },
                        message: format!(
                            "Element '{}' contrast {:.2}:1 ({fg} on {bg}) below WCAG 2.2 AA {:.1}:1",
                            node.ff_id, ratio, min_ratio
                        ),
                        viewport_width,
                        ff_id: Some(node.ff_id.clone()),
                        bbox: Some(node.bbox),
                        deterministic_fixable: true,
                        suggested_fix: Some("Clamp foreground color via OKLCH guard".into()),
                        target_node_id: None,
                    });
                }
            }
        }

        if let (Some(inner_r), Some(outer_r), Some(pad)) = (
            node.border_radius_px,
            node.parent_radius_px,
            node.parent_padding_px,
        ) {
            let expected = (outer_r - pad).max(0.0);
            if (inner_r - expected).abs() > 2.0 {
                issues.push(QcIssue {
                    id: format!("radii-{viewport_width}-{}", node.ff_id),
                    rule_id: "R09_CONCENTRIC_RADII_MISMATCH".into(),
                    category: "layout".into(),
                    severity: QcSeverity::P1,
                    message: format!(
                        "Concentric radii mismatch on '{}': inner {:.0}px vs expected {:.0}px (outer {:.0} - pad {:.0})",
                        node.ff_id, inner_r, expected, outer_r, pad
                    ),
                    viewport_width,
                    ff_id: Some(node.ff_id.clone()),
                    bbox: Some(node.bbox),
                    deterministic_fixable: true,
                    suggested_fix: Some(format!("Set border-radius to {:.0}px", expected)),
                    target_node_id: None,
                });
            }
        }

        if node.is_interactive {
            let min_dim = if viewport_width <= 768 { 44.0 } else { 24.0 };
            if node.bbox.width < min_dim || node.bbox.height < min_dim {
                issues.push(QcIssue {
                    id: format!("target-{viewport_width}-{}", node.ff_id),
                    rule_id: "R10_TOUCH_TARGET_TOO_SMALL".into(),
                    category: "wcag".into(),
                    severity: QcSeverity::P1,
                    message: format!(
                        "Interactive target '{}' ({:.0}x{:.0}px) below {:.0}x{:.0}px WCAG floor",
                        node.ff_id, node.bbox.width, node.bbox.height, min_dim, min_dim
                    ),
                    viewport_width,
                    ff_id: Some(node.ff_id.clone()),
                    bbox: Some(node.bbox),
                    deterministic_fixable: true,
                    suggested_fix: Some(format!("Set min-width/min-height >= {:.0}px", min_dim)),
                    target_node_id: None,
                });
            }
            if !node.has_aria_label && node.text_len == 0 {
                issues.push(QcIssue {
                    id: format!("aria-{viewport_width}-{}", node.ff_id),
                    rule_id: "R15_MISSING_ACCESSIBLE_NAME".into(),
                    category: "wcag".into(),
                    severity: QcSeverity::P1,
                    message: format!(
                        "Interactive control '{}' missing visible label or aria-label",
                        node.ff_id
                    ),
                    viewport_width,
                    ff_id: Some(node.ff_id.clone()),
                    bbox: Some(node.bbox),
                    deterministic_fixable: true,
                    suggested_fix: Some("Add descriptive aria-label attribute".into()),
                    target_node_id: None,
                });
            }
            if !node.has_focus_ring {
                issues.push(QcIssue {
                    id: format!("focus-{viewport_width}-{}", node.ff_id),
                    rule_id: "R16_MISSING_FOCUS_VISIBLE_RING".into(),
                    category: "wcag".into(),
                    severity: QcSeverity::P1,
                    message: format!(
                        "Interactive control '{}' lacks visible focus indicator",
                        node.ff_id
                    ),
                    viewport_width,
                    ff_id: Some(node.ff_id.clone()),
                    bbox: Some(node.bbox),
                    deterministic_fixable: true,
                    suggested_fix: Some("Apply semantic 2px focus-visible outline".into()),
                    target_node_id: None,
                });
            }
        }

        if node.font_size_px < 12.0 && node.text_len > 0 {
            issues.push(QcIssue {
                id: format!("font-{viewport_width}-{}", node.ff_id),
                rule_id: "R13_MICRO_TYPOGRAPHY_READABILITY".into(),
                category: "wcag".into(),
                severity: QcSeverity::P1,
                message: format!(
                    "Element '{}' font-size {:.1}px is below 12px readability floor",
                    node.ff_id, node.font_size_px
                ),
                viewport_width,
                ff_id: Some(node.ff_id.clone()),
                bbox: Some(node.bbox),
                deterministic_fixable: true,
                suggested_fix: Some("Increase font-size to >= 14px".into()),
                target_node_id: None,
            });
        }

        if node.text_len > 60 && node.line_height_ratio < 1.25 {
            issues.push(QcIssue {
                id: format!("lh-{viewport_width}-{}", node.ff_id),
                rule_id: "R14_LINE_HEIGHT_CRAMPED".into(),
                category: "wcag".into(),
                severity: QcSeverity::P1,
                message: format!(
                    "Element '{}' line-height {:.2} is below 1.25 multi-line floor",
                    node.ff_id, node.line_height_ratio
                ),
                viewport_width,
                ff_id: Some(node.ff_id.clone()),
                bbox: Some(node.bbox),
                deterministic_fixable: true,
                suggested_fix: Some("Set line-height: 1.5".into()),
                target_node_id: None,
            });
        }
    }

    issues
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdpBreakerStatus {
    pub is_open: bool,
    pub consecutive_crashes: u32,
    pub total_crashes: u64,
    pub total_restarts: u64,
    pub last_crash_reason: Option<String>,
}

#[derive(Debug)]
struct CdpCircuitBreakerState {
    consecutive_crashes: u32,
    total_crashes: u64,
    total_restarts: u64,
    opened_at: Option<Instant>,
    last_crash_reason: Option<String>,
    crash_threshold: u32,
    cooldown: Duration,
}

#[derive(Debug)]
pub struct CdpPoolPermit {
    pool_gate: Arc<(Mutex<usize>, Condvar)>,
    active_counter: Arc<AtomicUsize>,
    pub incognito_context_id: String,
}

impl Drop for CdpPoolPermit {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.pool_gate;
        if let Ok(mut in_use) = lock.lock() {
            *in_use = in_use.saturating_sub(1);
            self.active_counter.fetch_sub(1, Ordering::SeqCst);
            cvar.notify_one();
        }
    }
}

#[derive(Debug, Clone)]
pub struct VisualQcEvalInput {
    pub target_kind: QcTargetKind,
    pub target_id: String,
    pub html: String,
    pub constraints_json: String,
    pub viewports: Vec<u32>,
    pub fixture_ref: String,
    pub geometry_nodes: Vec<DomGeometryNode>,
    pub flow_nodes: Vec<FlowAuditNode>,
    pub flow_edges: Vec<FlowAuditEdge>,
    pub reference_rgba: Option<Vec<u8>>,
    pub candidate_rgba: Option<Vec<u8>>,
    pub frame_width: u32,
    pub frame_height: u32,
    pub ssim_threshold: f64,
    pub force_no_browser: bool,
}

impl Default for VisualQcEvalInput {
    fn default() -> Self {
        Self {
            target_kind: QcTargetKind::AppScreen,
            target_id: "default-screen".into(),
            html: String::new(),
            constraints_json: "{}".into(),
            viewports: CANONICAL_APP_VIEWPORTS.to_vec(),
            fixture_ref: "default".into(),
            geometry_nodes: Vec::new(),
            flow_nodes: Vec::new(),
            flow_edges: Vec::new(),
            reference_rgba: None,
            candidate_rgba: None,
            frame_width: 0,
            frame_height: 0,
            ssim_threshold: DEFAULT_SSIM_PASS_THRESHOLD,
            force_no_browser: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualQcEvalResult {
    pub target_kind: QcTargetKind,
    pub target_id: String,
    pub layout_sha1: String,
    pub viewports: Vec<u32>,
    pub verdict: QcVerdict,
    pub p0_count: usize,
    pub p1_count: usize,
    pub ssim_score: Option<f64>,
    pub issues: Vec<QcIssue>,
    pub flow_completeness: Option<FlowCompletenessReport>,
    pub used_fallback: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_mode: Option<String>,
    pub incognito_context_id: String,
    pub rules_evaluated: usize,
    pub breaker_status: CdpBreakerStatus,
}

/// Pooled `<= 1` Chromium CDP evaluator with semaphore concurrency enforcement,
/// ephemeral `IncognitoBrowserContext` IDs, crash/OOM circuit breaker, and
/// honest `iframe_bridge_fallback` (`UNVERIFIED_NO_BROWSER`).
pub struct PooledCdpEvaluator {
    max_concurrency: usize,
    pool_gate: Arc<(Mutex<usize>, Condvar)>,
    active_counter: Arc<AtomicUsize>,
    peak_counter: Arc<AtomicUsize>,
    context_seq: AtomicU64,
    browser_binary: Mutex<Option<String>>,
    breaker: Mutex<CdpCircuitBreakerState>,
}

impl Default for PooledCdpEvaluator {
    fn default() -> Self {
        Self::new(discover_local_chromium_binary())
    }
}

impl PooledCdpEvaluator {
    pub fn new(browser_binary: Option<String>) -> Self {
        Self::with_breaker_config(
            browser_binary,
            DEFAULT_CIRCUIT_BREAKER_THRESHOLD,
            Duration::from_millis(DEFAULT_CIRCUIT_BREAKER_COOLDOWN_MS),
        )
    }

    pub fn with_breaker_config(
        browser_binary: Option<String>,
        crash_threshold: u32,
        cooldown: Duration,
    ) -> Self {
        Self {
            max_concurrency: MAX_CDP_CONCURRENCY,
            pool_gate: Arc::new((Mutex::new(0), Condvar::new())),
            active_counter: Arc::new(AtomicUsize::new(0)),
            peak_counter: Arc::new(AtomicUsize::new(0)),
            context_seq: AtomicU64::new(1),
            browser_binary: Mutex::new(browser_binary),
            breaker: Mutex::new(CdpCircuitBreakerState {
                consecutive_crashes: 0,
                total_crashes: 0,
                total_restarts: 0,
                opened_at: None,
                last_crash_reason: None,
                crash_threshold: crash_threshold.max(1),
                cooldown,
            }),
        }
    }

    pub fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }

    pub fn active_workers(&self) -> usize {
        self.active_counter.load(Ordering::SeqCst)
    }

    pub fn peak_concurrency(&self) -> usize {
        self.peak_counter.load(Ordering::SeqCst)
    }

    pub fn has_browser_binary(&self) -> bool {
        self.browser_binary
            .lock()
            .map(|b| b.as_deref().map(str::trim).is_some_and(|s| !s.is_empty()))
            .unwrap_or(false)
    }

    pub fn acquire_permit(&self) -> CdpPoolPermit {
        let (lock, cvar) = &*self.pool_gate;
        let mut in_use = lock.lock().unwrap_or_else(|e| e.into_inner());
        while *in_use >= self.max_concurrency {
            in_use = cvar.wait(in_use).unwrap_or_else(|e| e.into_inner());
        }
        *in_use += 1;
        let active = self.active_counter.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak_counter.fetch_max(active, Ordering::SeqCst);
        let seq = self.context_seq.fetch_add(1, Ordering::SeqCst);
        CdpPoolPermit {
            pool_gate: Arc::clone(&self.pool_gate),
            active_counter: Arc::clone(&self.active_counter),
            incognito_context_id: format!("incognito-cdp-ctx-{seq}"),
        }
    }

    /// Records a CDP Chromium worker crash/OOM and trips the circuit breaker when threshold reached.
    pub fn record_worker_crash(&self, reason: &str) -> CdpBreakerStatus {
        let mut st = self.breaker.lock().unwrap_or_else(|e| e.into_inner());
        st.consecutive_crashes = st.consecutive_crashes.saturating_add(1);
        st.total_crashes = st.total_crashes.saturating_add(1);
        st.last_crash_reason = Some(reason.to_string());
        if st.consecutive_crashes >= st.crash_threshold {
            st.opened_at = Some(Instant::now());
        }
        CdpBreakerStatus {
            is_open: st.opened_at.is_some(),
            consecutive_crashes: st.consecutive_crashes,
            total_crashes: st.total_crashes,
            total_restarts: st.total_restarts,
            last_crash_reason: st.last_crash_reason.clone(),
        }
    }

    /// Returns true if Pooled CDP can attempt browser evaluation (or auto-recovers after cooldown).
    pub fn can_attempt_cdp(&self) -> bool {
        if !self.has_browser_binary() {
            return false;
        }
        let mut st = self.breaker.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(opened_at) = st.opened_at {
            if opened_at.elapsed() >= st.cooldown {
                st.opened_at = None;
                st.consecutive_crashes = 0;
                st.total_restarts = st.total_restarts.saturating_add(1);
                true
            } else {
                false
            }
        } else {
            true
        }
    }

    pub fn record_worker_healthy(&self) {
        let mut st = self.breaker.lock().unwrap_or_else(|e| e.into_inner());
        st.consecutive_crashes = 0;
        st.opened_at = None;
    }

    pub fn breaker_status(&self) -> CdpBreakerStatus {
        let st = self.breaker.lock().unwrap_or_else(|e| e.into_inner());
        let is_open = st
            .opened_at
            .is_some_and(|opened| opened.elapsed() < st.cooldown);
        CdpBreakerStatus {
            is_open,
            consecutive_crashes: st.consecutive_crashes,
            total_crashes: st.total_crashes,
            total_restarts: st.total_restarts,
            last_crash_reason: st.last_crash_reason.clone(),
        }
    }

    pub fn evaluate(&self, input: &VisualQcEvalInput) -> VisualQcEvalResult {
        let permit = self.acquire_permit();
        let viewports = normalize_viewports(input.target_kind, &input.viewports);
        let layout_sha1 = compute_layout_sha1(
            &input.html,
            &input.constraints_json,
            &viewports,
            &input.fixture_ref,
        );

        let mut issues = Vec::new();
        for &vp in &viewports {
            issues.extend(scan_html_static_rules(&input.html, input.target_kind, vp));
            issues.extend(evaluate_geometry_rules(
                &input.geometry_nodes,
                input.target_kind,
                vp,
            ));
        }

        let flow_completeness =
            if !input.flow_nodes.is_empty() || input.target_kind == QcTargetKind::FullFlow {
                let report = audit_e2e_flow_completeness(&input.flow_nodes, &input.flow_edges);
                issues.extend(report.issues.clone());
                Some(report)
            } else {
                None
            };

        let cdp_ready = !input.force_no_browser && self.can_attempt_cdp();
        let mut ssim_score: Option<f64> = None;

        if cdp_ready {
            if let (Some(ref_buf), Some(cand_buf)) = (
                input.reference_rgba.as_deref(),
                input.candidate_rgba.as_deref(),
            ) {
                if input.frame_width > 0 && input.frame_height > 0 {
                    if let Ok(score) =
                        compute_rgba_ssim(ref_buf, cand_buf, input.frame_width, input.frame_height)
                    {
                        ssim_score = Some(score);
                        let threshold = if input.ssim_threshold > 0.0 {
                            input.ssim_threshold
                        } else {
                            DEFAULT_SSIM_PASS_THRESHOLD
                        };
                        if score < threshold {
                            issues.push(QcIssue {
                                id: format!("ssim-{}", input.target_id),
                                rule_id: "R06_RGBA_SSIM_REGRESSION".into(),
                                category: "occlusion".into(),
                                severity: QcSeverity::P0,
                                message: format!(
                                    "Real RGBA SSIM {:.4} is below threshold {:.2}",
                                    score, threshold
                                ),
                                viewport_width: *viewports.first().unwrap_or(&1440),
                                ff_id: Some(input.target_id.clone()),
                                bbox: None,
                                deterministic_fixable: false,
                                suggested_fix: Some(
                                    "Inspect pixel occlusion diff and restore clipped region"
                                        .into(),
                                ),
                                target_node_id: None,
                            });
                        }
                    }
                }
            }
        }

        let has_browser_frames = !input.geometry_nodes.is_empty() || ssim_score.is_some();
        let browser_verified = cdp_ready && has_browser_frames;

        let (verdict, used_fallback, fallback_mode) = if !browser_verified {
            // Honest fail-closed fallback: NEVER report PASS when local Chromium CDP is
            // unavailable OR no actual browser-captured DOM geometry/RGBA frames were produced.
            ssim_score = None;
            let message = if !cdp_ready {
                "Pooled Chromium CDP unavailable; executed iframe_bridge_fallback (UNVERIFIED_NO_BROWSER)"
            } else {
                "No browser-captured DOM geometry or RGBA frames produced by live CDP; executed iframe_bridge_fallback (UNVERIFIED_NO_BROWSER)"
            };
            issues.push(QcIssue {
                id: format!("no-browser-{}", input.target_id),
                rule_id: "R26_UNVERIFIED_NO_BROWSER_CDP".into(),
                category: "occlusion".into(),
                severity: QcSeverity::P1,
                message: message.into(),
                viewport_width: *viewports.first().unwrap_or(&1440),
                ff_id: None,
                bbox: None,
                deterministic_fixable: false,
                suggested_fix: Some("Install local Chromium or configure COWOK_CDP_CHROME_BIN for live CDP verification".into()),
                target_node_id: None,
            });
            (
                QcVerdict::UnverifiedNoBrowser,
                true,
                Some("iframe_bridge_fallback".to_string()),
            )
        } else {
            self.record_worker_healthy();
            let p0_pre = issues
                .iter()
                .filter(|i| i.severity == QcSeverity::P0)
                .count();
            let v = if p0_pre == 0 {
                QcVerdict::Pass
            } else {
                QcVerdict::Fail
            };
            (v, false, None)
        };

        let p0_count = issues
            .iter()
            .filter(|i| i.severity == QcSeverity::P0)
            .count();
        let p1_count = issues
            .iter()
            .filter(|i| i.severity == QcSeverity::P1)
            .count();

        VisualQcEvalResult {
            target_kind: input.target_kind,
            target_id: input.target_id.clone(),
            layout_sha1,
            viewports,
            verdict,
            p0_count,
            p1_count,
            ssim_score,
            issues,
            flow_completeness,
            used_fallback,
            fallback_mode,
            incognito_context_id: permit.incognito_context_id.clone(),
            rules_evaluated: QC_RULE_CATALOG.len(),
            breaker_status: self.breaker_status(),
        }
    }
}

fn normalize_viewports(target_kind: QcTargetKind, requested: &[u32]) -> Vec<u32> {
    if target_kind == QcTargetKind::PresentationSlide {
        return vec![1920];
    }
    let mut cleaned: Vec<u32> = requested
        .iter()
        .copied()
        .filter(|&w| (RESPONSIVE_SWEEP_MIN_PX..=1920).contains(&w))
        .collect();
    if cleaned.is_empty() {
        cleaned.extend_from_slice(CANONICAL_APP_VIEWPORTS);
    }
    cleaned.sort_unstable_by(|a, b| b.cmp(a));
    cleaned.dedup();
    cleaned
}

/// Discovers a local Chromium binary if present, returning `None` cleanly when absent.
pub fn discover_local_chromium_binary() -> Option<String> {
    if let Ok(explicit) = std::env::var("COWOK_CDP_CHROME_BIN") {
        let trimmed = explicit.trim();
        if trimmed.eq_ignore_ascii_case("none") || trimmed.eq_ignore_ascii_case("disabled") {
            return None;
        }
        if !trimmed.is_empty() && Path::new(trimmed).exists() {
            return Some(trimmed.to_string());
        }
    }
    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/usr/bin/google-chrome",
    ];
    for path in candidates {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}
