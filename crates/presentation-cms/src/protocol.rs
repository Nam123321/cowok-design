// @story 06.1
// @implements AC-01, AC-04, AC-05
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;
#[cfg(feature = "ts")]
use ts_rs::TS;

pub const MAX_SLIDE_HTML_BYTES: usize = 524_288;

/// Wire contract for bidirectional Host <-> Sandboxed Slide Iframe messages (`Story 06.1` L2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum DeckProtocolMessage {
    DeckProtocolInit {
        epoch: i64,
        seq: i64,
        protocol_version: String,
        slide_count: i64,
        active_slide_index: i64,
        unique_ff_ids_count: i64,
    },
    DeckKeyNav {
        epoch: i64,
        seq: i64,
        direction: String,
        target_index: Option<i64>,
    },
    SlideChange {
        epoch: i64,
        seq: i64,
        slide_index: i64,
        slide_uid: Option<String>,
    },
    SlideOverflowWarn {
        epoch: i64,
        seq: i64,
        slide_index: i64,
        scroll_height: i64,
        client_height: i64,
    },
    PatchAck {
        epoch: i64,
        seq: i64,
        ff_id: String,
        applied_at_ms: i64,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolValidationError {
    #[error("E_PAYLOAD_TOO_LARGE: Slide HTML byte length {actual} exceeds {max}")]
    PayloadTooLarge { actual: usize, max: usize },
    #[error("E_DECK_PROTOCOL_INVALID: Missing .slide or .slide.active element")]
    MissingActiveSlide,
    #[error("E_DECK_PROTOCOL_INVALID: Duplicate data-ff-id attribute '{0}'")]
    DuplicateFfId(String),
    #[error("E_DECK_PROTOCOL_INVALID: Missing required [data-ff-id] slots on slide")]
    MissingFfIds,
    #[error("E_MXSS_BLOCKED: Rejected forbidden HTML construct '{0}'")]
    ForbiddenHtmlConstruct(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSlideHtml {
    pub sanitized_html: String,
    pub unique_ff_ids: Vec<String>,
    pub has_active_slide: bool,
    pub has_deck_protocol_attr: bool,
}

/// Strips untrusted scripts, inline event handlers, dangerous tags
/// (`script`, `foreignobject`, `iframe`, `object`, `embed`, `svg`, `template`, `noscript`, `meta`, `base`),
/// and `javascript:`/`vbscript:`/`data:text/html`/`data:image/svg+xml` URIs (including whitespace/entity obfuscations).
pub fn sanitize_slide_html_fragment(raw_html: &str) -> Result<String, ProtocolValidationError> {
    if raw_html.len() > MAX_SLIDE_HTML_BYTES {
        return Err(ProtocolValidationError::PayloadTooLarge {
            actual: raw_html.len(),
            max: MAX_SLIDE_HTML_BYTES,
        });
    }

    let mut cleaned = raw_html.to_string();
    const MAX_STRIP_PASSES: usize = 8;
    let mut converged = false;
    for _ in 0..MAX_STRIP_PASSES {
        let mut next = strip_tag_block(&cleaned, "script");
        for block_tag in [
            "foreignobject",
            "iframe",
            "object",
            "embed",
            "svg",
            "template",
            "noscript",
        ] {
            next = strip_tag_block(&next, block_tag);
        }
        for void_tag in ["meta", "base"] {
            next = strip_void_tag(&next, void_tag);
        }
        next = strip_inline_event_handlers(&next);
        if next == cleaned {
            converged = true;
            break;
        }
        cleaned = next;
    }
    if !converged {
        return Err(ProtocolValidationError::ForbiddenHtmlConstruct(
            "E_MXSS_BLOCKED: nested tag stripping exceeded pass limit".to_string(),
        ));
    }

    let normalized = normalize_uri_probe(&cleaned);
    for forbidden in [
        "javascript:",
        "vbscript:",
        "data:text/html",
        "data:image/svg+xml",
    ] {
        if normalized.contains(forbidden) {
            return Err(ProtocolValidationError::ForbiddenHtmlConstruct(
                forbidden.to_string(),
            ));
        }
    }

    validate_inline_and_style_blocks(&cleaned)?;
    Ok(cleaned)
}

fn validate_inline_and_style_blocks(html: &str) -> Result<(), ProtocolValidationError> {
    let lower = html.to_ascii_lowercase();
    let mut pos = 0usize;
    while let Some(rel_start) = lower[pos..].find("<style") {
        let abs_start = pos + rel_start;
        let Some(tag_end_rel) = lower[abs_start..].find('>') else {
            return Err(ProtocolValidationError::ForbiddenHtmlConstruct(
                "E_MXSS_BLOCKED: unclosed style tag".to_string(),
            ));
        };
        let content_start = abs_start + tag_end_rel + 1;
        let Some(close_rel) = lower[content_start..].find("</style") else {
            return Err(ProtocolValidationError::ForbiddenHtmlConstruct(
                "E_MXSS_BLOCKED: unclosed style block".to_string(),
            ));
        };
        let css_block = &html[content_start..content_start + close_rel];
        crate::cms_patcher::validate_and_scrub_css_tokens(css_block, false)
            .map_err(|err| ProtocolValidationError::ForbiddenHtmlConstruct(err.to_string()))?;
        pos = content_start + close_rel + "</style".len();
    }
    for style_attr in try_extract_attribute_values(html, "style")? {
        crate::cms_patcher::validate_and_scrub_css_tokens(&style_attr, true)
            .map_err(|err| ProtocolValidationError::ForbiddenHtmlConstruct(err.to_string()))?;
    }
    Ok(())
}

/// Validates `<section class="... slide active ...">` presence and `[data-ff-id]` uniqueness across a slide document.
pub fn validate_slide_protocol_structure(
    raw_html: &str,
) -> Result<ValidatedSlideHtml, ProtocolValidationError> {
    let sanitized_html = sanitize_slide_html_fragment(raw_html)?;
    let lower = sanitized_html.to_ascii_lowercase();

    let has_active_slide = has_active_slide_section(&sanitized_html);
    if !has_active_slide {
        return Err(ProtocolValidationError::MissingActiveSlide);
    }

    let ff_ids = extract_attribute_values(&sanitized_html, "data-ff-id");
    if ff_ids.is_empty() {
        return Err(ProtocolValidationError::MissingFfIds);
    }

    let mut seen = HashSet::new();
    for id in &ff_ids {
        if id.trim().is_empty() || !seen.insert(id.clone()) {
            return Err(ProtocolValidationError::DuplicateFfId(id.clone()));
        }
    }

    Ok(ValidatedSlideHtml {
        has_deck_protocol_attr: lower.contains("data-cowok-deck-protocol=\"1\"")
            || lower.contains("data-cowok-deck-protocol='1'"),
        sanitized_html,
        unique_ff_ids: ff_ids,
        has_active_slide,
    })
}

const MAX_DUP_FF_ID_LEN: usize = 64;

fn build_bounded_duplicate_ff_id(
    old_id: &str,
    suffix_token: &str,
    used: &mut HashSet<String>,
) -> String {
    let clean_suffix: String = suffix_token
        .bytes()
        .filter(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
        .take(24)
        .map(char::from)
        .collect();
    let suffix = if clean_suffix.is_empty() {
        "dup"
    } else {
        clean_suffix.as_str()
    };
    let candidate = format!("{old_id}-{suffix}");
    if candidate.len() <= MAX_DUP_FF_ID_LEN && !used.contains(&candidate) {
        used.insert(candidate.clone());
        return candidate;
    }
    let digest = &crate::templates::compute_sha256_hex(old_id.as_bytes())[..8];
    let tail = format!("-{digest}-{suffix}");
    let prefix_max = MAX_DUP_FF_ID_LEN.saturating_sub(tail.len());
    let mut next_id = format!("{}{tail}", &old_id[..old_id.len().min(prefix_max)]);
    let mut disambig = 0usize;
    while used.contains(&next_id) || next_id.len() > MAX_DUP_FF_ID_LEN {
        let indexed_tail = format!("-{digest}{disambig}-{suffix}");
        let idx_prefix = MAX_DUP_FF_ID_LEN.saturating_sub(indexed_tail.len());
        next_id = format!("{}{indexed_tail}", &old_id[..old_id.len().min(idx_prefix)]);
        disambig += 1;
    }
    used.insert(next_id.clone());
    next_id
}

/// Clones slide HTML for duplication with fresh `slide_uid` and fresh unique `data-ff-id` attributes.
pub fn rewrite_slide_html_for_duplicate(
    source_html: &str,
    new_slide_uid: &str,
    suffix_token: &str,
) -> Result<(String, Vec<(String, String)>), ProtocolValidationError> {
    let validated = validate_slide_protocol_structure(source_html)?;
    let mut updated = validated.sanitized_html.clone();
    let mut id_map = Vec::with_capacity(validated.unique_ff_ids.len());
    let mut used_ids: HashSet<String> = HashSet::with_capacity(validated.unique_ff_ids.len());

    if let Some(existing_uid) = extract_attribute_values(&updated, "data-slide-uid")
        .into_iter()
        .next()
    {
        updated = replace_attr_exact(&updated, "data-slide-uid", &existing_uid, new_slide_uid);
    }

    for old_id in validated.unique_ff_ids {
        let new_id = build_bounded_duplicate_ff_id(&old_id, suffix_token, &mut used_ids);
        updated = replace_attr_exact(&updated, "data-ff-id", &old_id, &new_id);
        id_map.push((old_id, new_id));
    }

    Ok((updated, id_map))
}

fn replace_attr_exact(html: &str, attr_name: &str, old_val: &str, new_val: &str) -> String {
    let double_pat = format!("{attr_name}=\"{old_val}\"");
    let double_rep = format!("{attr_name}=\"{new_val}\"");
    let single_pat = format!("{attr_name}='{old_val}'");
    let single_rep = format!("{attr_name}=\"{new_val}\"");
    html.replace(&double_pat, &double_rep)
        .replace(&single_pat, &single_rep)
}

fn find_unquoted_tag_end(input: &str, start_idx: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut i = start_idx;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => quote = Some(b),
            None if b == b'>' => return Some(i),
            None => {}
        }
        i += 1;
    }
    None
}

fn strip_tag_block(input: &str, tag: &str) -> String {
    let lower_tag = tag.to_ascii_lowercase();
    let open_pat = format!("<{lower_tag}");
    let close_pat = format!("</{lower_tag}");
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0usize;

    while cursor < input.len() {
        let rem = &input[cursor..];
        let rem_lower = rem.to_ascii_lowercase();
        let Some(start_rel) = rem_lower.find(&open_pat) else {
            out.push_str(rem);
            break;
        };
        let abs_start = cursor + start_rel;
        let next_char = rem_lower
            .as_bytes()
            .get(start_rel + open_pat.len())
            .copied();
        let is_boundary = matches!(
            next_char,
            Some(b'>' | b' ' | b'\n' | b'\r' | b'\t' | b'/') | None
        );
        if !is_boundary {
            out.push_str(&input[cursor..abs_start + open_pat.len()]);
            cursor = abs_start + open_pat.len();
            continue;
        }
        out.push_str(&input[cursor..abs_start]);
        let Some(open_end) = find_unquoted_tag_end(input, abs_start + open_pat.len()) else {
            break;
        };
        let body_start = open_end + 1;
        let body_lower = input[body_start..].to_ascii_lowercase();
        let mut search_pos = 0usize;
        let mut closed_at = None;
        while let Some(c_rel) = body_lower[search_pos..].find(&close_pat) {
            let candidate = search_pos + c_rel;
            let after_name = body_lower
                .as_bytes()
                .get(candidate + close_pat.len())
                .copied();
            if matches!(after_name, Some(b'>' | b' ' | b'\n' | b'\r' | b'\t' | b'/')) {
                if let Some(gt_rel) = input[body_start + candidate..].find('>') {
                    closed_at = Some(body_start + candidate + gt_rel + 1);
                }
                break;
            }
            search_pos = candidate + close_pat.len();
        }
        if let Some(end_idx) = closed_at {
            cursor = end_idx;
        } else {
            // Unclosed forbidden container: drop tail rather than leaving active markup
            break;
        }
    }
    out
}

fn strip_void_tag(input: &str, tag: &str) -> String {
    let open_pat = format!("<{}", tag.to_ascii_lowercase());
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0usize;
    while cursor < input.len() {
        let rem = &input[cursor..];
        let rem_lower = rem.to_ascii_lowercase();
        let Some(start_rel) = rem_lower.find(&open_pat) else {
            out.push_str(rem);
            break;
        };
        let abs_start = cursor + start_rel;
        let next_char = rem_lower
            .as_bytes()
            .get(start_rel + open_pat.len())
            .copied();
        let is_boundary = matches!(
            next_char,
            Some(b'>' | b' ' | b'\n' | b'\r' | b'\t' | b'/') | None
        );
        if !is_boundary {
            out.push_str(&input[cursor..abs_start + open_pat.len()]);
            cursor = abs_start + open_pat.len();
            continue;
        }
        out.push_str(&input[cursor..abs_start]);
        if let Some(tag_end) = find_unquoted_tag_end(input, abs_start + open_pat.len()) {
            cursor = tag_end + 1;
        } else {
            break;
        }
    }
    out
}

fn normalize_uri_probe(input: &str) -> String {
    let mut decoded = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'&' && bytes.get(i + 1) == Some(&b'#') {
            if let Some(semi_rel) = input[i + 2..].find(';').filter(|&r| r <= 8) {
                let raw_num = &input[i + 2..i + 2 + semi_rel];
                let code = if let Some(hex) = raw_num
                    .strip_prefix('x')
                    .or_else(|| raw_num.strip_prefix('X'))
                {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    raw_num.parse::<u32>().ok()
                };
                if let Some(ch) = code.and_then(char::from_u32) {
                    if !ch.is_whitespace() && !ch.is_control() {
                        decoded.push(ch.to_ascii_lowercase());
                    }
                    i = i + 2 + semi_rel + 1;
                    continue;
                }
            }
        }
        let ch = bytes[i] as char;
        if !ch.is_ascii_whitespace() && !ch.is_control() {
            decoded.push(ch.to_ascii_lowercase());
        }
        i += 1;
    }
    decoded
}

fn strip_inline_event_handlers(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0usize;
    while let Some(rel_lt) = input[cursor..].find('<') {
        let tag_start = cursor + rel_lt;
        out.push_str(&input[cursor..tag_start]);
        if input[tag_start..].starts_with("</") || input[tag_start..].starts_with("<!") {
            let end_idx = find_unquoted_tag_end(input, tag_start + 1)
                .map(|e| e + 1)
                .unwrap_or(input.len());
            out.push_str(&input[tag_start..end_idx]);
            cursor = end_idx;
            continue;
        }
        let Some(tag_end) = find_unquoted_tag_end(input, tag_start + 1) else {
            out.push_str(&input[tag_start..]);
            return out;
        };
        out.push_str(&sanitize_open_tag(&input[tag_start..=tag_end]));
        cursor = tag_end + 1;
    }
    out.push_str(&input[cursor..]);
    out
}

fn sanitize_open_tag(tag_slice: &str) -> String {
    let inner = &tag_slice[1..tag_slice.len().saturating_sub(1)];
    let mut i = 0usize;
    let bytes = inner.as_bytes();
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'/' {
        i += 1;
    }
    let mut cleaned = String::with_capacity(tag_slice.len());
    cleaned.push('<');
    cleaned.push_str(&inner[..i]);

    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            cleaned.push(bytes[i] as char);
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'/'
        {
            i += 1;
        }
        let attr_name = &inner[name_start..i];
        let mut val_cursor = i;
        while val_cursor < bytes.len() && bytes[val_cursor].is_ascii_whitespace() {
            val_cursor += 1;
        }
        let attr_end = if val_cursor < bytes.len() && bytes[val_cursor] == b'=' {
            val_cursor += 1;
            while val_cursor < bytes.len() && bytes[val_cursor].is_ascii_whitespace() {
                val_cursor += 1;
            }
            if val_cursor < bytes.len() && (bytes[val_cursor] == b'"' || bytes[val_cursor] == b'\'')
            {
                let q = bytes[val_cursor];
                val_cursor += 1;
                while val_cursor < bytes.len() && bytes[val_cursor] != q {
                    val_cursor += 1;
                }
                if val_cursor < bytes.len() {
                    val_cursor + 1
                } else {
                    bytes.len()
                }
            } else {
                while val_cursor < bytes.len() && !bytes[val_cursor].is_ascii_whitespace() {
                    val_cursor += 1;
                }
                val_cursor
            }
        } else {
            i
        };
        let lower_name = attr_name.to_ascii_lowercase();
        let is_event_handler = lower_name.len() >= 3
            && lower_name.starts_with("on")
            && lower_name[2..].bytes().all(|b| b.is_ascii_alphanumeric());
        if !is_event_handler {
            cleaned.push_str(&inner[name_start..attr_end]);
        }
        i = attr_end;
    }
    cleaned.push('>');
    cleaned
}

fn has_active_slide_section(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(rel) = lower[cursor..].find("<section") {
        let start = cursor + rel;
        let next = lower.as_bytes().get(start + "<section".len()).copied();
        if !matches!(next, Some(b'>' | b' ' | b'\n' | b'\r' | b'\t' | b'/')) {
            cursor = start + "<section".len();
            continue;
        }
        let Some(end) = find_unquoted_tag_end(html, start + "<section".len()) else {
            break;
        };
        let open_tag = &html[start..=end];
        for class_val in extract_attribute_values(open_tag, "class") {
            let tokens: Vec<&str> = class_val.split_ascii_whitespace().collect();
            if tokens.contains(&"slide") && tokens.contains(&"active") {
                return true;
            }
        }
        cursor = end + 1;
    }
    false
}

fn extract_attribute_values(html: &str, attr_name: &str) -> Vec<String> {
    try_extract_attribute_values(html, attr_name).unwrap_or_default()
}

fn try_extract_attribute_values(
    html: &str,
    attr_name: &str,
) -> Result<Vec<String>, ProtocolValidationError> {
    let mut values = Vec::new();
    let target_lower = attr_name.to_ascii_lowercase();
    let lower = html.to_ascii_lowercase();
    let mut offset = 0usize;
    while let Some(pos) = lower[offset..].find(&target_lower) {
        let name_start = offset + pos;
        let prev = if name_start > 0 {
            lower.as_bytes()[name_start - 1]
        } else {
            b' '
        };
        let valid_prefix =
            name_start == 0 || matches!(prev, b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'"' | b'\'');
        let after_name = name_start + target_lower.len();
        if !valid_prefix {
            offset = after_name;
            continue;
        }
        let bytes = html.as_bytes();
        let mut cur = after_name;
        while cur < bytes.len() && bytes[cur].is_ascii_whitespace() {
            cur += 1;
        }
        if cur >= bytes.len() || bytes[cur] != b'=' {
            offset = after_name;
            continue;
        }
        cur += 1;
        while cur < bytes.len() && bytes[cur].is_ascii_whitespace() {
            cur += 1;
        }
        if cur >= bytes.len() {
            break;
        }
        let q = bytes[cur];
        if q == b'"' || q == b'\'' {
            let val_start = cur + 1;
            if let Some(end_rel) = html[val_start..].find(q as char) {
                values.push(html[val_start..val_start + end_rel].to_string());
                offset = val_start + end_rel + 1;
                continue;
            }
            return Err(ProtocolValidationError::ForbiddenHtmlConstruct(
                "E_MXSS_BLOCKED: unclosed quoted attribute".to_string(),
            ));
        }
        let val_start = cur;
        let mut val_end = cur;
        while val_end < bytes.len()
            && !bytes[val_end].is_ascii_whitespace()
            && bytes[val_end] != b'>'
        {
            val_end += 1;
        }
        if val_end > val_start {
            values.push(html[val_start..val_end].to_string());
        }
        offset = val_end.max(cur + 1);
    }
    Ok(values)
}
