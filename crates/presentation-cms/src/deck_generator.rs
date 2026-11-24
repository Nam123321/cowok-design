// @story 06.5
// @implements AC-01, AC-02, AC-04, AC-05 (Stageshow <Stage>/<Window>/<Phone> Multi-Slide Deck Generator, <=30 Word Budget & Overflow Auto-Split)
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::chart_normalizer::NormalizedChartOutput;
use crate::cms_patcher::validate_and_scrub_css_tokens;
use crate::protocol::{
    validate_slide_protocol_structure, ProtocolValidationError, MAX_SLIDE_HTML_BYTES,
};
use crate::templates::{compute_sha256_hex, SlideLayoutKind};
use crate::ulid::generate_slide_uid;

pub const STAGESHOW_MIN_BULLET_WORDS: usize = 3;
pub const STAGESHOW_MAX_BULLET_WORDS: usize = 7;
pub const STAGESHOW_MAX_WORDS_PER_SLIDE: usize = 30;
pub const STAGESHOW_MAX_HEADLINE_WORDS: usize = 8;
pub const STAGESHOW_MAX_EYEBROW_WORDS: usize = 4;
pub const MAX_GENERATED_SLIDES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StageshowFramePreset {
    #[default]
    #[serde(rename = "Stage")]
    Stage,
    #[serde(rename = "Window")]
    Window,
    #[serde(rename = "Phone")]
    Phone,
}

impl StageshowFramePreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stage => "Stage",
            Self::Window => "Window",
            Self::Phone => "Phone",
        }
    }

    pub fn css_modifier(&self) -> &'static str {
        match self {
            Self::Stage => "stage",
            Self::Window => "window",
            Self::Phone => "phone",
        }
    }

    pub fn parse_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "stage" | "<stage>" => Some(Self::Stage),
            "window" | "<window>" => Some(Self::Window),
            "phone" | "<phone>" => Some(Self::Phone),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum DeckGeneratorError {
    #[error("VALIDATION_FAILED: {0}")]
    InvalidInput(String),
    #[error("E_DECK_SLIDE_LIMIT: Generated slide count {actual} exceeds maximum {max}")]
    SlideLimitExceeded { actual: usize, max: usize },
    #[error("{0}")]
    Protocol(#[from] ProtocolValidationError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedWordingSlot {
    pub ff_id: String,
    pub slot_label: String,
    pub slot_kind: String,
    pub text_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charts_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedSlideArtifact {
    pub slide_uid: String,
    pub slide_index: usize,
    pub layout_kind: SlideLayoutKind,
    pub frame_preset: StageshowFramePreset,
    pub eyebrow_tag: String,
    pub headline: String,
    pub bullets: Vec<String>,
    pub word_count: usize,
    pub is_continuation: bool,
    pub speaker_notes: String,
    pub html: String,
    pub content_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charts_json: Option<String>,
    pub slots: Vec<GeneratedWordingSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedDeckOutput {
    pub title: String,
    pub frame_preset: StageshowFramePreset,
    pub theme_preset: String,
    pub aspect_ratio: String,
    pub slides: Vec<GeneratedSlideArtifact>,
    pub total_slides: usize,
    pub auto_split_count: usize,
    pub max_words_per_slide: usize,
}

#[derive(Debug, Clone)]
pub struct DraftSlideInput {
    pub layout_kind: SlideLayoutKind,
    pub eyebrow_tag: String,
    pub headline: String,
    pub bullets: Vec<String>,
    pub speaker_notes: String,
    pub charts_json: Option<String>,
    pub chart_title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GenerateDeckSpecInput {
    pub title: String,
    pub brief: String,
    pub frame_preset: StageshowFramePreset,
    pub theme_preset: String,
    pub aspect_ratio: String,
    pub primary_tokens_css: String,
    pub normalized_chart: Option<NormalizedChartOutput>,
    pub llm_slides_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct SlideRewriteInput {
    pub prompt: String,
    pub current_text: String,
    pub slot_kind: String,
    pub target_ff_id: String,
    pub frame_preset: StageshowFramePreset,
    pub llm_output: Option<String>,
    pub existing_other_slots_words: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideRewriteOutput {
    pub primary_text: String,
    pub word_count: usize,
    pub overflow_bullets: Vec<String>,
    pub was_condensed_or_split: bool,
    pub used_llm_output: bool,
}

/// Counts whitespace-delimited words in a string.
pub fn count_words(text: &str) -> usize {
    text.split_whitespace().filter(|w| !w.is_empty()).count()
}

fn clean_inline_text(input: &str) -> String {
    input
        .replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape_html_text(input: &str) -> String {
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
        _ => ("#0B1220", "#172033", "#F8FAFC", "#38BDF8"),
    }
}

fn aspect_dimensions(aspect_ratio: &str) -> (&'static str, &'static str) {
    match aspect_ratio {
        "16:10" => ("1920px", "1200px"),
        "Auto" | "auto" => ("1440px", "900px"),
        _ => ("1920px", "1080px"),
    }
}

/// Splits or normalizes raw bullet statements so every bullet adheres strictly to `3..=7` words.
pub fn normalize_bullets_to_budget(raw_bullets: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = Vec::new();
    for raw in raw_bullets {
        let cleaned = clean_inline_text(raw)
            .trim_start_matches(['-', '*', '•', '·', ' '])
            .trim()
            .to_string();
        if cleaned.is_empty() {
            continue;
        }
        let words: Vec<&str> = cleaned.split_whitespace().collect();
        if words.len() <= STAGESHOW_MAX_BULLET_WORDS {
            if words.len() < STAGESHOW_MIN_BULLET_WORDS {
                let padded = pad_short_bullet(&words);
                normalized.push(padded);
            } else {
                normalized.push(words.join(" "));
            }
        } else {
            // Split long text cleanly into 5-7 word bullets without dropping information
            let mut idx = 0usize;
            while idx < words.len() {
                let remaining = words.len() - idx;
                let take = if remaining <= STAGESHOW_MAX_BULLET_WORDS {
                    remaining
                } else if remaining - STAGESHOW_MAX_BULLET_WORDS < STAGESHOW_MIN_BULLET_WORDS {
                    // Balance last two chunks so neither falls below 3 words
                    remaining.div_ceil(2)
                } else {
                    STAGESHOW_MAX_BULLET_WORDS
                };
                let slice = &words[idx..idx + take];
                if slice.len() < STAGESHOW_MIN_BULLET_WORDS {
                    normalized.push(pad_short_bullet(slice));
                } else {
                    normalized.push(slice.join(" "));
                }
                idx += take;
            }
        }
    }
    normalized
}

fn pad_short_bullet(words: &[&str]) -> String {
    match words.len() {
        0 => "Verified key outcome".to_string(),
        1 => format!("Key focus: {}", words[0]),
        2 => format!("Focus: {} {}", words[0], words[1]),
        _ => words.join(" "),
    }
}

fn condense_headline(raw: &str, fallback: &str) -> String {
    let cleaned = clean_inline_text(raw);
    let source = if cleaned.is_empty() {
        fallback
    } else {
        cleaned.as_str()
    };
    let words: Vec<&str> = source.split_whitespace().collect();
    if words.len() <= STAGESHOW_MAX_HEADLINE_WORDS {
        words.join(" ")
    } else {
        words[..STAGESHOW_MAX_HEADLINE_WORDS].join(" ")
    }
}

fn condense_eyebrow(raw: &str, fallback: &str) -> String {
    let cleaned = clean_inline_text(raw);
    let source = if cleaned.is_empty() {
        fallback
    } else {
        cleaned.as_str()
    };
    let words: Vec<&str> = source.split_whitespace().collect();
    if words.len() <= STAGESHOW_MAX_EYEBROW_WORDS {
        words.join(" ")
    } else {
        words[..STAGESHOW_MAX_EYEBROW_WORDS].join(" ")
    }
}

fn build_continuation_eyebrow(base_eyebrow: &str) -> String {
    let max_prefix = STAGESHOW_MAX_EYEBROW_WORDS.saturating_sub(2).max(1);
    let words: Vec<&str> = base_eyebrow.split_whitespace().collect();
    let prefix = if words.len() > max_prefix {
        words[..max_prefix].join(" ")
    } else {
        base_eyebrow.to_string()
    };
    format!("{prefix} · CONT.")
}

fn wrap_stageshow_document_html(
    frame_preset: StageshowFramePreset,
    theme_preset: &str,
    aspect_ratio: &str,
    primary_tokens_css: &str,
    body_sections_html: &str,
) -> Result<String, DeckGeneratorError> {
    let (bg_canvas, bg_card, text_ink, accent) = theme_palette_css(theme_preset);
    let (slide_w, slide_h) = aspect_dimensions(aspect_ratio);
    let custom_vars = if primary_tokens_css.trim().is_empty() {
        String::new()
    } else {
        validate_and_scrub_css_tokens(primary_tokens_css, false)
            .map(|safe| sanitize_style_close_tags(&safe))
            .unwrap_or_default()
    };

    let html = format!(
        "<!DOCTYPE html>
<html lang=\"en\" data-cowok-deck-protocol=\"1\" data-stageshow-preset=\"{preset}\">
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
  padding: clamp(24px, 3.8vw, 64px) clamp(28px, 4.2vw, 72px);
  display: none;
  flex-direction: column;
  justify-content: space-between;
  gap: 20px;
  background: var(--deck-bg);
}}
.slide.active {{ display: flex; }}
.deck-eyebrow {{ font-size: clamp(11px, 1.1vw, 16px); letter-spacing: 0.08em; color: var(--deck-accent); font-weight: 600; text-transform: uppercase; }}
.deck-headline {{ font-size: clamp(24px, 3.2vw, 52px); line-height: 1.15; margin: 10px 0 0 0; font-weight: 700; }}
.stageshow-frame {{
  flex: 1;
  display: flex;
  flex-direction: column;
  background: var(--deck-surface);
  border: 1px solid rgba(148, 163, 184, 0.28);
  border-radius: 16px;
  overflow: hidden;
}}
.stageshow-frame--stage {{ padding: 24px; justify-content: center; }}
.stageshow-frame--window {{ box-shadow: 0 18px 40px rgba(0, 0, 0, 0.24); }}
.stageshow-window-bar {{ display: flex; align-items: center; gap: 8px; padding: 10px 16px; border-bottom: 1px solid rgba(148, 163, 184, 0.22); }}
.stageshow-dot {{ width: 10px; height: 10px; border-radius: 999px; background: rgba(148, 163, 184, 0.5); display: inline-block; }}
.stageshow-window-title {{ margin-left: 8px; font-size: 12px; opacity: 0.78; }}
.stageshow-window-body {{ padding: 22px; display: flex; flex-direction: column; gap: 16px; }}
.stageshow-frame--phone {{ max-width: 440px; width: 100%; margin: 0 auto; border-radius: 32px; border-width: 3px; padding: 14px 20px 22px; }}
.stageshow-phone-notch {{ width: 96px; height: 8px; border-radius: 999px; background: rgba(148, 163, 184, 0.4); margin: 0 auto 16px; }}
.stageshow-phone-body {{ display: flex; flex-direction: column; gap: 12px; }}
.stageshow-bullets {{ margin: 0; padding-left: 22px; display: flex; flex-direction: column; gap: 12px; font-size: clamp(16px, 1.8vw, 26px); line-height: 1.38; }}
.stageshow-chart-card {{ margin-top: 12px; padding: 16px; border-radius: 12px; border: 1px dashed rgba(148, 163, 184, 0.35); }}
.stageshow-chart-title {{ font-size: 16px; font-weight: 600; color: var(--deck-accent); }}
</style>
</head>
<body>
{body_sections_html}
</body>
</html>",
        preset = frame_preset.as_str(),
    );

    if html.len() > MAX_SLIDE_HTML_BYTES {
        return Err(DeckGeneratorError::Protocol(
            ProtocolValidationError::PayloadTooLarge {
                actual: html.len(),
                max: MAX_SLIDE_HTML_BYTES,
            },
        ));
    }

    let validated = validate_slide_protocol_structure(&html)?;
    Ok(validated.sanitized_html)
}

/// Assembles all slides of a `one-page` stageshow deck into a single protocol-compliant
/// multi-`<section data-slide-uid="...">` HTML document so scoped rewrites preserve sibling slides.
pub fn assemble_one_page_stageshow_html(
    slides: &[GeneratedSlideArtifact],
    frame_preset: StageshowFramePreset,
    theme_preset: &str,
    aspect_ratio: &str,
    primary_tokens_css: &str,
) -> Result<String, DeckGeneratorError> {
    if slides.is_empty() {
        return Err(DeckGeneratorError::InvalidInput(
            "Cannot assemble one-page deck with 0 slides".to_string(),
        ));
    }
    let mut sections = String::new();
    for (idx, slide) in slides.iter().enumerate() {
        let Some(sec_start) = slide.html.find("<section ") else {
            return Err(DeckGeneratorError::InvalidInput(format!(
                "Generated slide {} missing <section> element",
                slide.slide_uid
            )));
        };
        let Some(sec_end_rel) = slide.html[sec_start..].rfind("</section>") else {
            return Err(DeckGeneratorError::InvalidInput(format!(
                "Generated slide {} missing closing </section> element",
                slide.slide_uid
            )));
        };
        let raw_section = &slide.html[sec_start..sec_start + sec_end_rel + "</section>".len()];
        if idx == 0 {
            sections.push_str(raw_section);
        } else {
            sections.push('\n');
            sections.push_str(&raw_section.replacen(
                "class=\"slide active stageshow-slide\"",
                "class=\"slide stageshow-slide\"",
                1,
            ));
        }
    }
    wrap_stageshow_document_html(
        frame_preset,
        theme_preset,
        aspect_ratio,
        primary_tokens_css,
        &sections,
    )
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

/// Renders a single protocol-compliant `stageshow-slide` HTML document wrapped in `<Stage>`, `<Window>`, or `<Phone>`.
#[allow(clippy::too_many_arguments)]
pub fn render_stageshow_slide_html(
    slide_uid: &str,
    slide_index: usize,
    layout_kind: SlideLayoutKind,
    frame_preset: StageshowFramePreset,
    theme_preset: &str,
    aspect_ratio: &str,
    primary_tokens_css: &str,
    eyebrow_tag: &str,
    headline: &str,
    bullets: &[String],
    charts_json: Option<&str>,
    chart_title: Option<&str>,
) -> Result<(String, Vec<GeneratedWordingSlot>), DeckGeneratorError> {
    let seq = slide_index + 1;
    let ff_eyebrow = format!("ff-s{seq}-eyebrow");
    let ff_headline = format!("ff-s{seq}-headline");
    let ff_chart = format!("ff-s{seq}-chart");

    let resolved_eyebrow = if eyebrow_tag.trim().is_empty() {
        "STAGESHOW"
    } else {
        eyebrow_tag.trim()
    };
    let safe_eyebrow = escape_html_text(resolved_eyebrow);
    let safe_headline = escape_html_text(headline);

    let mut slots: Vec<GeneratedWordingSlot> = vec![
        GeneratedWordingSlot {
            ff_id: ff_eyebrow.clone(),
            slot_label: "Eyebrow".to_string(),
            slot_kind: "text".to_string(),
            text_value: resolved_eyebrow.to_string(),
            charts_json: None,
        },
        GeneratedWordingSlot {
            ff_id: ff_headline.clone(),
            slot_label: "Headline".to_string(),
            slot_kind: "heading".to_string(),
            text_value: headline.to_string(),
            charts_json: None,
        },
    ];

    let mut bullets_html = String::new();
    if !bullets.is_empty() {
        bullets_html.push_str("<ul class=\"stageshow-bullets\">");
        for (b_idx, bullet) in bullets.iter().enumerate() {
            let ff_bullet = format!("ff-s{seq}-b{}", b_idx + 1);
            let escaped = escape_html_text(bullet);
            bullets_html.push_str(&format!(
                "<li class=\"stageshow-bullet\" data-ff-id=\"{ff_bullet}\">{escaped}</li>"
            ));
            slots.push(GeneratedWordingSlot {
                ff_id: ff_bullet,
                slot_label: format!("Point {}", b_idx + 1),
                slot_kind: "text".to_string(),
                text_value: bullet.clone(),
                charts_json: None,
            });
        }
        bullets_html.push_str("</ul>");
    }

    let chart_html = if let Some(raw_chart_json) = charts_json {
        let escaped_json = escape_html_text(raw_chart_json);
        let label_title = chart_title.unwrap_or("Data Visualization");
        let escaped_title = escape_html_text(label_title);
        slots.push(GeneratedWordingSlot {
            ff_id: ff_chart.clone(),
            slot_label: "ChartSpecV12".to_string(),
            slot_kind: "chart_json".to_string(),
            text_value: label_title.to_string(),
            charts_json: Some(raw_chart_json.to_string()),
        });
        format!(
            "<div class=\"stageshow-chart-card\" data-ff-id=\"{ff_chart}\" data-slot-kind=\"chart_json\" data-charts-json=\"{escaped_json}\"><div class=\"stageshow-chart-title\">{escaped_title}</div></div>"
        )
    } else {
        String::new()
    };

    let frame_chrome = match frame_preset {
        StageshowFramePreset::Stage => format!(
            "<div class=\"stageshow-frame stageshow-frame--stage\" data-stageshow-tag=\"Stage\"><div class=\"stageshow-stage-body\">{bullets_html}{chart_html}</div></div>"
        ),
        StageshowFramePreset::Window => format!(
            "<div class=\"stageshow-frame stageshow-frame--window\" data-stageshow-tag=\"Window\"><div class=\"stageshow-window-bar\"><span class=\"stageshow-dot\"></span><span class=\"stageshow-dot\"></span><span class=\"stageshow-dot\"></span><span class=\"stageshow-window-title\">{safe_eyebrow}</span></div><div class=\"stageshow-window-body\">{bullets_html}{chart_html}</div></div>"
        ),
        StageshowFramePreset::Phone => format!(
            "<div class=\"stageshow-frame stageshow-frame--phone\" data-stageshow-tag=\"Phone\"><div class=\"stageshow-phone-notch\"></div><div class=\"stageshow-phone-body\">{bullets_html}{chart_html}</div></div>"
        ),
    };

    let section_html = format!(
        "<section class=\"slide active stageshow-slide\" data-slide-uid=\"{slide_uid}\" data-layout-kind=\"{layout}\" data-stageshow-frame=\"{preset}\">
  <header>
    <div class=\"deck-eyebrow\" data-ff-id=\"{ff_eyebrow}\">{safe_eyebrow}</div>
    <h1 class=\"deck-headline\" data-ff-id=\"{ff_headline}\">{safe_headline}</h1>
  </header>
  {frame_chrome}
</section>",
        preset = frame_preset.as_str(),
        layout = layout_kind.as_str(),
    );

    let sanitized = wrap_stageshow_document_html(
        frame_preset,
        theme_preset,
        aspect_ratio,
        primary_tokens_css,
        &section_html,
    )?;
    Ok((sanitized, slots))
}

fn build_continuation_headline(base_headline: &str, part_num: usize) -> String {
    let words: Vec<&str> = base_headline.split_whitespace().collect();
    let max_prefix = STAGESHOW_MAX_HEADLINE_WORDS.saturating_sub(2).max(1);
    let prefix = if words.len() > max_prefix {
        words[..max_prefix].join(" ")
    } else {
        base_headline.to_string()
    };
    format!("{prefix} (Part {part_num})")
}

/// Splits a draft slide into one or more slides strictly obeying `<= 30` words/slide (including eyebrow, headline, and bullets)
/// and `3..=7` words/bullet, allocating a fresh ULID `SLD-*` for every continuation slide and enforcing `MAX_GENERATED_SLIDES`.
pub fn enforce_slide_word_budget_and_split(
    draft: &DraftSlideInput,
    start_index: usize,
    frame_preset: StageshowFramePreset,
    theme_preset: &str,
    aspect_ratio: &str,
    primary_tokens_css: &str,
) -> Result<(Vec<GeneratedSlideArtifact>, usize), DeckGeneratorError> {
    let base_eyebrow = condense_eyebrow(&draft.eyebrow_tag, "STAGESHOW");
    let cont_eyebrow = build_continuation_eyebrow(&base_eyebrow);
    let base_headline = condense_headline(&draft.headline, "Key Executive Insight");
    let normalized_bullets = normalize_bullets_to_budget(&draft.bullets);

    type SlideChunkTuple = (
        String,
        String,
        Vec<String>,
        bool,
        Option<String>,
        Option<String>,
    );
    // Partition bullets into groups where `eyebrow_words + headline_words + sum(bullet_words) <= 30`
    let mut slide_chunks: Vec<SlideChunkTuple> = Vec::new();
    if normalized_bullets.is_empty() {
        slide_chunks.push((
            base_eyebrow,
            base_headline,
            Vec::new(),
            false,
            draft.charts_json.clone(),
            draft.chart_title.clone(),
        ));
    } else {
        let mut current_bullets: Vec<String> = Vec::new();
        let mut chunk_idx = 0usize;
        let mut current_words = count_words(&base_eyebrow) + count_words(&base_headline);

        for bullet in normalized_bullets {
            let b_words = count_words(&bullet);
            let active_eyebrow = if chunk_idx == 0 {
                base_eyebrow.clone()
            } else {
                cont_eyebrow.clone()
            };
            let active_headline = if chunk_idx == 0 {
                base_headline.clone()
            } else {
                build_continuation_headline(&base_headline, chunk_idx + 1)
            };
            if !current_bullets.is_empty()
                && current_words + b_words > STAGESHOW_MAX_WORDS_PER_SLIDE
            {
                let (c_json, c_title) = if chunk_idx == 0 {
                    (draft.charts_json.clone(), draft.chart_title.clone())
                } else {
                    (None, None)
                };
                slide_chunks.push((
                    active_eyebrow,
                    active_headline,
                    std::mem::take(&mut current_bullets),
                    chunk_idx > 0,
                    c_json,
                    c_title,
                ));
                chunk_idx += 1;
                let next_h = build_continuation_headline(&base_headline, chunk_idx + 1);
                current_words = count_words(&cont_eyebrow) + count_words(&next_h);
            } else if current_bullets.is_empty() {
                current_words = count_words(&active_eyebrow) + count_words(&active_headline);
            }
            current_bullets.push(bullet);
            current_words += b_words;
        }

        if !current_bullets.is_empty() {
            let active_eyebrow = if chunk_idx == 0 {
                base_eyebrow
            } else {
                cont_eyebrow
            };
            let h = if chunk_idx == 0 {
                base_headline
            } else {
                build_continuation_headline(&base_headline, chunk_idx + 1)
            };
            let (c_json, c_title) = if chunk_idx == 0 {
                (draft.charts_json.clone(), draft.chart_title.clone())
            } else {
                (None, None)
            };
            slide_chunks.push((
                active_eyebrow,
                h,
                current_bullets,
                chunk_idx > 0,
                c_json,
                c_title,
            ));
        }
    }

    let projected_total = start_index.saturating_add(slide_chunks.len());
    if projected_total > MAX_GENERATED_SLIDES {
        return Err(DeckGeneratorError::SlideLimitExceeded {
            actual: projected_total,
            max: MAX_GENERATED_SLIDES,
        });
    }

    let auto_splits = slide_chunks.len().saturating_sub(1);
    let mut emitted = Vec::with_capacity(slide_chunks.len());
    for (offset, (eyebrow, headline, bullets, is_cont, c_json, c_title)) in
        slide_chunks.into_iter().enumerate()
    {
        let slide_idx = start_index + offset;
        let slide_uid = generate_slide_uid();
        let word_count = count_words(&eyebrow)
            + count_words(&headline)
            + bullets.iter().map(|b| count_words(b)).sum::<usize>();

        let (html, slots) = render_stageshow_slide_html(
            &slide_uid,
            slide_idx,
            draft.layout_kind,
            frame_preset,
            theme_preset,
            aspect_ratio,
            primary_tokens_css,
            &eyebrow,
            &headline,
            &bullets,
            c_json.as_deref(),
            c_title.as_deref(),
        )?;
        let content_sha256 = compute_sha256_hex(html.as_bytes());
        emitted.push(GeneratedSlideArtifact {
            slide_uid,
            slide_index: slide_idx,
            layout_kind: draft.layout_kind,
            frame_preset,
            eyebrow_tag: eyebrow,
            headline,
            bullets,
            word_count,
            is_continuation: is_cont,
            speaker_notes: draft.speaker_notes.clone(),
            html,
            content_sha256,
            charts_json: c_json,
            slots,
        });
    }

    Ok((emitted, auto_splits))
}

fn extract_sentences_from_brief(brief: &str) -> Vec<String> {
    brief
        .split(['\n', '.', ';', '!'])
        .map(clean_inline_text)
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_llm_slides_drafts(
    llm_val: &serde_json::Value,
    normalized_chart: Option<&NormalizedChartOutput>,
) -> Vec<DraftSlideInput> {
    let Some(arr) = llm_val
        .get("slides")
        .and_then(|v| v.as_array())
        .or_else(|| llm_val.as_array())
    else {
        return Vec::new();
    };
    let mut drafts = Vec::new();
    let mut chart_attached = false;
    for (idx, item) in arr.iter().enumerate() {
        let headline = item
            .get("headline")
            .or_else(|| item.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if headline.is_empty() {
            continue;
        }
        let eyebrow = item
            .get("eyebrow")
            .or_else(|| item.get("eyebrowTag"))
            .and_then(|v| v.as_str())
            .unwrap_or(if idx == 0 {
                "EXECUTIVE BRIEF"
            } else {
                "INSIGHT"
            })
            .to_string();
        let bullets: Vec<String> = item
            .get("bullets")
            .and_then(|v| v.as_array())
            .map(|b_arr| {
                b_arr
                    .iter()
                    .filter_map(|b| b.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let notes = item
            .get("speakerNotes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let wants_chart = item
            .get("includeChart")
            .and_then(|v| v.as_bool())
            .unwrap_or(idx == 1);
        let (charts_json, chart_title, layout_kind) = if !chart_attached && wants_chart {
            if let Some(chart) = normalized_chart {
                chart_attached = true;
                (
                    Some(chart.canonical_charts_json.clone()),
                    Some(chart.spec.title.clone()),
                    SlideLayoutKind::DataTableReport,
                )
            } else {
                (None, None, SlideLayoutKind::ProblemSolution)
            }
        } else if idx == 0 {
            (None, None, SlideLayoutKind::HeroCover)
        } else {
            (None, None, SlideLayoutKind::KpiMetricGrid)
        };
        drafts.push(DraftSlideInput {
            layout_kind,
            eyebrow_tag: eyebrow,
            headline,
            bullets,
            speaker_notes: notes,
            charts_json,
            chart_title,
        });
    }
    drafts
}

fn build_deterministic_drafts(
    title: &str,
    brief: &str,
    frame_preset: StageshowFramePreset,
    normalized_chart: Option<&NormalizedChartOutput>,
) -> Vec<DraftSlideInput> {
    let sentences = extract_sentences_from_brief(brief);
    let mut drafts = Vec::new();

    // Slide 1: Hero Cover with crisp framing bullets
    let cover_bullets = if let Some(first) = sentences.first() {
        vec![first.clone()]
    } else {
        vec![format!(
            "Structured {} deck overview",
            frame_preset.as_str()
        )]
    };
    drafts.push(DraftSlideInput {
        layout_kind: SlideLayoutKind::HeroCover,
        eyebrow_tag: format!("{} BRIEF", frame_preset.as_str().to_ascii_uppercase()),
        headline: title.to_string(),
        bullets: cover_bullets,
        speaker_notes: clean_inline_text(brief).chars().take(280).collect(),
        charts_json: None,
        chart_title: None,
    });

    // Slide 2: Core insights from remaining brief statements (auto-splits if > 30 words!)
    let insight_bullets: Vec<String> = if sentences.len() > 1 {
        sentences[1..].to_vec()
    } else if !sentences.is_empty() {
        sentences.clone()
    } else {
        vec![
            "Align core product narrative goals".to_string(),
            "Highlight measurable quarterly impact drivers".to_string(),
        ]
    };
    drafts.push(DraftSlideInput {
        layout_kind: SlideLayoutKind::ProblemSolution,
        eyebrow_tag: "KEY INSIGHTS".to_string(),
        headline: "Core Findings & Strategic Focus".to_string(),
        bullets: insight_bullets,
        speaker_notes: "Distilled executive bullets obeying the 30-word stageshow budget."
            .to_string(),
        charts_json: None,
        chart_title: None,
    });

    // Slide 3 (when CSV/TSV/PDF chart provided): Real ChartSpecV12 data slide
    if let Some(chart) = normalized_chart {
        let top_bullets: Vec<String> = chart
            .raw_points
            .iter()
            .take(3)
            .map(|pt| {
                if chart.requested_kind.is_proportional() {
                    format!(
                        "{}: {} ({:.1}%)",
                        pt.category, pt.raw_display, pt.normalized_value
                    )
                } else {
                    format!("{}: {} recorded", pt.category, pt.raw_display)
                }
            })
            .collect();
        drafts.push(DraftSlideInput {
            layout_kind: SlideLayoutKind::DataTableReport,
            eyebrow_tag: format!(
                "VERIFIED {}",
                chart.source_format.as_str().to_ascii_uppercase()
            ),
            headline: chart.spec.title.clone(),
            bullets: top_bullets,
            speaker_notes: format!(
                "Normalized {} rows across {} series from {}.",
                chart.row_count,
                chart.series_count,
                chart.source_format.as_str()
            ),
            charts_json: Some(chart.canonical_charts_json.clone()),
            chart_title: Some(chart.spec.title.clone()),
        });
    }

    drafts
}

/// Generates a complete multi-slide `stageshow-slide` deck enforcing `3..=7` words/bullet,
/// `<= 30` words/slide, automatic overflow splitting into fresh ULID slides, and `<= 512 KB/slide`.
pub fn generate_stageshow_deck(
    input: &GenerateDeckSpecInput,
) -> Result<GeneratedDeckOutput, DeckGeneratorError> {
    let title = clean_inline_text(&input.title);
    if title.is_empty() || title.chars().count() > 200 {
        return Err(DeckGeneratorError::InvalidInput(
            "Deck title must be between 1 and 200 characters".to_string(),
        ));
    }
    let brief = clean_inline_text(&input.brief);
    if brief.is_empty() {
        return Err(DeckGeneratorError::InvalidInput(
            "Deck brief cannot be empty".to_string(),
        ));
    }

    let mut drafts = input
        .llm_slides_json
        .as_ref()
        .map(|val| parse_llm_slides_drafts(val, input.normalized_chart.as_ref()))
        .unwrap_or_default();
    if drafts.is_empty() {
        drafts = build_deterministic_drafts(
            &title,
            &brief,
            input.frame_preset,
            input.normalized_chart.as_ref(),
        );
    }

    let mut slides: Vec<GeneratedSlideArtifact> = Vec::new();
    let mut total_auto_splits = 0usize;

    for draft in &drafts {
        let (emitted, splits) = enforce_slide_word_budget_and_split(
            draft,
            slides.len(),
            input.frame_preset,
            &input.theme_preset,
            &input.aspect_ratio,
            &input.primary_tokens_css,
        )?;
        total_auto_splits += splits;
        slides.extend(emitted);
        if slides.len() > MAX_GENERATED_SLIDES {
            return Err(DeckGeneratorError::SlideLimitExceeded {
                actual: slides.len(),
                max: MAX_GENERATED_SLIDES,
            });
        }
    }

    let max_words_per_slide = slides.iter().map(|s| s.word_count).max().unwrap_or(0);
    let total_slides = slides.len();

    Ok(GeneratedDeckOutput {
        title,
        frame_preset: input.frame_preset,
        theme_preset: input.theme_preset.clone(),
        aspect_ratio: input.aspect_ratio.clone(),
        slides,
        total_slides,
        auto_split_count: total_auto_splits,
        max_words_per_slide,
    })
}

const IMPERATIVE_PREFIXES: &[&str] = &[
    "please ",
    "rewrite ",
    "refine ",
    "update ",
    "tighten ",
    "summarize ",
    "make this ",
    "make it ",
    "change to ",
    "convert to ",
    "turn into ",
];

fn strip_instruction_prefix(prompt: &str) -> String {
    let cleaned = clean_inline_text(prompt);
    let lower = cleaned.to_ascii_lowercase();
    for prefix in IMPERATIVE_PREFIXES {
        if lower.starts_with(prefix) {
            let stripped = cleaned[prefix.len()..]
                .trim()
                .trim_start_matches(':')
                .trim();
            if !stripped.is_empty() {
                return stripped.to_string();
            }
        }
    }
    cleaned
}

/// Synthesizes a structured `stageshow-slide` slot rewrite, retiring raw `prompt.chars().take(160)` echo stubs.
/// Guarantees `primary_text` stays within remaining slide word budget (`30.saturating_sub(existing_other_slots_words)`)
/// and emits `overflow_bullets` for atomic continuation slides.
pub fn synthesize_slide_rewrite(input: &SlideRewriteInput) -> SlideRewriteOutput {
    let used_llm = input
        .llm_output
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());

    let raw_candidate = if let Some(ref llm_text) = input.llm_output {
        let trimmed = clean_inline_text(llm_text);
        if !trimmed.is_empty() {
            trimmed
        } else {
            derive_structured_rewrite_text(input)
        }
    } else {
        derive_structured_rewrite_text(input)
    };

    let slot_budget = STAGESHOW_MAX_WORDS_PER_SLIDE
        .saturating_sub(input.existing_other_slots_words)
        .clamp(1, STAGESHOW_MAX_WORDS_PER_SLIDE);

    let sentences = extract_sentences_from_brief(&raw_candidate);
    let bullets = if sentences.len() > 1 || count_words(&raw_candidate) > slot_budget {
        normalize_bullets_to_budget(&sentences)
    } else {
        Vec::new()
    };

    if bullets.is_empty() {
        let words: Vec<&str> = raw_candidate.split_whitespace().collect();
        if words.len() <= slot_budget {
            return SlideRewriteOutput {
                primary_text: words.join(" "),
                word_count: words.len(),
                overflow_bullets: Vec::new(),
                was_condensed_or_split: false,
                used_llm_output: used_llm,
            };
        }
        let primary = words[..slot_budget].join(" ");
        let rem_str = words[slot_budget..].join(" ");
        let overflow = normalize_bullets_to_budget(&[rem_str]);
        return SlideRewriteOutput {
            word_count: count_words(&primary),
            primary_text: primary,
            overflow_bullets: overflow,
            was_condensed_or_split: true,
            used_llm_output: used_llm,
        };
    }

    let mut primary_bullets: Vec<String> = Vec::new();
    let mut overflow_bullets: Vec<String> = Vec::new();
    let mut running_words = 0usize;
    for bullet in bullets {
        let bw = count_words(&bullet);
        if running_words + bw <= slot_budget {
            running_words += bw;
            primary_bullets.push(bullet);
        } else if primary_bullets.is_empty() {
            let words: Vec<&str> = bullet.split_whitespace().collect();
            let head = words[..slot_budget.min(words.len())].join(" ");
            running_words = count_words(&head);
            primary_bullets.push(head);
            if words.len() > slot_budget {
                let tail = words[slot_budget..].join(" ");
                overflow_bullets.extend(normalize_bullets_to_budget(&[tail]));
            }
        } else {
            overflow_bullets.push(bullet);
        }
    }

    let primary_text = primary_bullets.join("; ");
    let word_count = count_words(&primary_text);
    let was_condensed_or_split = !overflow_bullets.is_empty();

    SlideRewriteOutput {
        primary_text,
        word_count,
        overflow_bullets,
        was_condensed_or_split,
        used_llm_output: used_llm,
    }
}

fn derive_structured_rewrite_text(input: &SlideRewriteInput) -> String {
    let prompt_core = strip_instruction_prefix(&input.prompt);
    let current_clean = clean_inline_text(&input.current_text);
    let slot_kind = input.slot_kind.trim().to_ascii_lowercase();

    // If prompt is already rich multi-sentence copy, distill directly via stageshow rules
    if count_words(&prompt_core) > 12 {
        return prompt_core;
    }

    // Never return raw prompt verbatim: combine slot semantics + current copy + directive concisely
    if slot_kind == "heading" {
        let base = if !prompt_core.is_empty() {
            prompt_core
        } else if !current_clean.is_empty() {
            current_clean
        } else {
            "Executive Key Outcome".to_string()
        };
        let concise = condense_headline(&base, "Executive Key Outcome");
        if concise.eq_ignore_ascii_case(input.prompt.trim()) {
            format!("{concise} — {} Focus", input.frame_preset.as_str())
        } else {
            concise
        }
    } else if !current_clean.is_empty() && !prompt_core.is_empty() {
        let merged = format!("{current_clean}. {prompt_core}");
        let bullets = normalize_bullets_to_budget(&extract_sentences_from_brief(&merged));
        if bullets.is_empty() {
            format!("{} — Verified Update", prompt_core)
        } else {
            bullets.join(". ")
        }
    } else if !prompt_core.is_empty() {
        let bullets = normalize_bullets_to_budget(std::slice::from_ref(&prompt_core));
        let distilled = bullets.join(". ");
        if distilled.eq_ignore_ascii_case(input.prompt.trim()) {
            format!("{} · {} Ready", distilled, input.frame_preset.as_str())
        } else {
            distilled
        }
    } else {
        format!(
            "Concise {} narrative aligned to slot {}",
            input.frame_preset.as_str(),
            input.target_ff_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_splits_overflow_brief_into_continuation_slides_with_fresh_ulids() {
        let long_brief = "Revenue accelerated across enterprise accounts during the quarter with strong net retention. \
            Self-serve onboarding reduced activation time by forty percent across all regions. \
            Customer support automation resolved eighty percent of routine workspace requests immediately. \
            Platform reliability reached ninety nine point nine percent uptime under peak load. \
            Security compliance achieved full SOC2 readiness with zero critical findings recorded. \
            Next quarter prioritizes multi-region latency optimization and executive analytics rollout.";

        let deck = generate_stageshow_deck(&GenerateDeckSpecInput {
            title: "Q4 Executive Review".into(),
            brief: long_brief.into(),
            frame_preset: StageshowFramePreset::Window,
            theme_preset: "night".into(),
            aspect_ratio: "16:9".into(),
            primary_tokens_css: String::new(),
            normalized_chart: None,
            llm_slides_json: None,
        })
        .expect("generate deck");

        assert!(deck.auto_split_count >= 1, "expected overflow auto-split");
        assert!(deck.max_words_per_slide <= STAGESHOW_MAX_WORDS_PER_SLIDE);
        let mut uids = std::collections::HashSet::new();
        for slide in &deck.slides {
            assert!(slide.word_count <= STAGESHOW_MAX_WORDS_PER_SLIDE);
            assert!(slide.html.len() <= MAX_SLIDE_HTML_BYTES);
            assert!(slide.html.contains("data-stageshow-frame=\"Window\""));
            assert!(uids.insert(slide.slide_uid.clone()));
            for b in &slide.bullets {
                let wc = count_words(b);
                assert!(
                    (STAGESHOW_MIN_BULLET_WORDS..=STAGESHOW_MAX_BULLET_WORDS).contains(&wc),
                    "bullet '{b}' has {wc} words"
                );
            }
        }
    }

    #[test]
    fn rewrite_retires_echo_and_splits_overflow_past_30_words() {
        let long_prompt = "Rewrite: Our unified design workspace accelerates product teams from raw brief to validated HTML prototypes while eliminating manual CSS drift and keeping every executive stakeholder synchronized across desktop window and mobile phone frames with atomic SQLite persistence and zero latency.";
        let out = synthesize_slide_rewrite(&SlideRewriteInput {
            prompt: long_prompt.into(),
            current_text: "Old headline".into(),
            slot_kind: "text".into(),
            target_ff_id: "ff-s1-b1".into(),
            frame_preset: StageshowFramePreset::Phone,
            llm_output: None,
            existing_other_slots_words: 12,
        });
        assert_ne!(
            out.primary_text,
            long_prompt.chars().take(160).collect::<String>()
        );
        assert!(out.word_count <= STAGESHOW_MAX_WORDS_PER_SLIDE - 12);
        assert!(!out.overflow_bullets.is_empty());
    }

    #[test]
    fn assembles_one_page_html_with_all_slide_sections_preserved() {
        let deck = generate_stageshow_deck(&GenerateDeckSpecInput {
            title: "One Page Stageshow".into(),
            brief: "First insight drives adoption. Second insight lifts retention.".into(),
            frame_preset: StageshowFramePreset::Stage,
            theme_preset: "night".into(),
            aspect_ratio: "16:9".into(),
            primary_tokens_css: String::new(),
            normalized_chart: None,
            llm_slides_json: None,
        })
        .expect("generate deck");
        let one_page = assemble_one_page_stageshow_html(
            &deck.slides,
            deck.frame_preset,
            &deck.theme_preset,
            &deck.aspect_ratio,
            "",
        )
        .expect("assemble one-page");
        for slide in &deck.slides {
            assert!(one_page.contains(&format!("data-slide-uid=\"{}\"", slide.slide_uid)));
            assert!(
                slide.word_count >= count_words(&slide.eyebrow_tag) + count_words(&slide.headline)
            );
        }
    }
}
