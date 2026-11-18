fn replace_smart_quotes_html_safe(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    let mut quote_attr: Option<char> = None;
    let mut in_tag = false;
    for ch in input.chars() {
        if !in_tag && ch == '<' {
            in_tag = true;
            out.push(ch);
            continue;
        }
        if in_tag {
            if let Some(q) = quote_attr {
                if ch == q {
                    quote_attr = None;
                } else if matches!(ch, '“' | '”') && q == '"' {
                    out.push_str("&quot;");
                    continue;
                } else if matches!(ch, '‘' | '’') && q == '\'' {
                    out.push_str("&#39;");
                    continue;
                }
            } else if ch == '"' || ch == '\'' {
                quote_attr = Some(ch);
            } else if ch == '>' {
                in_tag = false;
            }
        }
        match ch {
            '“' | '”' => out.push('"'),
            '‘' | '’' => out.push('\''),
            _ => out.push(ch),
        }
    }
    out
}

fn rewrite_positive_tabindex(input: &str) -> (String, bool) {
    let mut out = String::with_capacity(input.len());
    let mut cursor = input;
    let mut changed = false;
    while let Some(pos) = cursor.to_ascii_lowercase().find("tabindex") {
        let end_key = pos + "tabindex".len();
        out.push_str(&cursor[..end_key]);
        let after_key = &cursor[end_key..];
        let trimmed_ws = after_key.trim_start();
        let ws_len = after_key.len() - trimmed_ws.len();
        if let Some(after_eq) = trimmed_ws.strip_prefix('=') {
            let after_eq_ws = after_eq.trim_start();
            let qchar = after_eq_ws.chars().next();
            if let Some(q @ ('"' | '\'')) = qchar {
                if let Some(close_idx) = after_eq_ws[1..].find(q) {
                    let inner = &after_eq_ws[1..1 + close_idx];
                    if inner.trim().parse::<i32>().is_ok_and(|n| n > 0) {
                        out.push_str(&after_key[..ws_len]);
                        out.push('=');
                        out.push(q);
                        out.push('0');
                        out.push(q);
                        cursor = &after_eq_ws[2 + close_idx..];
                        changed = true;
                        continue;
                    }
                }
            }
        }
        cursor = after_key;
    }
    out.push_str(cursor);
    (out, changed)
}

fn replace_ascii_case_insensitive(
    haystack: &str,
    needle: &str,
    replacement: &str,
) -> (String, bool) {
    let lower = haystack.to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    if !lower.contains(&needle_lower) {
        return (haystack.to_string(), false);
    }
    let mut out = String::with_capacity(haystack.len());
    let mut last = 0usize;
    for (idx, _) in lower.match_indices(&needle_lower) {
        if idx >= last {
            out.push_str(&haystack[last..idx]);
            out.push_str(replacement);
            last = idx + needle_lower.len();
        }
    }
    out.push_str(&haystack[last..]);
    (out, true)
}

/// Single-pass deterministic auto-fixer for Triangle Linter issues
pub fn auto_fix_issues(content: &str) -> (String, usize) {
    let mut fixed = content.to_string();
    let mut fix_count = 0;

    // 1. Fix curly smart quotes safely (escaping inside matching HTML attributes)
    if fixed.contains('“') || fixed.contains('”') || fixed.contains('‘') || fixed.contains('’')
    {
        fixed = replace_smart_quotes_html_safe(&fixed);
        fix_count += 1;
    }

    // 2. Fix unrequested AI purple/indigo gradients to semantic tokens (case-insensitive)
    let mut color_changed = false;
    for (hex, tok) in [
        ("#6366f1", "var(--action-primary, #2563eb)"),
        ("#8b5cf6", "var(--action-primary-hover, #1d4ed8)"),
        ("#7c3aed", "var(--action-primary, #2563eb)"),
    ] {
        let (next, changed) = replace_ascii_case_insensitive(&fixed, hex, tok);
        if changed {
            fixed = next;
            color_changed = true;
        }
    }
    if color_changed {
        fix_count += 1;
    }

    // 3. Fix positive tabindex anti-pattern (any positive integer 1..N)
    let (next_tab, tab_changed) = rewrite_positive_tabindex(&fixed);
    if tab_changed {
        fixed = next_tab;
        fix_count += 1;
    }

    // 4. Fix individual <svg> icons missing aria-hidden/aria-label/role without skipping tags when another <svg> is accessible
    if fixed.contains("<svg") {
        let mut out = String::with_capacity(fixed.len() + 32);
        let mut cursor = fixed.as_str();
        let mut changed_svg = false;
        while let Some(pos) = cursor.find("<svg") {
            out.push_str(&cursor[..pos]);
            let after_start = &cursor[pos..];
            if let Some(gt_offset) = after_start.find('>') {
                let open_tag = &after_start[..=gt_offset];
                let tag_lower = open_tag.to_ascii_lowercase();
                if !tag_lower.contains("aria-hidden")
                    && !tag_lower.contains("aria-label")
                    && !tag_lower.contains("role=\"img\"")
                {
                    out.push_str("<svg aria-hidden=\"true\"");
                    out.push_str(&open_tag[4..]);
                    changed_svg = true;
                } else {
                    out.push_str(open_tag);
                }
                cursor = &after_start[gt_offset + 1..];
            } else {
                out.push_str(after_start);
                cursor = "";
                break;
            }
        }
        out.push_str(cursor);
        if changed_svg {
            fixed = out;
            fix_count += 1;
        }
    }

    // 5. Fix line-height: 1; on paragraph / body text
    if fixed.contains("line-height: 1;") {
        fixed = fixed.replace("line-height: 1;", "line-height: 1.5;");
        fix_count += 1;
    }

    // 6. Fix viewport meta tag if head exists and meta is missing (case-insensitive)
    let fixed_lower = fixed.to_ascii_lowercase();
    if let Some(head_pos) = fixed_lower.find("<head>") {
        if !fixed_lower.contains("name=\"viewport\"") && !fixed_lower.contains("name='viewport'") {
            let mut next = String::with_capacity(fixed.len() + 80);
            next.push_str(&fixed[..head_pos + 6]);
            next.push_str(
                "\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">",
            );
            next.push_str(&fixed[head_pos + 6..]);
            fixed = next;
            fix_count += 1;
        }
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
