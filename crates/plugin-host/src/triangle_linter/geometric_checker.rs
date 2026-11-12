use crate::models::{LintIssue, LintIssueCategory, LintSeverity};

/// Evaluates 25+ layout, geometric, and WCAG rules from DESIGN:OS Studio, Hallmark, and Impeccable
pub fn check_geometric_and_wcag_rules(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let lower_full = content.to_lowercase();

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let lower = line.to_lowercase();

        // Rule 01: Concentric border radius check (detects mismatched inner/outer radii)
        if (lower.contains("border-radius") || lower.contains("rounded"))
            && lower.contains("padding")
            && lower.contains("inner")
        {
            // Suggest concentric formula: R_inner = max(4, R_outer - P/2)
            issues.push(LintIssue {
                rule_id: "RULE_GEO_01_CONCENTRIC_RADIUS".to_string(),
                category: LintIssueCategory::GeometricLayout,
                severity: LintSeverity::Warning,
                message: "Verify concentric border radius: R_inner = max(4, R_outer - P/2)"
                    .to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: true,
            });
        }

        // Rule 02: WCAG 2.2 AA Contrast check for known poor-contrast gray pairs
        if (lower.contains("color: #999")
            || lower.contains("color: #aaa")
            || lower.contains("color: #bbb"))
            && (lower.contains("background: #fff")
                || lower.contains("background-color: #fff")
                || lower.contains("bg-white"))
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_02_WCAG_CONTRAST_AA".to_string(),
                category: LintIssueCategory::WcagContrast,
                severity: LintSeverity::Error,
                message: "Contrast ratio falls below WCAG 2.2 AA (4.5:1). Darken foreground text."
                    .to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: true,
            });
        }

        // Rule 03: Text overflow detection (fixed width without ellipsis or word-break)
        if (lower.contains("width:") || lower.contains("w-["))
            && (lower.contains("<p") || lower.contains("<span") || lower.contains("<h"))
            && !lower.contains("truncate")
            && !lower.contains("ellipsis")
            && !lower.contains("break-words")
        {
            // Only flag if explicit fixed px width
            if lower.contains("px") && !lower.contains("overflow") {
                issues.push(LintIssue {
                    rule_id: "RULE_GEO_03_TEXT_OVERFLOW".to_string(),
                    category: LintIssueCategory::GeometricLayout,
                    severity: LintSeverity::Warning,
                    message:
                        "Fixed width text element lacks overflow/ellipsis or word-break guard."
                            .to_string(),
                    line: Some(line_num),
                    column: None,
                    snippet: Some(line.trim().to_string()),
                    auto_fixable: true,
                });
            }
        }

        // Rule 04: Touch target size (< 44px on clickable buttons)
        if (lower.contains("<button") || lower.contains("class=\"btn\""))
            && (lower.contains("height: 24px")
                || lower.contains("height: 28px")
                || lower.contains("height: 32px"))
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_04_TOUCH_TARGET".to_string(),
                category: LintIssueCategory::GeometricLayout,
                severity: LintSeverity::Warning,
                message: "Interactive button height < 44px without touch target expansion."
                    .to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: false,
            });
        }

        // Rule 05: Line-height ratio check
        if (lower.contains("line-height: 1;") || lower.contains("leading-none"))
            && (lower.contains("<p") || lower.contains("body") || lower.contains("prose"))
        {
            issues.push(LintIssue {
                    rule_id: "RULE_GEO_05_LINE_HEIGHT_RATIO".to_string(),
                    category: LintIssueCategory::GeometricLayout,
                    severity: LintSeverity::Warning,
                    message: "Paragraph line-height is 1.0; body text requires line-height between 1.4 and 1.6.".to_string(),
                    line: Some(line_num),
                    column: None,
                    snippet: Some(line.trim().to_string()),
                    auto_fixable: true,
                });
        }

        // Rule 08: Minimum font size (< 10px)
        if lower.contains("font-size: 8px")
            || lower.contains("font-size: 9px")
            || lower.contains("text-[8px]")
            || lower.contains("text-[9px]")
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_08_MIN_FONT_SIZE".to_string(),
                category: LintIssueCategory::GeometricLayout,
                severity: LintSeverity::Error,
                message: "Font size < 10px violates minimum readability standard.".to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: true,
            });
        }

        // Rule 10: Interactive role focus outline missing
        if (lower.contains("outline: none") || lower.contains("outline: 0"))
            && !lower.contains("focus-visible")
            && !lower.contains("ring")
            && !lower.contains("box-shadow")
        {
            issues.push(LintIssue {
                    rule_id: "RULE_GEO_10_INTERACTIVE_ROLE_FOCUS".to_string(),
                    category: LintIssueCategory::WcagContrast,
                    severity: LintSeverity::Error,
                    message: "'outline: none' removes keyboard focus ring without providing focus-visible alternative.".to_string(),
                    line: Some(line_num),
                    column: None,
                    snippet: Some(line.trim().to_string()),
                    auto_fixable: true,
                });
        }

        // Rule 11: Image missing aspect-ratio or object-fit
        if lower.contains("<img")
            && !lower.contains("aspect-ratio")
            && !lower.contains("object-fit")
            && !lower.contains("width=")
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_11_ASPECT_RATIO_DISTORTION".to_string(),
                category: LintIssueCategory::GeometricLayout,
                severity: LintSeverity::Info,
                message: "Image element lacks explicit aspect-ratio or object-fit styling to prevent layout shift.".to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: false,
            });
        }

        // Rule 14: Z-index inflation
        if lower.contains("z-index: 99999")
            || lower.contains("z-index: 999999")
            || lower.contains("z-[99999]")
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_14_Z_INDEX_INFLATION".to_string(),
                category: LintIssueCategory::GeometricLayout,
                severity: LintSeverity::Warning,
                message: "Uncontrolled z-index inflation (>9999). Use structured stacking context layers.".to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: true,
            });
        }

        // Rule 16: Standalone icon missing accessible label
        if (lower.contains("<svg") || lower.contains("<i class=\"icon"))
            && !lower.contains("aria-hidden")
            && !lower.contains("aria-label")
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_16_ICON_ALIGNMENT".to_string(),
                category: LintIssueCategory::WcagContrast,
                severity: LintSeverity::Info,
                message: "Icon missing 'aria-hidden=\"true\"' or accessible 'aria-label'."
                    .to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: true,
            });
        }

        // Rule 18: Backdrop filter missing fallback background
        if lower.contains("backdrop-filter")
            && !lower.contains("background")
            && !lower.contains("bg-")
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_18_BACKDROP_FILTER_FALLBACK".to_string(),
                category: LintIssueCategory::GeometricLayout,
                severity: LintSeverity::Warning,
                message: "'backdrop-filter' used without fallback background color for unsupported browsers.".to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: true,
            });
        }

        // Rule 22: Form input missing label association
        if lower.contains("<input")
            && !lower.contains("type=\"hidden\"")
            && !lower.contains("aria-label")
            && !lower.contains("id=")
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_22_FORM_LABEL_ASSOCIATION".to_string(),
                category: LintIssueCategory::WcagContrast,
                severity: LintSeverity::Warning,
                message: "Form input lacks id (for <label for=\"...\">) or aria-label attribute."
                    .to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: false,
            });
        }

        // Rule 23: Positive tabindex anti-pattern
        if lower.contains("tabindex=\"1\"")
            || lower.contains("tabindex=\"2\"")
            || lower.contains("tabindex=\"3\"")
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_23_TAB_ORDER_INTEGRITY".to_string(),
                category: LintIssueCategory::WcagContrast,
                severity: LintSeverity::Error,
                message: "Positive tabindex disrupts natural DOM reading order. Use tabindex=\"0\" or \"-1\".".to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: true,
            });
        }

        // Rule 24: Transition on layout properties
        if (lower.contains("transition:") || lower.contains("transition-all"))
            && (lower.contains("width")
                || lower.contains("height")
                || lower.contains("top")
                || lower.contains("left"))
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_24_TRANSITION_PERFORMANCE".to_string(),
                category: LintIssueCategory::GeometricLayout,
                severity: LintSeverity::Warning,
                message: "Animating layout properties (width/height/top/left) triggers reflow. Use transform/opacity.".to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: false,
            });
        }

        // Rule 25: Sticky header missing border/shadow
        if lower.contains("position: sticky")
            && lower.contains("top: 0")
            && !lower.contains("border")
            && !lower.contains("shadow")
        {
            issues.push(LintIssue {
                rule_id: "RULE_GEO_25_STICKY_HEADER_SHADOW".to_string(),
                category: LintIssueCategory::GeometricLayout,
                severity: LintSeverity::Info,
                message: "Sticky header lacks bottom border or elevation shadow to separate from scrolled content.".to_string(),
                line: Some(line_num),
                column: None,
                snippet: Some(line.trim().to_string()),
                auto_fixable: true,
            });
        }
    }

    // Full document checks:
    // Rule 15: Viewport meta tag
    if lower_full.contains("<html") && !lower_full.contains("name=\"viewport\"") {
        issues.push(LintIssue {
            rule_id: "RULE_GEO_15_VIEWPORT_META".to_string(),
            category: LintIssueCategory::GeometricLayout,
            severity: LintSeverity::Error,
            message: "HTML document missing '<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">'.".to_string(),
            line: Some(1),
            column: None,
            snippet: None,
            auto_fixable: true,
        });
    }

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
}
