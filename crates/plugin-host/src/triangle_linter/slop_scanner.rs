use crate::models::{LintIssue, LintIssueCategory, LintSeverity};

/// Scans source code / markup for anti-AI slop tells and syntax integrity issues
pub fn scan_slop_and_tells(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;

        // Check for smart quotes (breaks JSON and strings)
        if line.contains('“') || line.contains('”') || line.contains('‘') || line.contains('’')
        {
            issues.push(LintIssue {
                rule_id: "ANTI_SLOP_01_SMART_QUOTES".to_string(),
                category: LintIssueCategory::SyntaxIntegrity,
                severity: LintSeverity::Error,
                message: "Contains curly smart quotes instead of standard ASCII quotes".to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: true,
            });
        }

        // Check for default AI purple / indigo slop gradients
        let lower = line.to_lowercase();
        if lower.contains("#6366f1")
            || lower.contains("#8b5cf6")
            || lower.contains("#7c3aed")
            || lower.contains("rgb(99, 102, 241)")
            || lower.contains("rgb(139, 92, 246)")
            || lower.contains("from-purple-500")
            || lower.contains("from-indigo-600")
        {
            issues.push(LintIssue {
                rule_id: "ANTI_SLOP_02_PURPLE_INDIGO_GRADIENT".to_string(),
                category: LintIssueCategory::AntiAiSlop,
                severity: LintSeverity::Warning,
                message: "Detected unrequested default AI purple/indigo gradient tell. Use semantic tokens instead.".to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: true,
            });
        }

        // Check for emoji used as icons in buttons or badges
        let has_emoji = ['✨', '🚀', '💡', '🎉', '🔥', '⚡', '🤖', '🪄']
            .iter()
            .any(|&e| line.contains(e));
        if has_emoji
            && (lower.contains("<button")
                || lower.contains("btn")
                || lower.contains("badge")
                || lower.contains("<badge"))
        {
            issues.push(LintIssue {
                rule_id: "ANTI_SLOP_03_EMOJI_ICON_SUBSTITUTE".to_string(),
                category: LintIssueCategory::AntiAiSlop,
                severity: LintSeverity::Warning,
                message: "Detected emoji used as icon in button/badge. Use SVGs from Lucide/Phosphor or icon registry.".to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: false,
            });
        }

        // Check for lorem ipsum
        if lower.contains("lorem ipsum") || lower.contains("dolor sit amet") {
            issues.push(LintIssue {
                rule_id: "ANTI_SLOP_04_LOREM_IPSUM_PLACEHOLDER".to_string(),
                category: LintIssueCategory::AntiAiSlop,
                severity: LintSeverity::Warning,
                message: "Detected lorem ipsum placeholder text. Use realistic domain sample data."
                    .to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: false,
            });
        }

        // Check for unverified empirical metric claims
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
            issues.push(LintIssue {
                rule_id: "ANTI_SLOP_05_FABRICATED_METRIC_CLAIM".to_string(),
                category: LintIssueCategory::AntiAiSlop,
                severity: LintSeverity::Warning,
                message:
                    "Detected ungrounded metric claim without citation or data-source attribute."
                        .to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: false,
            });
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
