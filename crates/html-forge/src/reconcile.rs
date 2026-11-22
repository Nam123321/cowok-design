// @story 05.4
// @implements AC-04
use std::collections::{HashMap, HashSet};

use markup5ever_rcdom::{Handle, NodeData};
use serde::{Deserialize, Serialize};

use crate::patcher::{
    compute_sha256_hex, escape_html_attr, escape_html_text, is_valid_ff_id, parse_html5_dom,
    HtmlForgeError,
};

pub const DEFAULT_RECONCILE_MIN_CONFIDENCE: f64 = 0.65;
const AMBIGUITY_MARGIN: f64 = 0.08;
const VOID_HTML_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileReport {
    pub reconciled_html: String,
    pub content_sha256: String,
    pub preserved_count: usize,
    pub remapped_count: usize,
    pub total_ff_ids: usize,
    pub min_match_confidence: f64,
}

#[derive(Debug, Clone)]
struct ElementFingerprint {
    order_index: usize,
    tag: String,
    ff_id: Option<String>,
    id_attr: Option<String>,
    role: Option<String>,
    aria_label: Option<String>,
    classes: HashSet<String>,
    text: String,
    parent_tag: String,
    sibling_index: usize,
}

fn collect_direct_and_nested_text(handle: &Handle, out: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => {
            let chunk = contents.borrow();
            let trimmed = chunk.trim();
            if !trimmed.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(trimmed);
            }
        }
        NodeData::Element { .. } | NodeData::Document => {
            for child in handle.children.borrow().iter() {
                collect_direct_and_nested_text(child, out);
            }
        }
        _ => {}
    }
}

fn collect_fingerprints(handle: &Handle, parent_tag: &str, out: &mut Vec<ElementFingerprint>) {
    let mut element_sibling_idx = 0usize;
    for child in handle.children.borrow().iter() {
        if let NodeData::Element { name, attrs, .. } = &child.data {
            let tag: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
            let mut ff_id = None;
            let mut id_attr = None;
            let mut role = None;
            let mut aria_label = None;
            let mut classes = HashSet::new();

            for attr in attrs.borrow().iter() {
                let k: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
                let v = attr.value.trim().to_string();
                match k.as_str() {
                    "data-ff-id" if !v.is_empty() => ff_id = Some(v),
                    "id" if !v.is_empty() => id_attr = Some(v),
                    "role" if !v.is_empty() => role = Some(v.to_ascii_lowercase()),
                    "aria-label" if !v.is_empty() => aria_label = Some(v.to_ascii_lowercase()),
                    "class" => {
                        for tok in v.split_whitespace() {
                            classes.insert(tok.to_ascii_lowercase());
                        }
                    }
                    _ => {}
                }
            }

            let mut text = String::new();
            collect_direct_and_nested_text(child, &mut text);
            let normalized_text = text.to_ascii_lowercase();

            let order_index = out.len();
            out.push(ElementFingerprint {
                order_index,
                tag: tag.clone(),
                ff_id,
                id_attr,
                role,
                aria_label,
                classes,
                text: normalized_text,
                parent_tag: parent_tag.to_string(),
                sibling_index: element_sibling_idx,
            });
            element_sibling_idx += 1;

            collect_fingerprints(child, &tag, out);
        }
    }
}

fn token_jaccard(a: &str, b: &str) -> f64 {
    let sa: HashSet<&str> = a.split_whitespace().collect();
    let sb: HashSet<&str> = b.split_whitespace().collect();
    if sa.is_empty() && sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

fn set_jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

fn structural_similarity(prev: &ElementFingerprint, next: &ElementFingerprint) -> f64 {
    if prev.tag != next.tag {
        return 0.0;
    }
    let mut score = 0.30;

    if prev.id_attr.is_some() && prev.id_attr == next.id_attr {
        score += 0.35;
    }
    if prev.role.is_some() && prev.role == next.role {
        score += 0.15;
    }
    if prev.aria_label.is_some() && prev.aria_label == next.aria_label {
        score += 0.20;
    }

    if !prev.text.is_empty() && !next.text.is_empty() {
        if prev.text == next.text {
            score += 0.40;
        } else {
            let jaccard = token_jaccard(&prev.text, &next.text);
            if jaccard > 0.0 {
                score += 0.30 * jaccard;
            } else if prev.id_attr.is_none() || prev.id_attr != next.id_attr {
                score -= 0.15;
            }
        }
    }

    if !prev.classes.is_empty() && !next.classes.is_empty() {
        score += 0.20 * set_jaccard(&prev.classes, &next.classes);
    }

    if prev.parent_tag == next.parent_tag {
        score += 0.10;
        if prev.sibling_index == next.sibling_index {
            score += 0.05;
        }
    }

    score.clamp(0.0, 1.0)
}

/// Reconciles `data-ff-id` anchors between `prev_html` and `next_html`.
///
/// - Preserves every `data-ff-id` already present in `next_html` (confidence = `1.0`).
/// - Re-attaches missing `data-ff-id`s only when structural fingerprint similarity meets
///   `min_confidence` AND is unambiguous against sibling nodes (`0` wrong-sibling rebindings).
/// - Fails closed with `HtmlForgeError::StaleMapping` (`409 STALE_MAPPING`) when any previous
///   anchor cannot be matched reliably.
pub fn reconcile_ff_ids(
    prev_html: &str,
    next_html: &str,
    min_confidence: f64,
) -> Result<ReconcileReport, HtmlForgeError> {
    let threshold = if min_confidence.is_finite() {
        min_confidence.clamp(0.1, 1.0)
    } else {
        DEFAULT_RECONCILE_MIN_CONFIDENCE
    };

    let prev_dom = parse_html5_dom(prev_html);
    let next_dom = parse_html5_dom(next_html);

    let mut prev_nodes = Vec::new();
    let mut next_nodes = Vec::new();
    collect_fingerprints(&prev_dom.document, "#document", &mut prev_nodes);
    collect_fingerprints(&next_dom.document, "#document", &mut next_nodes);

    let prev_tagged: Vec<&ElementFingerprint> = prev_nodes
        .iter()
        .filter(|n| n.ff_id.as_deref().is_some_and(is_valid_ff_id))
        .collect();

    if prev_tagged.is_empty() {
        let sha = compute_sha256_hex(next_html.as_bytes());
        return Ok(ReconcileReport {
            reconciled_html: next_html.to_string(),
            content_sha256: sha,
            preserved_count: 0,
            remapped_count: 0,
            total_ff_ids: 0,
            min_match_confidence: 1.0,
        });
    }

    let mut claimed_next_indices: HashSet<usize> = HashSet::new();
    let mut next_existing_ff_ids: HashMap<String, usize> = HashMap::new();
    for node in &next_nodes {
        if let Some(ref fid) = node.ff_id {
            next_existing_ff_ids.insert(fid.clone(), node.order_index);
        }
    }

    let mut preserved_count = 0usize;
    let mut pending_prev: Vec<&ElementFingerprint> = Vec::new();
    for p in &prev_tagged {
        let fid = p.ff_id.as_deref().unwrap_or_default();
        if let Some(&next_idx) = next_existing_ff_ids.get(fid) {
            claimed_next_indices.insert(next_idx);
            preserved_count += 1;
        } else {
            pending_prev.push(*p);
        }
    }

    let mut assignments: HashMap<usize, String> = HashMap::new();
    let mut missing_ff_ids: Vec<String> = Vec::new();
    let mut lowest_confidence: f64 = 1.0;

    for p in pending_prev {
        let fid = p.ff_id.as_deref().unwrap_or_default().to_string();
        let mut candidates: Vec<(usize, f64)> = next_nodes
            .iter()
            .filter(|n| n.ff_id.is_none() && !claimed_next_indices.contains(&n.order_index))
            .map(|n| (n.order_index, structural_similarity(p, n)))
            .filter(|(_, s)| *s > 0.0)
            .collect();

        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        let Some(&(best_idx, best_score)) = candidates.first() else {
            missing_ff_ids.push(fid);
            lowest_confidence = 0.0;
            continue;
        };

        let second_score = candidates.get(1).map(|c| c.1).unwrap_or(0.0);
        // Never silently pick between near-identical siblings regardless of absolute score
        let is_ambiguous = candidates.len() > 1
            && (best_score - second_score) < AMBIGUITY_MARGIN
            && second_score >= threshold * 0.85;

        if best_score < threshold || is_ambiguous {
            missing_ff_ids.push(fid);
            lowest_confidence = lowest_confidence.min(best_score);
            continue;
        }

        claimed_next_indices.insert(best_idx);
        assignments.insert(best_idx, fid);
        lowest_confidence = lowest_confidence.min(best_score);
    }

    if !missing_ff_ids.is_empty() {
        return Err(HtmlForgeError::StaleMapping {
            missing_ff_ids,
            lowest_confidence,
            threshold,
        });
    }

    let remapped_count = assignments.len();
    let reconciled_html = if assignments.is_empty() {
        next_html.to_string()
    } else {
        serialize_reconciled_dom(&next_dom.document, &assignments)
    };

    let content_sha256 = compute_sha256_hex(reconciled_html.as_bytes());
    Ok(ReconcileReport {
        reconciled_html,
        content_sha256,
        preserved_count,
        remapped_count,
        total_ff_ids: preserved_count + remapped_count,
        min_match_confidence: lowest_confidence,
    })
}

const RAWTEXT_HTML_TAGS: &[&str] = &[
    "style",
    "script",
    "xmp",
    "iframe",
    "noembed",
    "noframes",
    "plaintext",
];

fn serialize_reconciled_dom(root: &Handle, assignments: &HashMap<usize, String>) -> String {
    let mut out = String::new();
    let mut cursor = 0usize;

    fn walk(
        handle: &Handle,
        assignments: &HashMap<usize, String>,
        cursor: &mut usize,
        out: &mut String,
        in_rawtext: bool,
    ) {
        match &handle.data {
            NodeData::Document => {
                for child in handle.children.borrow().iter() {
                    walk(child, assignments, cursor, out, false);
                }
            }
            NodeData::Doctype { .. } => out.push_str("<!DOCTYPE html>"),
            NodeData::Text { contents } => {
                let raw = contents.borrow();
                if in_rawtext {
                    out.push_str(&raw);
                } else {
                    out.push_str(&escape_html_text(&raw));
                }
            }
            NodeData::Element { name, attrs, .. } => {
                let my_idx = *cursor;
                *cursor += 1;
                let tag: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
                out.push('<');
                out.push_str(&tag);
                let mut has_ff = false;
                for attr in attrs.borrow().iter() {
                    let k: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
                    if k == "data-ff-id" {
                        has_ff = true;
                    }
                    out.push(' ');
                    out.push_str(&k);
                    out.push_str("=\"");
                    out.push_str(&escape_html_attr(&attr.value));
                    out.push('"');
                }
                if !has_ff {
                    if let Some(ff_id) = assignments.get(&my_idx) {
                        out.push_str(" data-ff-id=\"");
                        out.push_str(&escape_html_attr(ff_id));
                        out.push('"');
                    }
                }
                out.push('>');
                if !VOID_HTML_TAGS.contains(&tag.as_str()) {
                    let child_rawtext = RAWTEXT_HTML_TAGS.contains(&tag.as_str());
                    for child in handle.children.borrow().iter() {
                        walk(child, assignments, cursor, out, child_rawtext);
                    }
                    out.push_str("</");
                    out.push_str(&tag);
                    out.push('>');
                }
            }
            NodeData::Comment { .. } | NodeData::ProcessingInstruction { .. } => {}
        }
    }

    walk(root, assignments, &mut cursor, &mut out, false);
    out
}
