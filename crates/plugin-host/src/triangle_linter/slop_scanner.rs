// @story 08.1
// @implements AC-04, AC-08
use crate::models::{LintIssue, LintIssueCategory, LintSeverity};

fn make_issue(
    rule_id: &str,
    category: LintIssueCategory,
    severity: LintSeverity,
    message: &str,
    line_num: usize,
    snippet: &str,
    auto_fixable: bool,
) -> LintIssue {
    LintIssue {
        rule_id: rule_id.to_string(),
        category,
        severity,
        message: message.to_string(),
        line: Some(line_num),
        column: None,
        snippet: Some(snippet.trim().to_string()),
        auto_fixable,
    }
}

fn line_number_at(content: &str, byte_idx: usize) -> usize {
    1 + content[..byte_idx].bytes().filter(|&b| b == b'\n').count()
}

fn scan_multiline_external_beacons(content: &str, lower: &str, issues: &mut Vec<LintIssue>) {
    // Detect Markdown external image beacons including multi-line `![alt](\nhttps://...)`
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("![") {
        let start = from + rel;
        let rest = &lower[start + 2..];
        if let Some(close_bracket) = rest.find("](") {
            let after = &rest[close_bracket + 2..];
            if let Some(close_paren) = after.find(')') {
                let target = after[..close_paren].trim();
                if target.starts_with("http://") || target.starts_with("https://") {
                    let snippet = content[start..start + 2 + close_bracket + 2 + close_paren + 1].replace('\n', " ");
                    issues.push(make_issue(
                        "ANTI_SLOP_07_EXTERNAL_BEACON",
                        LintIssueCategory::SyntaxIntegrity,
                        LintSeverity::Error,
                        "Detected external image URL beacon in Markdown element.",
                        line_number_at(content, start),
                        &snippet,
                        false,
                    ));
                }
            }
        }
        from = start + 2;
    }

    // Detect multi-line HTML `<img\n src="https://...">` tags
    from = 0;
    while let Some(rel) = lower[from..].find("<img") {
        let start = from + rel;
        let next_char = lower.as_bytes().get(start + 4).copied();
        if next_char.is_none_or(|b| b.is_ascii_whitespace() || b == b'>' || b == b'/') {
            if let Some(tag_end) = lower[start..].find('>') {
                let tag_lower = &lower[start..start + tag_end + 1];
                let compact: String = tag_lower.chars().filter(|c| !c.is_ascii_whitespace()).collect();
                if compact.contains("src=\"http://")
                    || compact.contains("src=\"https://")
                    || compact.contains("src='http://")
                    || compact.contains("src='https://")
                {
                    let snippet = content[start..start + tag_end + 1].replace('\n', " ");
                    issues.push(make_issue(
                        "ANTI_SLOP_07_EXTERNAL_BEACON",
                        LintIssueCategory::SyntaxIntegrity,
                        LintSeverity::Error,
                        "Detected external image URL beacon in <img> element.",
                        line_number_at(content, start),
                        &snippet,
                        false,
                    ));
                }
            }
        }
        from = start + 4;
    }
}

/// Scans source code / markup for anti-AI slop tells, syntax integrity issues, and XSS/beacon hygiene.
pub fn scan_slop_and_tells(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    // Note: use ASCII-only case folding so byte offsets remain 1:1 identical with `content` UTF-8 boundaries.
    let lower_full = content.to_ascii_lowercase();
    scan_multiline_external_beacons(content, &lower_full, &mut issues);

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;

        if line.contains('“') || line.contains('”') || line.contains('‘') || line.contains('’') {
            issues.push(make_issue(
                "ANTI_SLOP_01_SMART_QUOTES",
                LintIssueCategory::SyntaxIntegrity,
                LintSeverity::Error,
                "Contains curly smart quotes instead of standard ASCII quotes",
                line_num,
                line,
                true,
            ));
        }

        let lower = line.to_lowercase();
        if lower.contains("#6366f1")
            || lower.contains("#8b5cf6")
            || lower.contains("#7c3aed")
            || lower.contains("rgb(99, 102, 241)")
            || lower.contains("rgb(139, 92, 246)")
            || lower.contains("from-purple-500")
            || lower.contains("from-indigo-600")
        {
            issues.push(make_issue(
                "ANTI_SLOP_02_PURPLE_INDIGO_GRADIENT",
                LintIssueCategory::AntiAiSlop,
                LintSeverity::Warning,
                "Detected unrequested default AI purple/indigo gradient tell. Use semantic tokens instead.",
                line_num,
                line,
                true,
            ));
        }

        let has_emoji = ['✨', '🚀', '💡', '🎉', '🔥', '⚡', '🤖', '🪄']
            .iter()
            .any(|&e| line.contains(e));
        if has_emoji
            && (lower.contains("<button")
                || lower.contains("btn")
                || lower.contains("badge")
                || lower.contains("<badge"))
        {
            issues.push(make_issue(
                "ANTI_SLOP_03_EMOJI_ICON_SUBSTITUTE",
                LintIssueCategory::AntiAiSlop,
                LintSeverity::Warning,
                "Detected emoji used as icon in button/badge. Use SVGs from Lucide/Phosphor or icon registry.",
                line_num,
                line,
                false,
            ));
        }

        if lower.contains("lorem ipsum") || lower.contains("dolor sit amet") {
            issues.push(make_issue(
                "ANTI_SLOP_04_LOREM_IPSUM_PLACEHOLDER",
                LintIssueCategory::AntiAiSlop,
                LintSeverity::Warning,
                "Detected lorem ipsum placeholder text. Use realistic domain sample data.",
                line_num,
                line,
                false,
            ));
        }

        let has_fabricated_metric = lower.contains("10x faster")
            || lower.contains("100x faster")
            || lower.contains("99.9% uptime")
            || lower.contains("trusted by 10,000+")
            || lower.contains("thousands of satisfied");
        if has_fabricated_metric
            && !lower.contains("data-source")
            && !lower.contains("citation")
            && !lower.contains("reference")
        {
            issues.push(make_issue(
                "ANTI_SLOP_05_FABRICATED_METRIC_CLAIM",
                LintIssueCategory::AntiAiSlop,
                LintSeverity::Warning,
                "Detected ungrounded metric claim without citation or data-source attribute.",
                line_num,
                line,
                false,
            ));
        }

        if lower.contains("<script")
            || lower.contains(concat!("dangerously", "setinnerhtml"))
            || lower.contains("javascript:")
        {
            issues.push(make_issue(
                "ANTI_SLOP_06_XSS_SCRIPT_HYGIENE",
                LintIssueCategory::SyntaxIntegrity,
                LintSeverity::Error,
                concat!(
                    "Detected unsafe inline <script>, dangerously",
                    "SetInnerHTML, or javascript: URI.",
                ),
                line_num,
                line,
                false,
            ));
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_smart_quotes_and_slop_gradient() {
        let snippet = r#"<div style="background: linear-gradient(#6366f1, #8b5cf6)">
  <button class="btn">✨ Get Started “Now”</button>
  <p>10x faster build times with lorem ipsum text</p>
</div>"#;
        let issues = scan_slop_and_tells(snippet);
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "ANTI_SLOP_01_SMART_QUOTES"));
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "ANTI_SLOP_02_PURPLE_INDIGO_GRADIENT"));
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "ANTI_SLOP_03_EMOJI_ICON_SUBSTITUTE"));
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "ANTI_SLOP_04_LOREM_IPSUM_PLACEHOLDER"));
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "ANTI_SLOP_05_FABRICATED_METRIC_CLAIM"));
    }
}
