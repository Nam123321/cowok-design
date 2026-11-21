// @story 02.1
// @implements AC-01, AC-02
use crate::types::{ContextCompilerError, DomOutlineNode};
use domain::estimate_tokens;

pub const MAX_OUTLINE_BPE_TOKENS: usize = 350;
pub const MIN_SSOT_DOM_RATIO: f64 = 0.60;

fn format_outline_line(node: &DomOutlineNode) -> String {
    let role_part = node
        .role
        .as_deref()
        .map(|r| format!(" role={r}"))
        .unwrap_or_default();
    let summary_part = node
        .summary
        .as_deref()
        .map(|s| format!(": {}", s.trim()))
        .unwrap_or_default();
    format!(
        "<{} data-ff-id=\"{}\"{role_part}/>{summary_part}",
        node.tag, node.ff_id
    )
}

pub fn compress_dom_outline(nodes: &[DomOutlineNode], max_tokens: usize) -> (String, usize, usize) {
    let cap = max_tokens.min(MAX_OUTLINE_BPE_TOKENS);
    if nodes.is_empty() || cap == 0 {
        return (String::new(), 0, 0);
    }
    let mut sorted = nodes.to_vec();
    sorted.sort_by(|a, b| a.ff_id.cmp(&b.ff_id).then_with(|| a.tag.cmp(&b.tag)));
    let mut kept_lines = Vec::new();
    let mut included = 0usize;
    for node in &sorted {
        let candidate_line = format_outline_line(node);
        let mut probe = kept_lines.join("\n");
        if !probe.is_empty() {
            probe.push('\n');
        }
        probe.push_str(&candidate_line);
        if estimate_tokens(&probe) <= cap {
            kept_lines.push(candidate_line);
            included += 1;
        }
    }
    let outline_text = kept_lines.join("\n");
    let outline_tokens = estimate_tokens(&outline_text);
    (outline_text, outline_tokens, included)
}

pub fn enforce_ssot_dom_ratio(
    ssot_dom_tokens: usize,
    non_ssot_tokens: usize,
) -> Result<f64, ContextCompilerError> {
    let denom = (ssot_dom_tokens + non_ssot_tokens).max(1) as f64;
    let ratio = (ssot_dom_tokens as f64) / denom;
    if ratio + f64::EPSILON < MIN_SSOT_DOM_RATIO {
        return Err(ContextCompilerError::SsotDomRatioBelowMinimum { ratio });
    }
    Ok((ratio * 10_000.0).round() / 10_000.0)
}
