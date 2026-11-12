/// Single-pass deterministic auto-fixer for Triangle Linter issues
pub fn auto_fix_issues(content: &str) -> (String, usize) {
    let mut fixed = content.to_string();
    let mut fix_count = 0;

    // 1. Fix curly smart quotes
    if fixed.contains('“') || fixed.contains('”') || fixed.contains('‘') || fixed.contains('’')
    {
        fixed = fixed
            .replace('“', "\"")
            .replace('”', "\"")
            .replace('‘', "'")
            .replace('’', "'");
        fix_count += 1;
    }

    // 2. Fix unrequested AI purple/indigo gradients to semantic tokens
    if fixed.contains("#6366f1") || fixed.contains("#8b5cf6") || fixed.contains("#7c3aed") {
        fixed = fixed
            .replace("#6366f1", "var(--action-primary, #2563eb)")
            .replace("#8b5cf6", "var(--action-primary-hover, #1d4ed8)")
            .replace("#7c3aed", "var(--action-primary, #2563eb)");
        fix_count += 1;
    }

    // 3. Fix positive tabindex anti-pattern
    if fixed.contains("tabindex=\"1\"")
        || fixed.contains("tabindex=\"2\"")
        || fixed.contains("tabindex=\"3\"")
    {
        fixed = fixed
            .replace("tabindex=\"1\"", "tabindex=\"0\"")
            .replace("tabindex=\"2\"", "tabindex=\"0\"")
            .replace("tabindex=\"3\"", "tabindex=\"0\"");
        fix_count += 1;
    }

    // 4. Fix positive SVG icons missing aria-hidden
    if fixed.contains("<svg") && !fixed.contains("aria-hidden") && !fixed.contains("aria-label") {
        fixed = fixed.replace("<svg", "<svg aria-hidden=\"true\"");
        fix_count += 1;
    }

    // 5. Fix line-height: 1; on paragraph / body text
    if fixed.contains("line-height: 1;") {
        fixed = fixed.replace("line-height: 1;", "line-height: 1.5;");
        fix_count += 1;
    }

    // 6. Fix viewport meta tag if head exists and meta is missing
    if fixed.contains("<head>") && !fixed.contains("name=\"viewport\"") {
        fixed = fixed.replace(
            "<head>",
            "<head>\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">",
        );
        fix_count += 1;
    }

    (fixed, fix_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_fixes_smart_quotes_and_slop_gradient() {
        let input = r#"<head></head><svg class="icon"><path/></svg><div style="background: #6366f1">“Hello”</div><input tabindex="1"/>"#;
        let (output, count) = auto_fix_issues(input);
        assert!(count >= 4);
        assert!(output.contains("\"Hello\""));
        assert!(!output.contains('“'));
        assert!(output.contains("var(--action-primary"));
        assert!(output.contains("tabindex=\"0\""));
        assert!(output.contains("aria-hidden=\"true\""));
        assert!(output.contains("name=\"viewport\""));
    }
}
