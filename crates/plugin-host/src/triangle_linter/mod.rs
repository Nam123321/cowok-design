pub mod auto_fixer;
pub mod geometric_checker;
pub mod slop_scanner;

use crate::models::TriangleLintReport;
use std::time::Instant;

pub use auto_fixer::auto_fix_issues;
pub use geometric_checker::check_geometric_and_wcag_rules;
pub use slop_scanner::scan_slop_and_tells;

/// Master Triangle Linter combining Anti-AI-Slop, 25+ Geometric/WCAG rules, and Auto-Fix
pub struct TriangleLinter;

impl TriangleLinter {
    /// Runs deterministic linting over code/markup in < 25ms with 0 LLM tokens
    pub fn lint(content: &str) -> TriangleLintReport {
        let start = Instant::now();
        let mut issues = Vec::new();

        // 1. Anti-AI-Slop and syntax scan
        issues.extend(scan_slop_and_tells(content));

        // 2. Geometric and WCAG 2.2 AA layout checks
        issues.extend(check_geometric_and_wcag_rules(content));

        let duration_ms = start.elapsed().as_millis() as u64;
        let total_issues = issues.len();
        let passed = issues.is_empty();

        TriangleLintReport {
            total_issues,
            new_in_this_call: total_issues,
            fixed_issues: 0,
            passed,
            duration_ms,
            issues,
            fixed_content: None,
        }
    }

    /// Lints and performs single-pass auto-fix, re-auditing to report remaining and newly introduced issues
    pub fn lint_and_fix(content: &str) -> (String, TriangleLintReport) {
        let start = Instant::now();
        let initial_issues = Self::lint(content);

        let (fixed_content, fix_count) = auto_fix_issues(content);

        // Re-audit fixed content
        let recheck = Self::lint(&fixed_content);
        let duration_ms = start.elapsed().as_millis() as u64;
        let mut prior_counts: std::collections::HashMap<(&str, &str), usize> =
            std::collections::HashMap::new();
        for issue in &initial_issues.issues {
            *prior_counts
                .entry((issue.rule_id.as_str(), issue.message.as_str()))
                .or_insert(0) += 1;
        }
        let mut newly_introduced = 0usize;
        for issue in &recheck.issues {
            let key = (issue.rule_id.as_str(), issue.message.as_str());
            if let Some(count) = prior_counts.get_mut(&key) {
                if *count > 0 {
                    *count -= 1;
                    continue;
                }
            }
            newly_introduced += 1;
        }

        let report = TriangleLintReport {
            total_issues: recheck.total_issues,
            new_in_this_call: newly_introduced,
            fixed_issues: fix_count,
            passed: recheck.passed,
            duration_ms,
            issues: recheck.issues,
            fixed_content: Some(fixed_content.clone()),
        };

        (fixed_content, report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_speed_under_25ms() {
        let sample = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Clean Sample</title>
</head>
<body style="line-height: 1.5; color: #111; background: #fff">
  <main>
    <h1>Welcome to Cowok Design</h1>
    <p>Professional design without AI slop.</p>
  </main>
</body>
</html>"#;
        let report = TriangleLinter::lint(sample);
        assert!(
            report.duration_ms < 25,
            "Linter took {}ms, expected < 25ms",
            report.duration_ms
        );
        assert!(report.passed);
        assert_eq!(report.total_issues, 0);
    }

    #[test]
    fn test_lint_and_fix_flow() {
        let buggy = r#"<head></head><div style="background: #6366f1">“Quotes”</div>"#;
        let (fixed, report) = TriangleLinter::lint_and_fix(buggy);
        assert!(report.fixed_issues >= 2);
        assert!(!fixed.contains('“'));
        assert!(fixed.contains("var(--action-primary"));
        assert_eq!(report.fixed_content.as_deref(), Some(fixed.as_str()));
        assert_eq!(
            report.new_in_this_call, 0,
            "pre-existing unfixed issues must not count as new_in_this_call"
        );
    }

    #[test]
    fn test_positive_tabindex_and_attr_quotes_fixed_safely() {
        let html = r#"<button tabindex="5" title="Say “hi”">Go</button>"#;
        let (fixed, _report) = TriangleLinter::lint_and_fix(html);
        assert!(fixed.contains(r#"tabindex="0""#));
        assert!(fixed.contains(r#"title="Say &quot;hi&quot;""#));
    }

    #[test]
    fn test_lint_and_fix_ignores_head_line_shift_and_same_line_snippet_edits() {
        let html = "<head>\n<title>Test</title>\n</head>\n<div style=\"color:#6366f1; padding: 3px\">Hello</div>";
        let (fixed, report) = TriangleLinter::lint_and_fix(html);
        assert!(report.fixed_issues >= 2);
        assert_eq!(report.fixed_content.as_deref(), Some(fixed.as_str()));
        assert_eq!(
            report.new_in_this_call, 0,
            "head viewport insertion line shift and same-line snippet edits must not inflate new_in_this_call"
        );
    }
}
