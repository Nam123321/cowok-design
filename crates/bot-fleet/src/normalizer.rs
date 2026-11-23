// @story 11.1
// @implements AC-04 (Post-Execution Normalizer: ANSI Strip, JSON-First Safe Coerce & Hardened SVG XSS + viewBox Parity)
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

const MAX_SANITIZE_PASSES: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizationReport {
    pub normalized_content: String,
    pub ansi_sequences_stripped: usize,
    pub json_blocks_extracted: usize,
    pub svg_viewboxes_injected: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted_json: Option<serde_json::Value>,
}

fn ansi_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-Z\\-_]")
            .expect("valid ANSI regex")
    })
}

fn fenced_block_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?s)```(?:json|JSON)?\s*\n(.*?)\n```").expect("valid fenced block regex")
    })
}

fn trailing_comma_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r",(\s*[}\]])").expect("valid trailing comma regex"))
}

fn svg_open_tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<svg\b([^>]*)>").expect("valid svg open tag regex"))
}

fn svg_viewbox_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bviewbox\s*=").expect("valid viewbox attr regex"))
}

fn svg_width_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)\bwidth\s*=\s*["']\s*([0-9]+(?:\.[0-9]+)?)"#)
            .expect("valid width attr regex")
    })
}

fn svg_height_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)\bheight\s*=\s*["']\s*([0-9]+(?:\.[0-9]+)?)"#)
            .expect("valid height attr regex")
    })
}

fn svg_dangerous_paired_tags_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?is)<(script|foreignObject|iframe|object|embed|animate|set)\b[^>]*>.*?</\s*(script|foreignObject|iframe|object|embed|animate|set)\s*>",
        )
        .expect("valid dangerous paired tag regex")
    })
}

fn svg_dangerous_self_closing_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?is)<\s*/?\s*(script|foreignObject|iframe|object|embed|animate|set)\b[^>]*>")
            .expect("valid dangerous self-closing tag regex")
    })
}

fn html_tag_token_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^<>]+>").expect("valid tag token regex"))
}

fn svg_event_handler_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)([\s/"'])on[a-z]+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>/]+)"#)
            .expect("valid event handler regex")
    })
}

fn svg_any_href_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)([\s/"'])(?:xlink:)?href\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>/]+))"#)
            .expect("valid href attr regex")
    })
}

fn is_safe_svg_href(raw_val: &str) -> bool {
    let cleaned: String = raw_val
        .chars()
        .filter(|c| !c.is_control() && !c.is_whitespace())
        .collect();
    if cleaned.is_empty() || cleaned.contains('&') {
        return false;
    }
    let lower = cleaned.to_ascii_lowercase();
    lower.starts_with('#')
        || (lower.starts_with('/') && !lower.starts_with("//"))
        || lower.starts_with("https://")
        || lower.starts_with("http://")
}

fn sanitize_tag_attributes(tag: &str) -> String {
    let mut current = tag.to_string();
    for _ in 0..MAX_SANITIZE_PASSES {
        let no_events = svg_event_handler_attr_regex()
            .replace_all(&current, "$1")
            .into_owned();
        let safe_hrefs = svg_any_href_attr_regex()
            .replace_all(&no_events, |caps: &regex::Captures<'_>| {
                let delim = caps.get(1).map(|m| m.as_str()).unwrap_or(" ");
                let val = caps
                    .get(2)
                    .or_else(|| caps.get(3))
                    .or_else(|| caps.get(4))
                    .map(|m| m.as_str())
                    .unwrap_or("");
                if is_safe_svg_href(val) {
                    caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string()
                } else {
                    delim.to_string()
                }
            })
            .into_owned();
        if safe_hrefs == current {
            return safe_hrefs;
        }
        current = safe_hrefs;
    }
    current
}

/// Strips ANSI CSI/OSC escape sequences and returns `(clean_text, count_stripped)`.
pub fn strip_ansi_sequences(raw: &str) -> (String, usize) {
    let re = ansi_regex();
    let count = re.find_iter(raw).count();
    if count == 0 {
        return (raw.to_string(), 0);
    }
    (re.replace_all(raw, "").into_owned(), count)
}

/// Attempts strict JSON parsing first, then safely strips trailing commas before `}`/`]`.
pub fn coerce_json_value(candidate: &str) -> Option<serde_json::Value> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Some(val);
    }
    let repaired = trailing_comma_regex().replace_all(trimmed, "$1");
    serde_json::from_str::<serde_json::Value>(repaired.trim()).ok()
}

/// Extracts fenced JSON blocks (or raw top-level JSON objects/arrays) safely.
pub fn extract_fenced_or_raw_json(text: &str) -> (Option<serde_json::Value>, usize) {
    let mut count = 0usize;
    let mut first_valid: Option<serde_json::Value> = None;

    for caps in fenced_block_regex().captures_iter(text) {
        if let Some(body) = caps.get(1) {
            if let Some(parsed) = coerce_json_value(body.as_str()) {
                count += 1;
                if first_valid.is_none() {
                    first_valid = Some(parsed);
                }
            }
        }
    }

    if first_valid.is_none() {
        let trimmed = text.trim();
        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
        {
            if let Some(parsed) = coerce_json_value(trimmed) {
                count = 1;
                first_valid = Some(parsed);
            }
        }
    }

    (first_valid, count)
}

fn sanitize_svg_xss_fixed_point(input: &str) -> String {
    let mut current = input.to_string();
    for _ in 0..MAX_SANITIZE_PASSES {
        let step1 = svg_dangerous_paired_tags_regex()
            .replace_all(&current, "")
            .into_owned();
        let step2 = svg_dangerous_self_closing_regex()
            .replace_all(&step1, "")
            .into_owned();
        let step3 = html_tag_token_regex()
            .replace_all(&step2, |caps: &regex::Captures<'_>| {
                sanitize_tag_attributes(caps.get(0).map(|m| m.as_str()).unwrap_or(""))
            })
            .into_owned();
        if step3 == current {
            return step3;
        }
        current = step3;
    }
    current
}

/// Ensures every `<svg>` element carries a valid `viewBox` attribute (reusing Story 04.6 invariants)
/// and applies fixed-point XSS sanitization (`script`, `foreignObject`, `iframe`, `object`, `embed`,
/// `animate`, `set`, `on*` event attributes, and `javascript:`/`data:` hrefs).
pub fn normalize_svg_viewboxes(input: &str) -> (String, usize) {
    if !input.to_ascii_lowercase().contains("<svg") {
        return (input.to_string(), 0);
    }
    let sanitized = sanitize_svg_xss_fixed_point(input);
    let viewbox_re = svg_viewbox_attr_regex();
    let width_re = svg_width_attr_regex();
    let height_re = svg_height_attr_regex();
    let mut injected = 0usize;

    let replaced = svg_open_tag_regex()
        .replace_all(&sanitized, |caps: &regex::Captures<'_>| {
            let full = caps.get(0).map(|m| m.as_str()).unwrap_or("<svg>");
            let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if viewbox_re.is_match(attrs) {
                return full.to_string();
            }
            let w = width_re
                .captures(attrs)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str())
                .unwrap_or("24");
            let h = height_re
                .captures(attrs)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str())
                .unwrap_or("24");
            injected += 1;
            let trimmed_attrs = attrs.trim_end();
            if let Some(prefix) = trimmed_attrs.strip_suffix('/') {
                format!("<svg{prefix} viewBox=\"0 0 {w} {h}\" />")
            } else {
                format!("<svg{attrs} viewBox=\"0 0 {w} {h}\">")
            }
        })
        .into_owned();

    (replaced, injected)
}

pub struct PostExecutionNormalizer;

impl PostExecutionNormalizer {
    /// Runs the complete normalization pipeline: ANSI strip -> JSON extraction (before SVG rewrite) -> SVG viewBox + XSS hardening.
    pub fn normalize(raw: &str) -> NormalizationReport {
        let (no_ansi, ansi_sequences_stripped) = strip_ansi_sequences(raw);
        let (extracted_json, json_blocks_extracted) = extract_fenced_or_raw_json(&no_ansi);
        let (svg_clean, svg_viewboxes_injected) = normalize_svg_viewboxes(&no_ansi);

        NormalizationReport {
            normalized_content: svg_clean.trim().to_string(),
            ansi_sequences_stripped,
            json_blocks_extracted,
            svg_viewboxes_injected,
            extracted_json,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r2_h02_strips_slash_quote_adjacent_events_and_entity_encoded_hrefs() {
        let cases = [
            r#"<svg/onload=alert(1)></svg>"#,
            r#"<svg width="24"onload="alert(1)"></svg>"#,
            r#"<svg><a/href="javascript:alert(1)">x</a></svg>"#,
            r#"<svg><a href="&#106;avascript:alert(1)">x</a></svg>"#,
            r#"<svg><a href="java&#x0a;script:alert(1)">x</a></svg>"#,
        ];
        for raw in cases {
            let (out, _) = normalize_svg_viewboxes(raw);
            let lower = out.to_ascii_lowercase();
            assert!(!lower.contains("onload"), "leaked onload in: {out}");
            assert!(!lower.contains("javascript"), "leaked javascript in: {out}");
            assert!(!lower.contains("&#106;"), "leaked entity href in: {out}");
            assert!(
                !lower.contains("&#x0a;"),
                "leaked hex entity href in: {out}"
            );
        }
        let (safe_out, _) = normalize_svg_viewboxes(r##"<svg><a href="#icon-ok">ok</a></svg>"##);
        assert!(
            safe_out.contains(r##"href="#icon-ok""##),
            "stripped safe fragment href: {safe_out}"
        );
    }
}
