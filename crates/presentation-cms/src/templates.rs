// @story 06.1
// @implements AC-01, AC-04, AC-05
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ulid::generate_slide_uid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideLayoutKind {
    HeroCover,
    ProblemSolution,
    KpiMetricGrid,
    SplitComparison,
    ArchitectureFlow,
    RoadmapTimeline,
    ExecutiveQuote,
    DataTableReport,
}

impl SlideLayoutKind {
    pub fn all() -> &'static [Self] {
        &[
            Self::HeroCover,
            Self::ProblemSolution,
            Self::KpiMetricGrid,
            Self::SplitComparison,
            Self::ArchitectureFlow,
            Self::RoadmapTimeline,
            Self::ExecutiveQuote,
            Self::DataTableReport,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HeroCover => "hero_cover",
            Self::ProblemSolution => "problem_solution",
            Self::KpiMetricGrid => "kpi_metric_grid",
            Self::SplitComparison => "split_comparison",
            Self::ArchitectureFlow => "architecture_flow",
            Self::RoadmapTimeline => "roadmap_timeline",
            Self::ExecutiveQuote => "executive_quote",
            Self::DataTableReport => "data_table_report",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "hero_cover" => Some(Self::HeroCover),
            "problem_solution" => Some(Self::ProblemSolution),
            "kpi_metric_grid" => Some(Self::KpiMetricGrid),
            "split_comparison" => Some(Self::SplitComparison),
            "architecture_flow" => Some(Self::ArchitectureFlow),
            "roadmap_timeline" => Some(Self::RoadmapTimeline),
            "executive_quote" => Some(Self::ExecutiveQuote),
            "data_table_report" => Some(Self::DataTableReport),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderSlideRequest {
    pub slide_uid: Option<String>,
    pub layout_kind: SlideLayoutKind,
    pub theme_preset: String,
    pub aspect_ratio: String,
    pub eyebrow_tag: String,
    pub headline: String,
    pub subtitle: String,
    pub primary_tokens_css: String,
    pub icon_manifest_version: String,
}

#[derive(Debug, Clone)]
pub struct RenderedSlideOutput {
    pub slide_uid: String,
    pub html: String,
    pub content_sha256: String,
    pub extracted_slots: Vec<(String, String, String)>,
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn theme_palette_css(
    theme_preset: &str,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match theme_preset {
        "warm" => ("#F7F5F0", "#FAF9F5", "#0F172A", "#4B7FFF"),
        "dim" => ("#ECEAE4", "#F4F2EC", "#18181B", "#315FB6"),
        "low-focus" => ("#F3F4F6", "#FFFFFF", "#111827", "#475569"),
        _ => ("#0B1220", "#172033", "#F8FAFC", "#38BDF8"), // isolated Night default
    }
}

fn aspect_dimensions(aspect_ratio: &str) -> (&'static str, &'static str) {
    match aspect_ratio {
        "16:10" => ("1920px", "1200px"),
        "Auto" | "auto" => ("1440px", "900px"),
        _ => ("1920px", "1080px"),
    }
}

pub fn render_branded_slide(req: &RenderSlideRequest) -> RenderedSlideOutput {
    let slide_uid = req.slide_uid.clone().unwrap_or_else(generate_slide_uid);
    let (bg_canvas, bg_card, text_ink, accent) = theme_palette_css(&req.theme_preset);
    let (slide_w, slide_h) = aspect_dimensions(&req.aspect_ratio);

    let ff_eyebrow = format!("ff-{}-eyebrow", req.layout_kind.as_str());
    let ff_headline = format!("ff-{}-headline", req.layout_kind.as_str());
    let ff_body = format!("ff-{}-body", req.layout_kind.as_str());

    let eyebrow_escaped = escape_html(if req.eyebrow_tag.is_empty() {
        "EXECUTIVE DECK"
    } else {
        &req.eyebrow_tag
    });
    let headline_escaped = escape_html(&req.headline);
    let subtitle_escaped = escape_html(if req.subtitle.is_empty() {
        "Generated from canonical PROJECT_CONSTRAINTS & tokens.css"
    } else {
        &req.subtitle
    });
    let tokens_comment = escape_html(&req.icon_manifest_version);

    let body_fragment: &'static str = match req.layout_kind {
        SlideLayoutKind::HeroCover => {
            "<div class=\"deck-hero\"><div class=\"deck-card\" data-ff-id=\"ff-hero-card\">Metric SLA &lt;16ms</div></div>"
        }
        SlideLayoutKind::ProblemSolution => {
            "<div class=\"deck-grid-2\"><div class=\"deck-card\" data-ff-id=\"ff-problem-box\">Problem Boundary</div><div class=\"deck-card\" data-ff-id=\"ff-solution-box\">Deterministic Solution</div></div>"
        }
        SlideLayoutKind::KpiMetricGrid => {
            "<div class=\"deck-grid-3\"><div class=\"deck-card\" data-ff-id=\"ff-kpi-1\">99.9% Gate</div><div class=\"deck-card\" data-ff-id=\"ff-kpi-2\">&lt;16ms Sync</div><div class=\"deck-card\" data-ff-id=\"ff-kpi-3\">0 Leaks</div></div>"
        }
        SlideLayoutKind::SplitComparison => {
            "<div class=\"deck-grid-2\"><div class=\"deck-card\" data-ff-id=\"ff-cmp-before\">Legacy</div><div class=\"deck-card\" data-ff-id=\"ff-cmp-after\">Canonical §9.1</div></div>"
        }
        SlideLayoutKind::ArchitectureFlow => {
            "<div class=\"deck-flow\" data-ff-id=\"ff-arch-flow\">Rust Core -> SQLite V12 -> Sandboxed Stage</div>"
        }
        SlideLayoutKind::RoadmapTimeline => {
            "<div class=\"deck-grid-3\"><div class=\"deck-card\" data-ff-id=\"ff-phase-1\">Sprint 1: Core Engine</div><div class=\"deck-card\" data-ff-id=\"ff-phase-2\">Sprint 2: Dual-Pane CMS</div><div class=\"deck-card\" data-ff-id=\"ff-phase-3\">Sprint 3: Presenter</div></div>"
        }
        SlideLayoutKind::ExecutiveQuote => {
            "<blockquote class=\"deck-quote\" data-ff-id=\"ff-quote-text\">Deterministic Human Governance + Agent Synthesis</blockquote>"
        }
        SlideLayoutKind::DataTableReport => {
            "<table class=\"deck-table\" data-ff-id=\"ff-table-report\"><thead><tr><th>Dimension</th><th>Status</th></tr></thead><tbody><tr><td>CSP-B</td><td>PASS</td></tr></tbody></table>"
        }
    };

    let custom_vars = if req.primary_tokens_css.trim().is_empty() {
        String::new()
    } else {
        crate::cms_patcher::validate_and_scrub_css_tokens(&req.primary_tokens_css, false)
            .map(|safe| sanitize_style_close_tags(&safe))
            .unwrap_or_default()
    };

    let html = format!(
        "<!DOCTYPE html>
<html lang=\"en\" data-cowok-deck-protocol=\"1\" data-icon-manifest=\"{tokens_comment}\">
<head>
<meta charset=\"utf-8\">
<style>
{custom_vars}
:root {{
  --slide-w: {slide_w};
  --slide-h: {slide_h};
  --deck-bg: {bg_canvas};
  --deck-surface: {bg_card};
  --deck-ink: {text_ink};
  --deck-accent: {accent};
  --type-display: 56px;
  --type-heading: 32px;
  --type-body: 22px;
  --type-micro: 16px;
}}
html, body {{
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  background: var(--deck-bg);
  color: var(--deck-ink);
  font-family: 'Geist', 'Inter', system-ui, sans-serif;
}}
.slide {{
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  padding: clamp(20px, 3.6vw, 64px) clamp(24px, 4vw, 72px);
  display: none;
  flex-direction: column;
  justify-content: space-between;
  background: var(--deck-bg);
}}
.slide.active {{ display: flex; }}
.deck-eyebrow {{ font-size: clamp(10px, 1.05vw, var(--type-micro)); letter-spacing: 0.08em; color: var(--deck-accent); font-weight: 600; }}
.deck-headline {{ font-size: clamp(22px, 3.3vw, var(--type-display)); line-height: 1.15; margin: 12px 0; font-weight: 700; overflow-wrap: break-word; }}
.deck-subtitle {{ font-size: clamp(12.5px, 1.5vw, var(--type-body)); line-height: 1.45; opacity: 0.88; max-width: 100%; }}
.deck-grid-2 {{ display: grid; grid-template-columns: 1fr 1fr; gap: clamp(12px, 2vw, 28px); }}
.deck-grid-3 {{ display: grid; grid-template-columns: repeat(3, 1fr); gap: clamp(10px, 1.8vw, 24px); }}
.deck-card, .deck-flow, .deck-quote, .deck-table {{
  background: var(--deck-surface);
  border: 1px solid rgba(148, 163, 184, 0.24);
  border-radius: 12px;
  padding: clamp(12px, 1.8vw, 28px);
  font-size: clamp(13px, 1.85vw, var(--type-heading));
}}
</style>
</head>
<body>
<section class=\"slide active\" data-slide-uid=\"{slide_uid}\" data-layout-kind=\"{layout}\">
  <header>
    <div class=\"deck-eyebrow\" data-ff-id=\"{ff_eyebrow}\">{eyebrow_escaped}</div>
    <h1 class=\"deck-headline\" data-ff-id=\"{ff_headline}\">{headline_escaped}</h1>
    <p class=\"deck-subtitle\" data-ff-id=\"{ff_body}\">{subtitle_escaped}</p>
  </header>
  {body_fragment}
</section>
</body>
</html>",
        layout = req.layout_kind.as_str(),
    );

    let content_sha256 = compute_sha256_hex(html.as_bytes());
    let mut extracted_slots = vec![
        (
            ff_eyebrow,
            "Eyebrow Tag".to_string(),
            req.eyebrow_tag.clone(),
        ),
        (ff_headline, "Headline".to_string(), req.headline.clone()),
        (ff_body, "Subtitle".to_string(), req.subtitle.clone()),
    ];
    match req.layout_kind {
        SlideLayoutKind::HeroCover => extracted_slots.push((
            "ff-hero-card".into(),
            "Hero Metric".into(),
            "Metric SLA <16ms".into(),
        )),
        SlideLayoutKind::ProblemSolution => {
            extracted_slots.push((
                "ff-problem-box".into(),
                "Problem".into(),
                "Problem Boundary".into(),
            ));
            extracted_slots.push((
                "ff-solution-box".into(),
                "Solution".into(),
                "Deterministic Solution".into(),
            ));
        }
        SlideLayoutKind::KpiMetricGrid => {
            extracted_slots.push(("ff-kpi-1".into(), "KPI 1".into(), "99.9% Gate".into()));
            extracted_slots.push(("ff-kpi-2".into(), "KPI 2".into(), "<16ms Sync".into()));
            extracted_slots.push(("ff-kpi-3".into(), "KPI 3".into(), "0 Leaks".into()));
        }
        SlideLayoutKind::SplitComparison => {
            extracted_slots.push(("ff-cmp-before".into(), "Before".into(), "Legacy".into()));
            extracted_slots.push((
                "ff-cmp-after".into(),
                "After".into(),
                "Canonical §9.1".into(),
            ));
        }
        SlideLayoutKind::ArchitectureFlow => extracted_slots.push((
            "ff-arch-flow".into(),
            "Architecture Flow".into(),
            "Rust Core -> SQLite V12 -> Sandboxed Stage".into(),
        )),
        SlideLayoutKind::RoadmapTimeline => {
            extracted_slots.push((
                "ff-phase-1".into(),
                "Phase 1".into(),
                "Sprint 1: Core Engine".into(),
            ));
            extracted_slots.push((
                "ff-phase-2".into(),
                "Phase 2".into(),
                "Sprint 2: Dual-Pane CMS".into(),
            ));
            extracted_slots.push((
                "ff-phase-3".into(),
                "Phase 3".into(),
                "Sprint 3: Presenter".into(),
            ));
        }
        SlideLayoutKind::ExecutiveQuote => extracted_slots.push((
            "ff-quote-text".into(),
            "Executive Quote".into(),
            "Deterministic Human Governance + Agent Synthesis".into(),
        )),
        SlideLayoutKind::DataTableReport => extracted_slots.push((
            "ff-table-report".into(),
            "Report Table".into(),
            "CSP-B PASS".into(),
        )),
    }

    RenderedSlideOutput {
        slide_uid,
        html,
        content_sha256,
        extracted_slots,
    }
}

pub fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn sanitize_style_close_tags(raw_css: &str) -> String {
    let lower = raw_css.to_ascii_lowercase();
    let mut out = String::with_capacity(raw_css.len());
    let mut cursor = 0usize;
    while let Some(rel) = lower[cursor..].find("</style") {
        let pos = cursor + rel;
        out.push_str(&raw_css[cursor..pos]);
        out.push_str("/*blocked*/");
        cursor = pos + "</style".len();
    }
    out.push_str(&raw_css[cursor..]);
    out
}
