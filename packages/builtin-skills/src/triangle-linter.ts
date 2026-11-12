import { LintIssue, TriangleLintReport } from "./types";

export function scanSlopAndTells(content: string): LintIssue[] {
  const issues: LintIssue[] = [];
  const lines = content.split("\n");

  lines.forEach((line, idx) => {
    const lineNum = idx + 1;
    const lower = line.toLowerCase();

    // Check for smart quotes
    if (line.includes("“") || line.includes("”") || line.includes("‘") || line.includes("’")) {
      issues.push({
        rule_id: "ANTI_SLOP_01_SMART_QUOTES",
        category: "SyntaxIntegrity",
        severity: "Error",
        message: "Contains curly smart quotes instead of standard ASCII quotes",
        line: lineNum,
        snippet: line.trim(),
        auto_fixable: true,
      });
    }

    // Check for default AI purple/indigo gradients
    if (
      lower.includes("#6366f1") ||
      lower.includes("#8b5cf6") ||
      lower.includes("#7c3aed") ||
      lower.includes("rgb(99, 102, 241)") ||
      lower.includes("rgb(139, 92, 246)") ||
      lower.includes("from-purple-500") ||
      lower.includes("from-indigo-600")
    ) {
      issues.push({
        rule_id: "ANTI_SLOP_02_PURPLE_INDIGO_GRADIENT",
        category: "AntiAiSlop",
        severity: "Warning",
        message: "Detected unrequested default AI purple/indigo gradient tell. Use semantic tokens instead.",
        line: lineNum,
        snippet: line.trim(),
        auto_fixable: true,
      });
    }

    // Check for emoji used as icons in buttons/badges
    const emojiTells = ["✨", "🚀", "💡", "🎉", "🔥", "⚡", "🤖", "🪄"];
    if (
      emojiTells.some((e) => line.includes(e)) &&
      (lower.includes("<button") || lower.includes("btn") || lower.includes("badge") || lower.includes("<badge"))
    ) {
      issues.push({
        rule_id: "ANTI_SLOP_03_EMOJI_ICON_SUBSTITUTE",
        category: "AntiAiSlop",
        severity: "Warning",
        message: "Detected emoji used as icon in button/badge. Use SVGs from icon registry.",
        line: lineNum,
        snippet: line.trim(),
        auto_fixable: false,
      });
    }

    // Check for lorem ipsum
    if (lower.includes("lorem ipsum") || lower.includes("dolor sit amet")) {
      issues.push({
        rule_id: "ANTI_SLOP_04_LOREM_IPSUM_PLACEHOLDER",
        category: "AntiAiSlop",
        severity: "Warning",
        message: "Detected lorem ipsum placeholder text. Use realistic domain sample data.",
        line: lineNum,
        snippet: line.trim(),
        auto_fixable: false,
      });
    }

    // Check for fabricated metrics
    if (
      (lower.includes("10x faster") ||
        lower.includes("100x faster") ||
        lower.includes("99.9% uptime") ||
        lower.includes("trusted by 10,000+")) &&
      !lower.includes("data-source") &&
      !lower.includes("citation")
    ) {
      issues.push({
        rule_id: "ANTI_SLOP_05_FABRICATED_METRIC_CLAIM",
        category: "AntiAiSlop",
        severity: "Warning",
        message: "Detected ungrounded metric claim without citation or data-source attribute.",
        line: lineNum,
        snippet: line.trim(),
        auto_fixable: false,
      });
    }
  });

  return issues;
}

export function checkGeometricAndWcagRules(content: string): LintIssue[] {
  const issues: LintIssue[] = [];
  const lines = content.split("\n");
  const lowerFull = content.toLowerCase();

  lines.forEach((line, idx) => {
    const lineNum = idx + 1;
    const lower = line.toLowerCase();

    // Concentric radius
    if (
      (lower.includes("border-radius") || lower.includes("rounded")) &&
      lower.includes("padding") &&
      lower.includes("inner")
    ) {
      issues.push({
        rule_id: "RULE_GEO_01_CONCENTRIC_RADIUS",
        category: "GeometricLayout",
        severity: "Warning",
        message: "Verify concentric border radius: R_inner = max(4, R_outer - P/2)",
        line: lineNum,
        snippet: line.trim(),
        auto_fixable: true,
      });
    }

    // WCAG contrast
    if (
      (lower.includes("color: #999") || lower.includes("color: #aaa") || lower.includes("color: #bbb")) &&
      (lower.includes("background: #fff") || lower.includes("bg-white") || lower.includes("background-color: #fff"))
    ) {
      issues.push({
        rule_id: "RULE_GEO_02_WCAG_CONTRAST_AA",
        category: "WcagContrast",
        severity: "Error",
        message: "Contrast ratio falls below WCAG 2.2 AA (4.5:1). Darken foreground text.",
        line: lineNum,
        snippet: line.trim(),
        auto_fixable: true,
      });
    }

    // Focus outline
    if (
      (lower.includes("outline: none") || lower.includes("outline: 0")) &&
      !lower.includes("focus-visible") &&
      !lower.includes("ring")
    ) {
      issues.push({
        rule_id: "RULE_GEO_10_INTERACTIVE_ROLE_FOCUS",
        category: "WcagContrast",
        severity: "Error",
        message: "'outline: none' removes keyboard focus ring without providing focus-visible alternative.",
        line: lineNum,
        snippet: line.trim(),
        auto_fixable: true,
      });
    }

    // Positive tabindex
    if (lower.includes('tabindex="1"') || lower.includes('tabindex="2"') || lower.includes('tabindex="3"')) {
      issues.push({
        rule_id: "RULE_GEO_23_TAB_ORDER_INTEGRITY",
        category: "WcagContrast",
        severity: "Error",
        message: 'Positive tabindex disrupts natural DOM reading order. Use tabindex="0" or "-1".',
        line: lineNum,
        snippet: line.trim(),
        auto_fixable: true,
      });
    }

    // Icon accessibility
    if (
      (lower.includes("<svg") || lower.includes('class="icon')) &&
      !lower.includes("aria-hidden") &&
      !lower.includes("aria-label")
    ) {
      issues.push({
        rule_id: "RULE_GEO_16_ICON_ALIGNMENT",
        category: "WcagContrast",
        severity: "Info",
        message: "Icon missing 'aria-hidden=\"true\"' or accessible 'aria-label'.",
        line: lineNum,
        snippet: line.trim(),
        auto_fixable: true,
      });
    }
  });

  // Viewport check
  if (lowerFull.includes("<html") && !lowerFull.includes('name="viewport"')) {
    issues.push({
      rule_id: "RULE_GEO_15_VIEWPORT_META",
      category: "GeometricLayout",
      severity: "Error",
      message: 'HTML document missing viewport meta tag.',
      line: 1,
      auto_fixable: true,
    });
  }

  return issues;
}

export function autoFixIssues(content: string): { fixed: string; count: number } {
  let fixed = content;
  let count = 0;

  // 1. Smart quotes
  if (fixed.includes("“") || fixed.includes("”") || fixed.includes("‘") || fixed.includes("’")) {
    fixed = fixed
      .replace(/“/g, '"')
      .replace(/”/g, '"')
      .replace(/‘/g, "'")
      .replace(/’/g, "'");
    count++;
  }

  // 2. Gradients
  if (fixed.includes("#6366f1") || fixed.includes("#8b5cf6") || fixed.includes("#7c3aed")) {
    fixed = fixed
      .replace(/#6366f1/g, "var(--action-primary, #2563eb)")
      .replace(/#8b5cf6/g, "var(--action-primary-hover, #1d4ed8)")
      .replace(/#7c3aed/g, "var(--action-primary, #2563eb)");
    count++;
  }

  // 3. Tabindex
  if (fixed.includes('tabindex="1"') || fixed.includes('tabindex="2"') || fixed.includes('tabindex="3"')) {
    fixed = fixed.replace(/tabindex="[1-9]"/g, 'tabindex="0"');
    count++;
  }

  // 4. SVG aria-hidden
  if (fixed.includes("<svg") && !fixed.includes("aria-hidden") && !fixed.includes("aria-label")) {
    fixed = fixed.replace(/<svg/g, '<svg aria-hidden="true"');
    count++;
  }

  // 5. Line-height
  if (fixed.includes("line-height: 1;")) {
    fixed = fixed.replace(/line-height: 1;/g, "line-height: 1.5;");
    count++;
  }

  // 6. Viewport meta
  if (fixed.includes("<head>") && !fixed.includes('name="viewport"')) {
    fixed = fixed.replace(
      "<head>",
      '<head>\n    <meta name="viewport" content="width=device-width, initial-scale=1.0">'
    );
    count++;
  }

  return { fixed, count };
}

export class TriangleLinter {
  static lint(content: string): TriangleLintReport {
    const start = Date.now();
    const issues = [...scanSlopAndTells(content), ...checkGeometricAndWcagRules(content)];
    const duration_ms = Date.now() - start;

    return {
      total_issues: issues.length,
      new_in_this_call: issues.length,
      fixed_issues: 0,
      passed: issues.length === 0,
      duration_ms,
      issues,
    };
  }

  static lintAndFix(content: string): { content: string; report: TriangleLintReport } {
    const start = Date.now();
    const { fixed, count } = autoFixIssues(content);
    const recheck = this.lint(fixed);
    const duration_ms = Date.now() - start;

    return {
      content: fixed,
      report: {
        total_issues: recheck.total_issues,
        new_in_this_call: recheck.total_issues,
        fixed_issues: count,
        passed: recheck.passed,
        duration_ms,
        issues: recheck.issues,
      },
    };
  }
}
