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
        }
    }

    /// Lints and performs single-pass auto-fix, re-auditing to report remaining issues
    pub fn lint_and_fix(content: &str) -> (String, TriangleLintReport) {
        let start = Instant::now();
        let _initial_issues = Self::lint(content);

        let (fixed_content, fix_count) = auto_fix_issues(content);

        // Re-audit fixed content
        let recheck = Self::lint(&fixed_content);
        let duration_ms = start.elapsed().as_millis() as u64;

        let report = TriangleLintReport {
            total_issues: recheck.total_issues,
            new_in_this_call: recheck.total_issues,
            fixed_issues: fix_count,
            passed: recheck.passed,
            duration_ms,
            issues: recheck.issues,
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
    }
}
