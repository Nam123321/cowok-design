// @story 08.1
// @implements AC-04, AC-08
import { LintIssue, LintIssueCategory, LintSeverity, TriangleLintReport } from "./types";

interface RuleSpec {
  id: string;
  cat: LintIssueCategory;
  sev: LintSeverity;
  msg: string;
  fix: boolean;
  match: (lower: string) => boolean;
}

function makeIssue(spec: RuleSpec, line: number, snippet?: string): LintIssue {
  return {
    rule_id: spec.id,
    category: spec.cat,
    severity: spec.sev,
    message: spec.msg,
    line,
    snippet: snippet?.trim(),
    auto_fixable: spec.fix,
  };
}

const SLOP_RULES: RuleSpec[] = [
  {
    id: "ANTI_SLOP_02_PURPLE_INDIGO_GRADIENT",
    cat: "AntiAiSlop",
    sev: "Warning",
    msg: "Detected unrequested default AI purple/indigo gradient tell. Use semantic tokens instead.",
    fix: true,
    match: (l) => ["#6366f1", "#8b5cf6", "#7c3aed", "rgb(99, 102, 241)", "rgb(139, 92, 246)", "from-purple-500", "from-indigo-600"].some((k) => l.includes(k)),
  },
  {
    id: "ANTI_SLOP_03_EMOJI_ICON_SUBSTITUTE",
    cat: "AntiAiSlop",
    sev: "Warning",
    msg: "Detected emoji used as icon in button/badge. Use SVGs from icon registry.",
    fix: false,
    match: (l) => ["✨", "🚀", "💡", "🎉", "🔥", "⚡", "🤖", "🪄"].some((e) => l.includes(e)) && ["<button", "btn", "badge", "<badge"].some((k) => l.includes(k)),
  },
  {
    id: "ANTI_SLOP_04_LOREM_IPSUM_PLACEHOLDER",
    cat: "AntiAiSlop",
    sev: "Warning",
    msg: "Detected lorem ipsum placeholder text. Use realistic domain sample data.",
    fix: false,
    match: (l) => l.includes("lorem ipsum") || l.includes("dolor sit amet"),
  },
  {
    id: "ANTI_SLOP_05_FABRICATED_METRIC_CLAIM",
    cat: "AntiAiSlop",
    sev: "Warning",
    msg: "Detected ungrounded metric claim without citation or data-source attribute.",
    fix: false,
    match: (l) => ["10x faster", "100x faster", "99.9% uptime", "trusted by 10,000+"].some((k) => l.includes(k)) && !l.includes("data-source") && !l.includes("citation"),
  },
  {
    id: "ANTI_SLOP_06_XSS_SCRIPT_HYGIENE",
    cat: "SyntaxIntegrity",
    sev: "Error",
    msg: "Detected unsafe inline <script>, dangerously" + "SetInnerHTML, or javascript: URI.",
    fix: false,
    match: (l) => l.includes("<script") || l.includes("dangerously" + "setinnerhtml") || l.includes("javascript:"),
  },
];

const GEO_LINE_RULES: RuleSpec[] = [
  { id: "RULE_GEO_01_CONCENTRIC_RADIUS", cat: "GeometricLayout", sev: "Warning", msg: "Verify concentric border radius: R_inner = max(4, R_outer - P/2)", fix: true, match: (l) => (l.includes("border-radius") || l.includes("rounded")) && l.includes("padding") && l.includes("inner") },
  { id: "RULE_GEO_02_WCAG_CONTRAST_AA", cat: "WcagContrast", sev: "Error", msg: "Contrast ratio falls below WCAG 2.2 AA (4.5:1). Darken foreground text.", fix: true, match: (l) => ["color: #999", "color: #aaa", "color: #bbb"].some((k) => l.includes(k)) && ["background: #fff", "bg-white", "background-color: #fff"].some((b) => l.includes(b)) },
  { id: "RULE_GEO_03_TEXT_OVERFLOW", cat: "GeometricLayout", sev: "Warning", msg: "Fixed width text element lacks overflow/ellipsis or word-break guard.", fix: true, match: (l) => (l.includes("width:") || l.includes("w-[")) && ["<p", "<span", "<h"].some((t) => l.includes(t)) && l.includes("px") && !["truncate", "ellipsis", "break-words", "overflow"].some((g) => l.includes(g)) },
  { id: "RULE_GEO_04_TOUCH_TARGET", cat: "GeometricLayout", sev: "Warning", msg: "Interactive button height < 44px without touch target expansion.", fix: false, match: (l) => (l.includes("<button") || l.includes('class="btn"')) && ["height: 24px", "height: 28px", "height: 32px"].some((h) => l.includes(h)) },
  { id: "RULE_GEO_05_LINE_HEIGHT_RATIO", cat: "GeometricLayout", sev: "Warning", msg: "Paragraph line-height is 1.0; body text requires line-height between 1.4 and 1.6.", fix: true, match: (l) => (l.includes("line-height: 1;") || l.includes("leading-none")) && ["<p", "body", "prose"].some((t) => l.includes(t)) },
  { id: "RULE_GEO_06_SPACING_GRID_SCALE", cat: "GeometricLayout", sev: "Warning", msg: "Off-grid spacing pixel literal detected; align to 4px/8px geometric rhythm.", fix: false, match: (l) => ["padding: 13px", "padding: 15px", "margin: 7px", "gap: 11px"].some((p) => l.includes(p)) },
  { id: "RULE_GEO_08_MIN_FONT_SIZE", cat: "GeometricLayout", sev: "Error", msg: "Font size < 10px violates minimum readability standard.", fix: true, match: (l) => ["font-size: 8px", "font-size: 9px", "text-[8px]", "text-[9px]"].some((f) => l.includes(f)) },
  { id: "RULE_GEO_09_HORIZONTAL_OVERFLOW", cat: "GeometricLayout", sev: "Warning", msg: "Large fixed pixel width risks horizontal overflow on tablet/mobile viewports.", fix: false, match: (l) => ["min-width: 1200px", "width: 1440px", "w-[1200px]", "w-[1440px]"].some((w) => l.includes(w)) && !l.includes("max-width") },
  { id: "RULE_GEO_10_INTERACTIVE_ROLE_FOCUS", cat: "WcagContrast", sev: "Error", msg: "'outline: none' removes keyboard focus ring without providing focus-visible alternative.", fix: true, match: (l) => (l.includes("outline: none") || l.includes("outline: 0")) && !["focus-visible", "ring", "box-shadow"].some((g) => l.includes(g)) },
  { id: "RULE_GEO_11_ASPECT_RATIO_DISTORTION", cat: "GeometricLayout", sev: "Info", msg: "Image element lacks explicit aspect-ratio or object-fit styling to prevent layout shift.", fix: false, match: (l) => l.includes("<img") && !["aspect-ratio", "object-fit", "width="].some((g) => l.includes(g)) },
  { id: "RULE_GEO_12_COLOR_ONLY_STATUS", cat: "WcagContrast", sev: "Warning", msg: "Status badge conveys state via color only without label or icon indicator.", fix: false, match: (l) => (l.includes("bg-red-500") || l.includes("bg-green-500")) && l.includes("badge") && !l.includes("aria-label") && (l.includes("></") || l.includes("status-dot")) },
  { id: "RULE_GEO_13_TRUNCATION_TOOLTIP", cat: "GeometricLayout", sev: "Info", msg: "Truncated text element lacks accessible title or aria-label fallback.", fix: false, match: (l) => (l.includes("truncate") || l.includes("text-overflow: ellipsis")) && !l.includes("title=") && !l.includes("aria-label=") },
  { id: "RULE_GEO_14_Z_INDEX_INFLATION", cat: "GeometricLayout", sev: "Warning", msg: "Uncontrolled z-index inflation (>9999). Use structured stacking context layers.", fix: true, match: (l) => l.includes("z-index: 99999") || l.includes("z-index: 999999") || l.includes("z-[99999]") },
  { id: "RULE_GEO_16_ICON_ALIGNMENT", cat: "WcagContrast", sev: "Info", msg: "Icon missing 'aria-hidden=\"true\"' or accessible 'aria-label'.", fix: true, match: (l) => (l.includes("<svg") || l.includes('class="icon')) && !l.includes("aria-hidden") && !l.includes("aria-label") },
  { id: "RULE_GEO_17_MEDIA_CONTROLS_A11Y", cat: "WcagContrast", sev: "Warning", msg: "Media element lacks native 'controls' attribute for keyboard/assistive playback.", fix: false, match: (l) => (l.includes("<video") || l.includes("<audio")) && !l.includes("controls") },
  { id: "RULE_GEO_18_BACKDROP_FILTER_FALLBACK", cat: "GeometricLayout", sev: "Warning", msg: "'backdrop-filter' used without fallback background color for unsupported browsers.", fix: true, match: (l) => l.includes("backdrop-filter") && !l.includes("background") && !l.includes("bg-") },
  { id: "RULE_GEO_21_BUTTON_TYPE_EXPLICIT", cat: "WcagContrast", sev: "Info", msg: "Button element lacks explicit type=\"button|submit|reset\" attribute.", fix: false, match: (l) => l.includes("<button") && !l.includes("type=") },
  { id: "RULE_GEO_22_FORM_LABEL_ASSOCIATION", cat: "WcagContrast", sev: "Warning", msg: "Form input lacks id (for <label for=\"...\">) or aria-label attribute.", fix: false, match: (l) => l.includes("<input") && !l.includes('type="hidden"') && !l.includes("aria-label") && !l.includes("id=") },
  { id: "RULE_GEO_23_TAB_ORDER_INTEGRITY", cat: "WcagContrast", sev: "Error", msg: 'Positive tabindex disrupts natural DOM reading order. Use tabindex="0" or "-1".', fix: true, match: (l) => l.includes('tabindex="1"') || l.includes('tabindex="2"') || l.includes('tabindex="3"') },
  { id: "RULE_GEO_24_TRANSITION_PERFORMANCE", cat: "GeometricLayout", sev: "Warning", msg: "Animating layout properties (width/height/top/left) triggers reflow. Use transform/opacity.", fix: false, match: (l) => (l.includes("transition:") || l.includes("transition-all")) && ["width", "height", "top", "left"].some((p) => l.includes(p)) },
  { id: "RULE_GEO_25_STICKY_HEADER_SHADOW", cat: "GeometricLayout", sev: "Info", msg: "Sticky header lacks bottom border or elevation shadow to separate from scrolled content.", fix: true, match: (l) => l.includes("position: sticky") && l.includes("top: 0") && !l.includes("border") && !l.includes("shadow") },
];

const GEO_DOC_RULES: RuleSpec[] = [
  { id: "RULE_GEO_07_HEADING_HIERARCHY", cat: "WcagContrast", sev: "Warning", msg: "Heading hierarchy skips <h2> between <h1> and <h3>.", fix: false, match: (f) => f.includes("<h1") && f.includes("<h3") && !f.includes("<h2") },
  { id: "RULE_GEO_15_VIEWPORT_META", cat: "GeometricLayout", sev: "Error", msg: 'HTML document missing \'<meta name="viewport" content="width=device-width, initial-scale=1.0">\'.', fix: true, match: (f) => f.includes("<html") && !f.includes('name="viewport"') },
  { id: "RULE_GEO_19_MOTION_REDUCED_PREF", cat: "WcagContrast", sev: "Warning", msg: "CSS animation lacks '@media (prefers-reduced-motion: reduce)' guard.", fix: false, match: (f) => (f.includes("@keyframes") || f.includes("animation:")) && !f.includes("prefers-reduced-motion") },
  { id: "RULE_GEO_20_FONT_DISPLAY_SWAP", cat: "GeometricLayout", sev: "Info", msg: "@font-face declaration lacks font-display: swap or optional to avoid FOIT.", fix: false, match: (f) => f.includes("@font-face") && !f.includes("font-display: swap") && !f.includes("font-display: optional") },
];

export function scanSlopAndTells(content: string): LintIssue[] {
  const issues: LintIssue[] = [];
  const smartQuotesRule: RuleSpec = { id: "ANTI_SLOP_01_SMART_QUOTES", cat: "SyntaxIntegrity", sev: "Error", msg: "Contains curly smart quotes instead of standard ASCII quotes", fix: true, match: () => true };

  content.split("\n").forEach((line, idx) => {
    const lineNum = idx + 1;
    if (line.includes("“") || line.includes("”") || line.includes("‘") || line.includes("’")) {
      issues.push(makeIssue(smartQuotesRule, lineNum, line));
    }
    const lower = line.toLowerCase();
    for (const rule of SLOP_RULES) {
      if (rule.match(lower)) issues.push(makeIssue(rule, lineNum, line));
    }
  });

  const beaconSpec: RuleSpec = { id: "ANTI_SLOP_07_EXTERNAL_BEACON", cat: "SyntaxIntegrity", sev: "Error", msg: "Detected external image URL beacon in Markdown or <img> element.", fix: false, match: () => true };
  for (const re of [/!\[[^\]]*\]\(\s*https?:\/\/[^)]*\)/gi, /<img\b[^>]*\bsrc\s*=\s*["']https?:\/\/[^"']*["'][^>]*>/gi]) {
    for (const m of content.matchAll(re)) {
      const idx = m.index ?? 0;
      issues.push(makeIssue(beaconSpec, content.slice(0, idx).split("\n").length, m[0].replace(/\s+/g, " ")));
    }
  }
  return issues;
}

export function checkGeometricAndWcagRules(content: string): LintIssue[] {
  const issues: LintIssue[] = [];
  const lowerFull = content.toLowerCase();

  content.split("\n").forEach((line, idx) => {
    const lower = line.toLowerCase();
    for (const rule of GEO_LINE_RULES) {
      if (rule.match(lower)) issues.push(makeIssue(rule, idx + 1, line));
    }
  });

  for (const docRule of GEO_DOC_RULES) {
    if (docRule.match(lowerFull)) issues.push(makeIssue(docRule, 1));
  }
  return issues;
}

export function autoFixIssues(content: string): { fixed: string; count: number } {
  let fixed = content;
  let count = 0;

  if (/[“”‘’]/.test(fixed)) {
    fixed = fixed.replace(/[“”]/g, '"').replace(/[‘’]/g, "'");
    count++;
  }
  if (fixed.includes("#6366f1") || fixed.includes("#8b5cf6") || fixed.includes("#7c3aed")) {
    fixed = fixed.replace(/#6366f1/g, "var(--action-primary, #2563eb)").replace(/#8b5cf6/g, "var(--action-primary-hover, #1d4ed8)").replace(/#7c3aed/g, "var(--action-primary, #2563eb)");
    count++;
  }
  if (/tabindex="[1-9]"/.test(fixed)) {
    fixed = fixed.replace(/tabindex="[1-9]"/g, 'tabindex="0"');
    count++;
  }
  if (fixed.includes("<svg") && !fixed.includes("aria-hidden") && !fixed.includes("aria-label")) {
    fixed = fixed.replace(/<svg/g, '<svg aria-hidden="true"');
    count++;
  }
  if (fixed.includes("line-height: 1;")) {
    fixed = fixed.replace(/line-height: 1;/g, "line-height: 1.5;");
    count++;
  }
  if (fixed.includes("<head>") && !fixed.includes('name="viewport"')) {
    fixed = fixed.replace("<head>", '<head>\n    <meta name="viewport" content="width=device-width, initial-scale=1.0">');
    count++;
  }
  return { fixed, count };
}

export class TriangleLinter {
  static lint(content: string): TriangleLintReport {
    const start = Date.now();
    const issues = [...scanSlopAndTells(content), ...checkGeometricAndWcagRules(content)];
    return {
      total_issues: issues.length,
      new_in_this_call: issues.length,
      fixed_issues: 0,
      passed: issues.length === 0,
      duration_ms: Date.now() - start,
      issues,
    };
  }

  static lintAndFix(content: string): { content: string; report: TriangleLintReport } {
    const start = Date.now();
    const { fixed, count } = autoFixIssues(content);
    const recheck = this.lint(fixed);
    return {
      content: fixed,
      report: {
        total_issues: recheck.total_issues,
        new_in_this_call: recheck.total_issues,
        fixed_issues: count,
        passed: recheck.passed,
        duration_ms: Date.now() - start,
        issues: recheck.issues,
      },
    };
  }
}
