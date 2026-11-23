// @story 08.5
// @implements AC-01 (Linear-time NFA Rule Packs: tells, impeccable, hallmark, stageshow <= 25ms)
use std::sync::OnceLock;
use std::time::Instant;

use regex::Regex;
use serde::{Deserialize, Serialize};

pub const DE_NFA_SLA_MS: u64 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NfaPackKind {
    Tells,
    Impeccable,
    Hallmark,
    Stageshow,
}

impl NfaPackKind {
    pub fn all() -> &'static [NfaPackKind] {
        &[
            NfaPackKind::Tells,
            NfaPackKind::Impeccable,
            NfaPackKind::Hallmark,
            NfaPackKind::Stageshow,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            NfaPackKind::Tells => "tells",
            NfaPackKind::Impeccable => "impeccable",
            NfaPackKind::Hallmark => "hallmark",
            NfaPackKind::Stageshow => "stageshow",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DeFindingSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeFinding {
    pub rule_id: String,
    pub pack: String,
    pub severity: DeFindingSeverity,
    pub message: String,
    pub line: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NfaPackReport {
    pub packs_run: Vec<NfaPackKind>,
    pub findings: Vec<DeFinding>,
    pub duration_ms: u64,
    pub duration_micros: u64,
    pub within_sla: bool,
    pub passed: bool,
}

struct CompiledNfaRule {
    rule_id: &'static str,
    pack: NfaPackKind,
    severity: DeFindingSeverity,
    message: &'static str,
    regex: Regex,
}

fn compiled_rules() -> &'static [CompiledNfaRule] {
    static RULES: OnceLock<Vec<CompiledNfaRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let defs: &[(&str, NfaPackKind, DeFindingSeverity, &str, &str)] = &[
            // ── TELLS pack (AI-slop clichés, smart quotes, generic indigo gradients) ──
            (
                "TELLS-001",
                NfaPackKind::Tells,
                DeFindingSeverity::Error,
                "AI-slop cliché phrase detected in copy; replace with concrete domain language",
                r"(?i)\b(delve into|tapestry of|unleash the power|elevate your workflow|seamless symphony|game-changing paradigm|revolutionize your journey|lorem ipsum)\b",
            ),
            (
                "TELLS-002",
                NfaPackKind::Tells,
                DeFindingSeverity::Warning,
                "Curly smart quotes detected in markup/code; use straight ASCII quotes",
                r"[\u{201C}\u{201D}\u{2018}\u{2019}]",
            ),
            (
                "TELLS-003",
                NfaPackKind::Tells,
                DeFindingSeverity::Error,
                "Generic AI indigo/purple template gradient detected; use brand semantic tokens",
                r"(?i)linear-gradient\([^)]{0,80}(#6366f1|#667eea|#764ba2|#8b5cf6)",
            ),
            (
                "TELLS-004",
                NfaPackKind::Tells,
                DeFindingSeverity::Error,
                "Unresolved placeholder copy (TODO/TBD/FIXME) leaked into UI surface",
                r"(?i)(?:>|\bplaceholder\s*=\s*['\x22])[^<]{0,24}\b(TODO|TBD|FIXME|INSERT_HERE)\b",
            ),
            // ── IMPECCABLE pack (token discipline, no Tailwind clusters, no !important) ──
            (
                "IMPECCABLE-001",
                NfaPackKind::Impeccable,
                DeFindingSeverity::Error,
                "Raw Tailwind utility class cluster detected; bind styles to semantic CSS tokens",
                r#"(?i)class(?:Name)?\s*=\s*["'][^"']*\b(?:bg|text|px|py|mt|mb|rounded|shadow)-(?:slate|gray|zinc|indigo|purple|blue|red|green)-[1-9]00\b"#,
            ),
            (
                "IMPECCABLE-002",
                NfaPackKind::Impeccable,
                DeFindingSeverity::Warning,
                "Inline hardcoded hex color in style attribute; use semantic CSS variable var(--*)",
                r#"(?i)style\s*=\s*["'][^"']*(?:color|background(?:-color)?|border-color)\s*:\s*#[0-9a-f]{3,8}\b"#,
            ),
            (
                "IMPECCABLE-003",
                NfaPackKind::Impeccable,
                DeFindingSeverity::Warning,
                "CSS !important override weakens token cascade integrity",
                r"(?i)!\s*important\b",
            ),
            // ── HALLMARK pack (typography, spacing & craft discipline) ──
            (
                "HALLMARK-001",
                NfaPackKind::Hallmark,
                DeFindingSeverity::Error,
                "Unprofessional novelty font family detected; use semantic font tokens",
                r"(?i)font-family\s*:\s*[^;]{0,60}\b(Comic Sans MS|Papyrus|Brush Script)\b",
            ),
            (
                "HALLMARK-002",
                NfaPackKind::Hallmark,
                DeFindingSeverity::Warning,
                "Cramped line-height (< 1.2) degrades body readability",
                r#"(?i)line-height\s*:\s*(?:0?\.[0-9]+|1\.0[0-9]?|1\.1[0-9]?)(?:\s|;|'|"|\})"#,
            ),
            (
                "HALLMARK-003",
                NfaPackKind::Hallmark,
                DeFindingSeverity::Warning,
                "Microscopic font-size (< 10px) violates legibility craft baseline",
                r"(?i)font-size\s*:\s*(?:[0-9](?:\.[0-9]+)?)px\b",
            ),
            // ── STAGESHOW pack (presentation slide structure & bullet discipline) ──
            (
                "STAGESHOW-001",
                NfaPackKind::Stageshow,
                DeFindingSeverity::Error,
                "Slide heading skips hierarchy directly from h1 to h4/h5/h6",
                r"(?is)<h1\b[^>]*>[^<]{1,120}</h1>[\s\r\n]{0,120}<h[456]\b",
            ),
            (
                "STAGESHOW-003",
                NfaPackKind::Stageshow,
                DeFindingSeverity::Error,
                "Raw unguarded script tag inside slide/UI artifact",
                r"(?i)<script\b",
            ),
        ];

        defs.iter()
            .map(|(rule_id, pack, severity, message, pattern)| CompiledNfaRule {
                rule_id,
                pack: *pack,
                severity: *severity,
                message,
                regex: Regex::new(pattern).expect("static linear-NFA regex must compile"),
            })
            .collect()
    })
}

fn stageshow_list_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<(?:ul|ol)\b[^>]*>.*?</(?:ul|ol)>").expect("list regex"))
}

fn stageshow_li_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)<li\b").expect("li regex"))
}

fn line_number_at(content: &str, byte_offset: usize) -> usize {
    let clamped = byte_offset.min(content.len());
    content[..clamped].bytes().filter(|&b| b == b'\n').count() + 1
}

fn excerpt_snippet(content: &str, start: usize, end: usize) -> String {
    let raw = &content[start.min(content.len())..end.min(content.len())];
    let compact: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(80)
        .collect();
    compact.trim().to_string()
}

/// Runs the selected linear-time NFA packs over `content`.
/// Rust's `regex` engine guarantees O(n * m) linear matching without exponential backtracking.
pub fn run_nfa_packs(content: &str, packs: &[NfaPackKind]) -> NfaPackReport {
    let rules = compiled_rules();
    let list_re = stageshow_list_re();
    let li_re = stageshow_li_re();
    let start = Instant::now();
    let active_packs: Vec<NfaPackKind> = if packs.is_empty() {
        NfaPackKind::all().to_vec()
    } else {
        packs.to_vec()
    };

    let mut findings = Vec::new();
    for rule in rules {
        if !active_packs.contains(&rule.pack) {
            continue;
        }
        // Cap matches per rule to keep output bounded even on huge files
        for m in rule.regex.find_iter(content).take(16) {
            findings.push(DeFinding {
                rule_id: rule.rule_id.to_string(),
                pack: rule.pack.as_str().to_string(),
                severity: rule.severity,
                message: rule.message.to_string(),
                line: line_number_at(content, m.start()),
                snippet: excerpt_snippet(content, m.start(), m.end()),
            });
        }
    }

    if active_packs.contains(&NfaPackKind::Stageshow) {
        for m in list_re.find_iter(content).take(16) {
            let li_count = li_re.find_iter(m.as_str()).count();
            if li_count >= 7 {
                findings.push(DeFinding {
                    rule_id: "STAGESHOW-002".to_string(),
                    pack: "stageshow".to_string(),
                    severity: DeFindingSeverity::Warning,
                    message: "Slide bullet wall detected (>= 7 list items in a single list)"
                        .to_string(),
                    line: line_number_at(content, m.start()),
                    snippet: format!("{li_count} list items"),
                });
            }
        }
    }

    let elapsed = start.elapsed();
    let duration_micros = elapsed.as_micros() as u64;
    let duration_ms = elapsed.as_millis() as u64;
    let passed = !findings
        .iter()
        .any(|f| matches!(f.severity, DeFindingSeverity::Error));

    NfaPackReport {
        packs_run: active_packs,
        findings,
        duration_ms,
        duration_micros,
        within_sla: duration_ms <= DE_NFA_SLA_MS,
        passed,
    }
}
