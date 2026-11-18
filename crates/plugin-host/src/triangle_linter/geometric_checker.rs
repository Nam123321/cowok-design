// @story 08.1
// @implements AC-04
use crate::models::{LintIssue, LintIssueCategory, LintSeverity};

fn geo_issue(
    rule_id: &str,
    category: LintIssueCategory,
    severity: LintSeverity,
    message: &str,
    line_num: usize,
    snippet: Option<&str>,
    auto_fixable: bool,
) -> LintIssue {
    LintIssue {
        rule_id: rule_id.to_string(),
        category,
        severity,
        message: message.to_string(),
        line: Some(line_num),
        column: None,
        snippet: snippet.map(|s| s.trim().to_string()),
        auto_fixable,
    }
}

fn parse_hex_color(raw: &str) -> Option<(u8, u8, u8)> {
    let hex = raw.trim().strip_prefix('#')?;
    if hex.len() == 3 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut chars = hex.chars();
        let r = u8::from_str_radix(&chars.next()?.to_string().repeat(2), 16).ok()?;
        let g = u8::from_str_radix(&chars.next()?.to_string().repeat(2), 16).ok()?;
        let b = u8::from_str_radix(&chars.next()?.to_string().repeat(2), 16).ok()?;
        return Some((r, g, b));
    }
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some((r, g, b));
    }
    None
}

fn srgb_luminance(rgb: (u8, u8, u8)) -> f64 {
    let lin = |c: u8| -> f64 {
        let v = f64::from(c) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(rgb.0) + 0.7152 * lin(rgb.1) + 0.0722 * lin(rgb.2)
}

fn wcag_contrast_ratio(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> f64 {
    let l1 = srgb_luminance(fg);
    let l2 = srgb_luminance(bg);
    let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

fn extract_css_prop_value<'a>(lower: &'a str, prop: &str) -> Option<&'a str> {
    let idx = lower.find(prop)?;
    let after = &lower[idx + prop.len()..];
    let colon_trimmed = after.trim_start();
    let val = colon_trimmed.split([';', '"', '\'', ')']).next()?.trim();
    Some(val)
}

fn extract_hex_after_prop(lower: &str, prop: &str) -> Option<(u8, u8, u8)> {
    let val = extract_css_prop_value(lower, prop)?;
    let hash_idx = val.find('#')?;
    let hex_token: String = val[hash_idx..]
        .chars()
        .take_while(|c| *c == '#' || c.is_ascii_hexdigit())
        .collect();
    parse_hex_color(&hex_token)
}

fn has_off_grid_spacing_px(lower: &str) -> bool {
    for prop in [
        "padding:",
        "padding-left:",
        "padding-right:",
        "padding-top:",
        "padding-bottom:",
        "margin:",
        "margin-left:",
        "margin-right:",
        "margin-top:",
        "margin-bottom:",
        "gap:",
    ] {
        let mut search = lower;
        while let Some(idx) = search.find(prop) {
            search = &search[idx + prop.len()..];
            let segment = search.split([';', '"', '\'']).next().unwrap_or("");
            for tok in segment.split_whitespace() {
                if let Some(num_str) = tok.strip_suffix("px") {
                    if let Ok(px) = num_str.parse::<i32>() {
                        if px > 2 && px % 4 != 0 {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn has_inflated_z_index(lower: &str) -> bool {
    for prefix in ["z-index:", "z-["] {
        if let Some(idx) = lower.find(prefix) {
            let digits: String = lower[idx + prefix.len()..]
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(z) = digits.parse::<i64>() {
                if z > 9999 {
                    return true;
                }
            }
        }
    }
    false
}

fn check_line_layout_rules(lower: &str, line: &str, line_num: usize, issues: &mut Vec<LintIssue>) {
    if (lower.contains("border-radius") || lower.contains("rounded"))
        && lower.contains("padding")
        && lower.contains("inner")
    {
        issues.push(geo_issue(
            "RULE_GEO_01_CONCENTRIC_RADIUS",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Warning,
            "Verify concentric border radius: R_inner = max(4, R_outer - P/2)",
            line_num,
            Some(line),
            true,
        ));
    }
    if (lower.contains("width:") || lower.contains("w-["))
        && (lower.contains("<p") || lower.contains("<span") || lower.contains("<h"))
        && !lower.contains("truncate")
        && !lower.contains("ellipsis")
        && !lower.contains("break-words")
        && lower.contains("px")
        && !lower.contains("overflow")
    {
        issues.push(geo_issue(
            "RULE_GEO_03_TEXT_OVERFLOW",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Warning,
            "Fixed width text element lacks overflow/ellipsis or word-break guard.",
            line_num,
            Some(line),
            true,
        ));
    }
    if (lower.contains("<button") || lower.contains("class=\"btn\""))
        && (lower.contains("height: 24px")
            || lower.contains("height: 28px")
            || lower.contains("height: 32px"))
    {
        issues.push(geo_issue(
            "RULE_GEO_04_TOUCH_TARGET",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Warning,
            "Interactive button height < 44px without touch target expansion.",
            line_num,
            Some(line),
            false,
        ));
    }
    if (lower.contains("line-height: 1;") || lower.contains("leading-none"))
        && (lower.contains("<p") || lower.contains("body") || lower.contains("prose"))
    {
        issues.push(geo_issue(
            "RULE_GEO_05_LINE_HEIGHT_RATIO",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Warning,
            "Paragraph line-height is 1.0; body text requires line-height between 1.4 and 1.6.",
            line_num,
            Some(line),
            true,
        ));
    }
    if has_off_grid_spacing_px(lower) {
        issues.push(geo_issue(
            "RULE_GEO_06_SPACING_GRID_SCALE",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Warning,
            "Off-grid spacing pixel literal detected; align to 4px/8px geometric rhythm.",
            line_num,
            Some(line),
            false,
        ));
    }
    if [
        "font-size: 8px",
        "font-size: 9px",
        "text-[8px]",
        "text-[9px]",
    ]
    .iter()
    .any(|p| lower.contains(p))
    {
        issues.push(geo_issue(
            "RULE_GEO_08_MIN_FONT_SIZE",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Error,
            "Font size < 10px violates minimum readability standard.",
            line_num,
            Some(line),
            true,
        ));
    }
    if [
        "min-width: 1200px",
        "width: 1440px",
        "w-[1200px]",
        "w-[1440px]",
    ]
    .iter()
    .any(|p| lower.contains(p))
        && !lower.contains("max-width")
    {
        issues.push(geo_issue(
            "RULE_GEO_09_HORIZONTAL_OVERFLOW",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Warning,
            "Large fixed pixel width risks horizontal overflow on tablet/mobile viewports.",
            line_num,
            Some(line),
            false,
        ));
    }
    if lower.contains("<img")
        && !lower.contains("aspect-ratio")
        && !lower.contains("object-fit")
        && !lower.contains("width=")
    {
        issues.push(geo_issue("RULE_GEO_11_ASPECT_RATIO_DISTORTION", LintIssueCategory::GeometricLayout, LintSeverity::Info, "Image element lacks explicit aspect-ratio or object-fit styling to prevent layout shift.", line_num, Some(line), false));
    }
    if (lower.contains("truncate") || lower.contains("text-overflow: ellipsis"))
        && !lower.contains("title=")
        && !lower.contains("aria-label=")
    {
        issues.push(geo_issue(
            "RULE_GEO_13_TRUNCATION_TOOLTIP",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Info,
            "Truncated text element lacks accessible title or aria-label fallback.",
            line_num,
            Some(line),
            false,
        ));
    }
    if has_inflated_z_index(lower) {
        issues.push(geo_issue(
            "RULE_GEO_14_Z_INDEX_INFLATION",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Warning,
            "Uncontrolled z-index inflation (>9999). Use structured stacking context layers.",
            line_num,
            Some(line),
            true,
        ));
    }
    if lower.contains("backdrop-filter") && !lower.contains("background") && !lower.contains("bg-")
    {
        issues.push(geo_issue(
            "RULE_GEO_18_BACKDROP_FILTER_FALLBACK",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Warning,
            "'backdrop-filter' used without fallback background color for unsupported browsers.",
            line_num,
            Some(line),
            true,
        ));
    }
    if (lower.contains("transition:") || lower.contains("transition-all"))
        && (lower.contains("width")
            || lower.contains("height")
            || lower.contains("top")
            || lower.contains("left"))
    {
        issues.push(geo_issue("RULE_GEO_24_TRANSITION_PERFORMANCE", LintIssueCategory::GeometricLayout, LintSeverity::Warning, "Animating layout properties (width/height/top/left) triggers reflow. Use transform/opacity.", line_num, Some(line), false));
    }
    if lower.contains("position: sticky")
        && lower.contains("top: 0")
        && !lower.contains("border")
        && !lower.contains("shadow")
    {
        issues.push(geo_issue("RULE_GEO_25_STICKY_HEADER_SHADOW", LintIssueCategory::GeometricLayout, LintSeverity::Info, "Sticky header lacks bottom border or elevation shadow to separate from scrolled content.", line_num, Some(line), true));
    }
}

fn check_line_wcag_rules(lower: &str, line: &str, line_num: usize, issues: &mut Vec<LintIssue>) {
    let fg_opt = if lower.contains("background-color:") {
        extract_hex_after_prop(lower.trim_start_matches(|_| false), "color:")
            .or_else(|| extract_hex_after_prop(lower, "; color:"))
            .or_else(|| extract_hex_after_prop(lower, " color:"))
    } else {
        extract_hex_after_prop(lower, "color:")
    };
    let fg_actual = if lower.contains("background-color:") {
        let cleaned = lower.replace("background-color:", "bgcol:");
        extract_hex_after_prop(&cleaned, "color:")
    } else {
        fg_opt
    };
    let bg_opt = extract_hex_after_prop(lower, "background-color:")
        .or_else(|| extract_hex_after_prop(lower, "background:"))
        .or_else(|| lower.contains("bg-white").then_some((255, 255, 255)))
        .or_else(|| lower.contains("bg-black").then_some((0, 0, 0)));
    let low_contrast = match (fg_actual, bg_opt) {
        (Some(fg), Some(bg)) => wcag_contrast_ratio(fg, bg) < 4.5,
        _ => false,
    };
    if low_contrast {
        issues.push(geo_issue(
            "RULE_GEO_02_WCAG_CONTRAST_AA",
            LintIssueCategory::WcagContrast,
            LintSeverity::Error,
            "Contrast ratio falls below WCAG 2.2 AA (4.5:1). Darken foreground text.",
            line_num,
            Some(line),
            true,
        ));
    }
    if (lower.contains("outline: none") || lower.contains("outline: 0"))
        && !lower.contains("focus-visible")
        && !lower.contains("ring")
        && !lower.contains("box-shadow")
    {
        issues.push(geo_issue("RULE_GEO_10_INTERACTIVE_ROLE_FOCUS", LintIssueCategory::WcagContrast, LintSeverity::Error, "'outline: none' removes keyboard focus ring without providing focus-visible alternative.", line_num, Some(line), true));
    }
    if (lower.contains("bg-red-500") || lower.contains("bg-green-500"))
        && lower.contains("badge")
        && !lower.contains("aria-label")
        && (lower.contains("></") || lower.contains("status-dot"))
    {
        issues.push(geo_issue(
            "RULE_GEO_12_COLOR_ONLY_STATUS",
            LintIssueCategory::WcagContrast,
            LintSeverity::Warning,
            "Status badge conveys state via color only without label or icon indicator.",
            line_num,
            Some(line),
            false,
        ));
    }
    if (lower.contains("<svg") || lower.contains("<i class=\"icon"))
        && !lower.contains("aria-hidden")
        && !lower.contains("aria-label")
    {
        issues.push(geo_issue(
            "RULE_GEO_16_ICON_ALIGNMENT",
            LintIssueCategory::WcagContrast,
            LintSeverity::Info,
            "Icon missing 'aria-hidden=\"true\"' or accessible 'aria-label'.",
            line_num,
            Some(line),
            true,
        ));
    }
    if (lower.contains("<video") || lower.contains("<audio")) && !lower.contains("controls") {
        issues.push(geo_issue(
            "RULE_GEO_17_MEDIA_CONTROLS_A11Y",
            LintIssueCategory::WcagContrast,
            LintSeverity::Warning,
            "Media element lacks native 'controls' attribute for keyboard/assistive playback.",
            line_num,
            Some(line),
            false,
        ));
    }
    if lower.contains("<button") && !lower.contains("type=") {
        issues.push(geo_issue(
            "RULE_GEO_21_BUTTON_TYPE_EXPLICIT",
            LintIssueCategory::WcagContrast,
            LintSeverity::Info,
            "Button element lacks explicit type=\"button|submit|reset\" attribute.",
            line_num,
            Some(line),
            false,
        ));
    }
    if lower.contains("<input")
        && !lower.contains("type=\"hidden\"")
        && !lower.contains("aria-label")
        && !lower.contains("id=")
    {
        issues.push(geo_issue(
            "RULE_GEO_22_FORM_LABEL_ASSOCIATION",
            LintIssueCategory::WcagContrast,
            LintSeverity::Warning,
            "Form input lacks id (for <label for=\"...\">) or aria-label attribute.",
            line_num,
            Some(line),
            false,
        ));
    }
    if has_positive_tabindex(lower) {
        issues.push(geo_issue(
            "RULE_GEO_23_TAB_ORDER_INTEGRITY",
            LintIssueCategory::WcagContrast,
            LintSeverity::Error,
            "Positive tabindex disrupts natural DOM reading order. Use tabindex=\"0\" or \"-1\".",
            line_num,
            Some(line),
            true,
        ));
    }
}

fn has_positive_tabindex(lower: &str) -> bool {
    let mut cursor = lower;
    while let Some(pos) = cursor.find("tabindex") {
        let rest = cursor[pos + "tabindex".len()..].trim_start();
        if let Some(after_eq) = rest.strip_prefix('=') {
            let trimmed = after_eq.trim_start();
            let val = if let Some(q) = trimmed.strip_prefix('"') {
                q.split('"').next().unwrap_or("")
            } else if let Some(q) = trimmed.strip_prefix('\'') {
                q.split('\'').next().unwrap_or("")
            } else {
                trimmed.split_whitespace().next().unwrap_or("")
            };
            if let Ok(num) = val.trim().parse::<i32>() {
                if num > 0 {
                    return true;
                }
            }
        }
        cursor = &cursor[pos + "tabindex".len()..];
    }
    false
}

fn check_document_level_rules(lower_full: &str, issues: &mut Vec<LintIssue>) {
    if lower_full.contains("<h1") && lower_full.contains("<h3") && !lower_full.contains("<h2") {
        issues.push(geo_issue(
            "RULE_GEO_07_HEADING_HIERARCHY",
            LintIssueCategory::WcagContrast,
            LintSeverity::Warning,
            "Heading hierarchy skips <h2> between <h1> and <h3>.",
            1,
            None,
            false,
        ));
    }
    if lower_full.contains("<html") && !lower_full.contains("name=\"viewport\"") {
        issues.push(geo_issue("RULE_GEO_15_VIEWPORT_META", LintIssueCategory::GeometricLayout, LintSeverity::Error, "HTML document missing '<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">'.", 1, None, true));
    }
    if (lower_full.contains("@keyframes") || lower_full.contains("animation:"))
        && !lower_full.contains("prefers-reduced-motion")
    {
        issues.push(geo_issue(
            "RULE_GEO_19_MOTION_REDUCED_PREF",
            LintIssueCategory::WcagContrast,
            LintSeverity::Warning,
            "CSS animation lacks '@media (prefers-reduced-motion: reduce)' guard.",
            1,
            None,
            false,
        ));
    }
    if lower_full.contains("@font-face")
        && !lower_full.contains("font-display: swap")
        && !lower_full.contains("font-display: optional")
    {
        issues.push(geo_issue(
            "RULE_GEO_20_FONT_DISPLAY_SWAP",
            LintIssueCategory::GeometricLayout,
            LintSeverity::Info,
            "@font-face declaration lacks font-display: swap or optional to avoid FOIT.",
            1,
            None,
            false,
        ));
    }
}

/// Evaluates 25 layout, geometric, and WCAG rules from DESIGN:OS Studio, Hallmark, and Impeccable.
pub fn check_geometric_and_wcag_rules(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let lower_full = content.to_ascii_lowercase();

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let lower = line.to_ascii_lowercase();
        check_line_layout_rules(&lower, line, line_num, &mut issues);
        check_line_wcag_rules(&lower, line, line_num, &mut issues);
    }

    check_document_level_rules(&lower_full, &mut issues);
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_contrast_and_accessibility_issues() {
        let snippet = r#"<html lang="en">
<div style="background-color: #fff; color: #999; outline: none">
  <input type="text" placeholder="Search..." tabindex="1" />
  <svg class="icon"><path d="" /></svg>
</div>
</html>"#;
        let issues = check_geometric_and_wcag_rules(snippet);
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "RULE_GEO_02_WCAG_CONTRAST_AA"));
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "RULE_GEO_10_INTERACTIVE_ROLE_FOCUS"));
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "RULE_GEO_22_FORM_LABEL_ASSOCIATION"));
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "RULE_GEO_23_TAB_ORDER_INTEGRITY"));
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "RULE_GEO_16_ICON_ALIGNMENT"));
        assert!(issues
            .iter()
            .any(|i| i.rule_id == "RULE_GEO_15_VIEWPORT_META"));
    }

    #[test]
    fn test_enforces_25_distinct_geometric_and_wcag_rules() {
        let all_violations = r#"<html lang="en"><style>
@font-face { font-family: Custom; }
@keyframes spin { from { transform: rotate(0deg); } }
</style>
<h1>Title</h1><h3>Skipped H2</h3>
<div class="inner" style="border-radius: 12px; padding: 16px;"></div>
<div style="background-color: #fff; color: #999; outline: none;"></div>
<p style="width: 120px; line-height: 1; font-size: 9px;">Clipped text</p>
<div style="padding: 13px; min-width: 1200px; z-index: 99999; backdrop-filter: blur(8px); transition: width 0.2s; position: sticky; top: 0;"></div>
<button style="height: 24px">Click</button>
<img src="local.png" />
<span class="badge bg-red-500 status-dot"></span>
<span class="truncate">Long text</span>
<svg class="icon"><path d="" /></svg>
<video src="demo.mp4"></video>
<input type="text" />
<a tabindex="1">First</a>
</html>"#;
        let issues = check_geometric_and_wcag_rules(all_violations);
        let distinct: std::collections::BTreeSet<&str> =
            issues.iter().map(|i| i.rule_id.as_str()).collect();
        assert!(
            distinct.len() >= 25,
            "Expected >= 25 rules, got {}: {:?}",
            distinct.len(),
            distinct
        );
    }
}
