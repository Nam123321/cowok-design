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
    let mut outline_text = String::new();
    let mut outline_tokens = 0usize;
    let mut included = 0usize;
    for node in &sorted {
        if outline_tokens >= cap {
            break;
        }
        let candidate_line = format_outline_line(node);
        let line_tokens = estimate_tokens(&candidate_line);
        if outline_tokens + line_tokens > cap + 1 {
            continue;
        }
        let prev_len = outline_text.len();
        if prev_len > 0 {
            outline_text.push('\n');
        }
        outline_text.push_str(&candidate_line);
        let next_tokens = estimate_tokens(&outline_text);
        if next_tokens <= cap {
            outline_tokens = next_tokens;
            included += 1;
        } else {
            outline_text.truncate(prev_len);
        }
    }
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
