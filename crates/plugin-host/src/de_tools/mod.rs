// @story 08.5
// @implements AC-01 (7 Built-in DE Tools: scan-ui, intake, tokens, nfa-packs, archify, get_artifact, notes-check)
pub mod archify_bfs;
pub mod nfa_packs;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use html_forge::oklch::{evaluate_oklch_contrast_guard, OklchContrastReport, WCAG_AA_MIN_CONTRAST};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::models::LintSeverity;
use crate::triangle_linter::TriangleLinter;

pub use archify_bfs::{
    archify_from_text, assert_no_symlink_under_root, bundle_artifacts_bfs, extract_relative_refs,
    read_file_nofollow_bounded, render_archify_svg, ArchifyBfsError, ArchifyEdge, ArchifyGraphIr,
    ArchifyNode, ArchifySvgOutput, ArtifactBundleReport, BundledArtifactEntry, ARCHIFY_MAX_EDGES,
    GET_ARTIFACT_MAX_DEPTH, GET_ARTIFACT_MAX_FILES, GET_ARTIFACT_MAX_FILE_BYTES,
};
pub use nfa_packs::{
    run_nfa_packs, DeFinding, DeFindingSeverity, NfaPackKind, NfaPackReport, DE_NFA_SLA_MS,
};

pub const KIND_CAP_UI_MAX_FILES: usize = 32;
pub const KIND_CAP_DATA_MAX_FILES: usize = 16;
pub const KIND_CAP_ASSET_MAX_FILES: usize = 24;
pub const KIND_CAP_DOC_MAX_FILES: usize = 16;
pub const KIND_CAP_MAX_TOTAL_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuiltInDeTool {
    #[serde(rename = "scan-ui")]
    ScanUi,
    #[serde(rename = "intake")]
    Intake,
    #[serde(rename = "tokens")]
    Tokens,
    #[serde(rename = "nfa-packs")]
    NfaPacks,
    #[serde(rename = "archify")]
    Archify,
    #[serde(rename = "get_artifact")]
    GetArtifact,
    #[serde(rename = "notes-check")]
    NotesCheck,
}

impl BuiltInDeTool {
    pub fn all() -> &'static [BuiltInDeTool] {
        &[
            BuiltInDeTool::ScanUi,
            BuiltInDeTool::Intake,
            BuiltInDeTool::Tokens,
            BuiltInDeTool::NfaPacks,
            BuiltInDeTool::Archify,
            BuiltInDeTool::GetArtifact,
            BuiltInDeTool::NotesCheck,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BuiltInDeTool::ScanUi => "scan-ui",
            BuiltInDeTool::Intake => "intake",
            BuiltInDeTool::Tokens => "tokens",
            BuiltInDeTool::NfaPacks => "nfa-packs",
            BuiltInDeTool::Archify => "archify",
            BuiltInDeTool::GetArtifact => "get_artifact",
            BuiltInDeTool::NotesCheck => "notes-check",
        }
    }

    pub fn parse_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "scan-ui" | "scan_ui" => Some(Self::ScanUi),
            "intake" => Some(Self::Intake),
            "tokens" => Some(Self::Tokens),
            "nfa-packs" | "nfa_packs" => Some(Self::NfaPacks),
            "archify" => Some(Self::Archify),
            "get_artifact" | "get-artifact" => Some(Self::GetArtifact),
            "notes-check" | "notes_check" => Some(Self::NotesCheck),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeToolResult {
    pub tool: BuiltInDeTool,
    pub passed: bool,
    pub duration_ms: u64,
    pub duration_micros: u64,
    pub findings: Vec<DeFinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeRunReport {
    pub target_artifact_path: String,
    pub passed: bool,
    pub cost_usd: f64,
    pub total_duration_ms: u64,
    pub within_sla: bool,
    pub results: Vec<DeToolResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntakeFileSpec {
    pub path: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntakeCapReport {
    pub counts_by_kind: BTreeMap<String, usize>,
    pub total_bytes: usize,
    pub cap_violations: Vec<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenPairInput {
    pub token_name: String,
    pub fg_hex: String,
    pub bg_hex: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokensCheckReport {
    pub pairs_checked: usize,
    pub clamped_count: usize,
    pub reports: Vec<OklchContrastReport>,
    pub findings: Vec<DeFinding>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotesCheckReport {
    pub word_count: usize,
    pub estimated_seconds: usize,
    pub bullet_count: usize,
    pub findings: Vec<DeFinding>,
    pub passed: bool,
}

fn img_no_alt_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)<img\b[^>]*>").expect("img regex"))
}

fn empty_button_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<button\b[^>]*>\s*</button>").expect("btn regex"))
}

fn hex_pair_fg_first_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\bcolor\s*:\s*(#[0-9a-f]{6}|#[0-9a-f]{3})\b[^{}]{0,80}\bbackground(?:-color)?\s*:\s*(#[0-9a-f]{6}|#[0-9a-f]{3})\b")
            .expect("hex pair fg-first regex")
    })
}

fn hex_pair_bg_first_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\bbackground(?:-color)?\s*:\s*(#[0-9a-f]{6}|#[0-9a-f]{3})\b[^{}]{0,80}\bcolor\s*:\s*(#[0-9a-f]{6}|#[0-9a-f]{3})\b")
            .expect("hex pair bg-first regex")
    })
}

/// Tool 1: `scan-ui` — Deterministic DOM/HTML/TSX accessibility & visual structure audit.
pub fn run_scan_ui(content: &str) -> DeToolResult {
    let start = Instant::now();
    let mut findings = Vec::new();

    let tri = TriangleLinter::lint(content);
    for issue in tri.issues {
        findings.push(DeFinding {
            rule_id: issue.rule_id,
            pack: "scan-ui".to_string(),
            severity: match issue.severity {
                LintSeverity::Error => DeFindingSeverity::Error,
                LintSeverity::Warning => DeFindingSeverity::Warning,
                LintSeverity::Info => DeFindingSeverity::Info,
            },
            message: issue.message,
            line: issue.line.unwrap_or(1),
            snippet: issue.snippet.unwrap_or_default(),
        });
    }

    for m in img_no_alt_re().find_iter(content) {
        let tag = m.as_str();
        if !tag.to_ascii_lowercase().contains("alt=") {
            findings.push(DeFinding {
                rule_id: "SCANUI-A11Y-001".to_string(),
                pack: "scan-ui".to_string(),
                severity: DeFindingSeverity::Error,
                message: "<img> element is missing an alt attribute for screen readers".to_string(),
                line: content[..m.start()].bytes().filter(|&b| b == b'\n').count() + 1,
                snippet: tag.chars().take(64).collect(),
            });
        }
    }

    for m in empty_button_re().find_iter(content) {
        let tag = m.as_str();
        if !tag.to_ascii_lowercase().contains("aria-label") {
            findings.push(DeFinding {
                rule_id: "SCANUI-A11Y-002".to_string(),
                pack: "scan-ui".to_string(),
                severity: DeFindingSeverity::Error,
                message: "Empty <button> element lacks accessible label or aria-label".to_string(),
                line: content[..m.start()].bytes().filter(|&b| b == b'\n').count() + 1,
                snippet: tag.chars().take(64).collect(),
            });
        }
    }

    let elapsed = start.elapsed();
    let passed = !findings
        .iter()
        .any(|f| matches!(f.severity, DeFindingSeverity::Error));
    DeToolResult {
        tool: BuiltInDeTool::ScanUi,
        passed,
        duration_ms: elapsed.as_millis() as u64,
        duration_micros: elapsed.as_micros() as u64,
        findings,
        metadata: Some(serde_json::json!({ "engine": "triangle-linter+a11y" })),
    }
}

fn classify_intake_kind(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".tsx")
        || lower.ends_with(".jsx")
        || lower.ends_with(".html")
        || lower.ends_with(".css")
    {
        "ui"
    } else if lower.ends_with(".json")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".sql")
    {
        "data"
    } else if lower.ends_with(".svg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
        || lower.ends_with(".jpg")
    {
        "asset"
    } else if lower.ends_with(".md") || lower.ends_with(".txt") {
        "doc"
    } else {
        "other"
    }
}

/// Tool 2: `intake` (`KIND_CAP`) — Enforces per-kind file caps & total intake byte budgets.
pub fn run_intake_kind_cap(files: &[IntakeFileSpec]) -> (DeToolResult, IntakeCapReport) {
    let start = Instant::now();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_bytes = 0usize;
    let mut findings = Vec::new();
    let mut violations = Vec::new();

    for f in files {
        let kind = classify_intake_kind(&f.path);
        *counts.entry(kind.to_string()).or_insert(0) += 1;
        total_bytes = total_bytes.saturating_add(f.byte_len);

        let lower = f.path.to_ascii_lowercase();
        if lower.ends_with(".exe")
            || lower.ends_with(".dll")
            || lower.ends_with(".so")
            || lower.ends_with(".dylib")
        {
            let msg = format!("KIND_CAP_BINARY_REJECTED: binary artifact '{}'", f.path);
            violations.push(msg.clone());
            findings.push(DeFinding {
                rule_id: "INTAKE-CAP-BIN".to_string(),
                pack: "intake".to_string(),
                severity: DeFindingSeverity::Error,
                message: msg,
                line: 1,
                snippet: f.path.clone(),
            });
        }
    }

    for (kind, cap) in [
        ("ui", KIND_CAP_UI_MAX_FILES),
        ("data", KIND_CAP_DATA_MAX_FILES),
        ("asset", KIND_CAP_ASSET_MAX_FILES),
        ("doc", KIND_CAP_DOC_MAX_FILES),
    ] {
        let count = counts.get(kind).copied().unwrap_or(0);
        if count > cap {
            let msg = format!("KIND_CAP_EXCEEDED: kind '{kind}' has {count} files (cap {cap})");
            violations.push(msg.clone());
            findings.push(DeFinding {
                rule_id: "INTAKE-CAP-COUNT".to_string(),
                pack: "intake".to_string(),
                severity: DeFindingSeverity::Error,
                message: msg,
                line: 1,
                snippet: format!("{kind}:{count}/{cap}"),
            });
        }
    }

    if total_bytes > KIND_CAP_MAX_TOTAL_BYTES {
        let msg = format!(
            "KIND_CAP_BYTES_EXCEEDED: total {total_bytes} bytes > cap {KIND_CAP_MAX_TOTAL_BYTES}"
        );
        violations.push(msg.clone());
        findings.push(DeFinding {
            rule_id: "INTAKE-CAP-BYTES".to_string(),
            pack: "intake".to_string(),
            severity: DeFindingSeverity::Error,
            message: msg,
            line: 1,
            snippet: format!("{total_bytes}"),
        });
    }

    let passed = violations.is_empty();
    let elapsed = start.elapsed();
    let report = IntakeCapReport {
        counts_by_kind: counts,
        total_bytes,
        cap_violations: violations,
        passed,
    };
    let result = DeToolResult {
        tool: BuiltInDeTool::Intake,
        passed,
        duration_ms: elapsed.as_millis() as u64,
        duration_micros: elapsed.as_micros() as u64,
        findings,
        metadata: serde_json::to_value(&report).ok(),
    };
    (result, report)
}

/// Tool 3: `tokens` — OKLCH auto-contrast verifier via `html-forge::oklch`.
pub fn run_tokens_oklch(
    content: &str,
    explicit_pairs: &[TokenPairInput],
) -> (DeToolResult, TokensCheckReport) {
    let start = Instant::now();
    let mut pairs: Vec<TokenPairInput> = explicit_pairs.to_vec();

    if pairs.is_empty() {
        let mut seen = std::collections::HashSet::new();
        for (idx, cap) in hex_pair_fg_first_re().captures_iter(content).enumerate() {
            if let (Some(fg), Some(bg)) = (cap.get(1), cap.get(2)) {
                let key = (
                    fg.as_str().to_ascii_lowercase(),
                    bg.as_str().to_ascii_lowercase(),
                );
                if seen.insert(key) {
                    pairs.push(TokenPairInput {
                        token_name: format!("inline-pair-{}", idx + 1),
                        fg_hex: fg.as_str().to_string(),
                        bg_hex: bg.as_str().to_string(),
                    });
                }
            }
        }
        for (idx, cap) in hex_pair_bg_first_re().captures_iter(content).enumerate() {
            if let (Some(bg), Some(fg)) = (cap.get(1), cap.get(2)) {
                let key = (
                    fg.as_str().to_ascii_lowercase(),
                    bg.as_str().to_ascii_lowercase(),
                );
                if seen.insert(key) {
                    pairs.push(TokenPairInput {
                        token_name: format!("inline-bg-pair-{}", idx + 1),
                        fg_hex: fg.as_str().to_string(),
                        bg_hex: bg.as_str().to_string(),
                    });
                }
            }
        }
    }

    let mut reports = Vec::new();
    let mut findings = Vec::new();
    let mut clamped_count = 0usize;

    for pair in &pairs {
        match evaluate_oklch_contrast_guard(&pair.fg_hex, &pair.bg_hex, WCAG_AA_MIN_CONTRAST) {
            Ok(rep) => {
                if rep.clamped {
                    clamped_count += 1;
                    findings.push(DeFinding {
                        rule_id: "TOKENS-OKLCH-CONTRAST".to_string(),
                        pack: "tokens".to_string(),
                        severity: DeFindingSeverity::Error,
                        message: format!(
                            "Token '{}' contrast {:.2}:1 < 4.5:1; OKLCH auto-clamp recommends {}",
                            pair.token_name, rep.original_ratio, rep.adjusted_fg_hex
                        ),
                        line: 1,
                        snippet: format!("{} on {}", pair.fg_hex, pair.bg_hex),
                    });
                }
                reports.push(rep);
            }
            Err(e) => {
                findings.push(DeFinding {
                    rule_id: "TOKENS-HEX-INVALID".to_string(),
                    pack: "tokens".to_string(),
                    severity: DeFindingSeverity::Error,
                    message: format!("Invalid token color '{}': {e}", pair.token_name),
                    line: 1,
                    snippet: format!("{}/{}", pair.fg_hex, pair.bg_hex),
                });
            }
        }
    }

    let passed = findings.is_empty();
    let elapsed = start.elapsed();
    let report = TokensCheckReport {
        pairs_checked: pairs.len(),
        clamped_count,
        reports,
        findings: findings.clone(),
        passed,
    };
    let result = DeToolResult {
        tool: BuiltInDeTool::Tokens,
        passed,
        duration_ms: elapsed.as_millis() as u64,
        duration_micros: elapsed.as_micros() as u64,
        findings,
        metadata: serde_json::to_value(&report).ok(),
    };
    (result, report)
}

/// Tool 7: `notes-check` — Speaker notes pacing, bullet density, and slop check.
pub fn run_notes_check(content: &str) -> (DeToolResult, NotesCheckReport) {
    let start = Instant::now();
    let mut findings = Vec::new();
    let trimmed = content.trim();
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    let word_count = words.len();
    let estimated_seconds = (word_count * 60) / 130;
    let bullet_count = content
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("- ") || t.starts_with("* ") || t.starts_with("• ")
        })
        .count();

    if word_count == 0 {
        findings.push(DeFinding {
            rule_id: "NOTES-EMPTY".to_string(),
            pack: "notes-check".to_string(),
            severity: DeFindingSeverity::Error,
            message: "Speaker notes are empty; provide talking points for slide narration"
                .to_string(),
            line: 1,
            snippet: String::new(),
        });
    } else if word_count > 180 {
        findings.push(DeFinding {
            rule_id: "NOTES-OVERFLOW".to_string(),
            pack: "notes-check".to_string(),
            severity: DeFindingSeverity::Error,
            message: format!(
                "Speaker notes too dense ({word_count} words > 180 words per slide budget)"
            ),
            line: 1,
            snippet: format!("{word_count} words (~{estimated_seconds}s)"),
        });
    }

    if bullet_count > 5 {
        findings.push(DeFinding {
            rule_id: "NOTES-BULLET-WALL".to_string(),
            pack: "notes-check".to_string(),
            severity: DeFindingSeverity::Warning,
            message: format!("Too many bullets ({bullet_count} > 5); split slide into two beats"),
            line: 1,
            snippet: format!("{bullet_count} bullets"),
        });
    }

    let lower = trimmed.to_ascii_lowercase();
    for filler in [
        "read this slide verbatim",
        "insert speaker notes",
        "todo: notes",
    ] {
        if lower.contains(filler) {
            findings.push(DeFinding {
                rule_id: "NOTES-PLACEHOLDER".to_string(),
                pack: "notes-check".to_string(),
                severity: DeFindingSeverity::Error,
                message: format!("Placeholder speaker note cue detected: '{filler}'"),
                line: 1,
                snippet: filler.to_string(),
            });
        }
    }

    let passed = !findings
        .iter()
        .any(|f| matches!(f.severity, DeFindingSeverity::Error));
    let elapsed = start.elapsed();
    let report = NotesCheckReport {
        word_count,
        estimated_seconds,
        bullet_count,
        findings: findings.clone(),
        passed,
    };
    let result = DeToolResult {
        tool: BuiltInDeTool::NotesCheck,
        passed,
        duration_ms: elapsed.as_millis() as u64,
        duration_micros: elapsed.as_micros() as u64,
        findings,
        metadata: serde_json::to_value(&report).ok(),
    };
    (result, report)
}

#[derive(Debug, Clone)]
pub struct DeSuiteInput<'a> {
    pub workspace_root: &'a Path,
    pub target_artifact_path: &'a str,
    pub content_override: Option<&'a str>,
    pub tools: &'a [BuiltInDeTool],
    pub token_pairs: &'a [TokenPairInput],
}

/// Runs any subset of the 7 built-in DE tools (`$0`, `<= 25ms` target).
pub fn run_builtin_de_suite(input: &DeSuiteInput<'_>) -> Result<DeRunReport, ArchifyBfsError> {
    let suite_start = Instant::now();
    // H-01: Always jail-validate target_artifact_path BEFORE any filesystem stat/exists check
    let safe_target_path =
        assert_no_symlink_under_root(input.workspace_root, Path::new(input.target_artifact_path))?;
    let loaded_content: String = if let Some(override_str) = input.content_override {
        override_str.to_string()
    } else {
        let (_, bytes) = read_file_nofollow_bounded(
            input.workspace_root,
            &safe_target_path,
            GET_ARTIFACT_MAX_FILE_BYTES,
        )?;
        String::from_utf8_lossy(&bytes).into_owned()
    };

    let selected = if input.tools.is_empty() {
        BuiltInDeTool::all()
    } else {
        input.tools
    };

    let mut results = Vec::with_capacity(selected.len());
    for &tool in selected {
        let res = match tool {
            BuiltInDeTool::ScanUi => run_scan_ui(&loaded_content),
            BuiltInDeTool::Intake => {
                let spec = IntakeFileSpec {
                    path: input.target_artifact_path.to_string(),
                    byte_len: loaded_content.len(),
                };
                run_intake_kind_cap(&[spec]).0
            }
            BuiltInDeTool::Tokens => run_tokens_oklch(&loaded_content, input.token_pairs).0,
            BuiltInDeTool::NfaPacks => {
                let rep = run_nfa_packs(&loaded_content, NfaPackKind::all());
                DeToolResult {
                    tool: BuiltInDeTool::NfaPacks,
                    passed: rep.passed,
                    duration_ms: rep.duration_ms,
                    duration_micros: rep.duration_micros,
                    findings: rep.findings,
                    metadata: Some(serde_json::json!({
                        "packsRun": rep.packs_run,
                        "withinSla": rep.within_sla,
                    })),
                }
            }
            BuiltInDeTool::Archify => {
                let t0 = Instant::now();
                match archify_from_text(&loaded_content, input.target_artifact_path) {
                    Ok(out) => {
                        let el = t0.elapsed();
                        DeToolResult {
                            tool: BuiltInDeTool::Archify,
                            passed: true,
                            duration_ms: el.as_millis() as u64,
                            duration_micros: el.as_micros() as u64,
                            findings: Vec::new(),
                            metadata: serde_json::to_value(&out).ok(),
                        }
                    }
                    Err(e) => {
                        let el = t0.elapsed();
                        DeToolResult {
                            tool: BuiltInDeTool::Archify,
                            passed: false,
                            duration_ms: el.as_millis() as u64,
                            duration_micros: el.as_micros() as u64,
                            findings: vec![DeFinding {
                                rule_id: "ARCHIFY-IR-ERR".to_string(),
                                pack: "archify".to_string(),
                                severity: DeFindingSeverity::Error,
                                message: e.to_string(),
                                line: 1,
                                snippet: String::new(),
                            }],
                            metadata: None,
                        }
                    }
                }
            }
            BuiltInDeTool::GetArtifact => {
                let t0 = Instant::now();
                let is_existing_regular_file = std::fs::symlink_metadata(&safe_target_path)
                    .is_ok_and(|m| m.file_type().is_file());
                if is_existing_regular_file {
                    let bundle = bundle_artifacts_bfs(
                        input.workspace_root,
                        &safe_target_path,
                        GET_ARTIFACT_MAX_DEPTH,
                        GET_ARTIFACT_MAX_FILES,
                    )?;
                    let el = t0.elapsed();
                    DeToolResult {
                        tool: BuiltInDeTool::GetArtifact,
                        passed: true,
                        duration_ms: el.as_millis() as u64,
                        duration_micros: el.as_micros() as u64,
                        findings: Vec::new(),
                        metadata: serde_json::to_value(&bundle).ok(),
                    }
                } else {
                    let refs = extract_relative_refs(&loaded_content);
                    let el = t0.elapsed();
                    DeToolResult {
                        tool: BuiltInDeTool::GetArtifact,
                        passed: true,
                        duration_ms: el.as_millis() as u64,
                        duration_micros: el.as_micros() as u64,
                        findings: Vec::new(),
                        metadata: Some(serde_json::json!({
                            "entryPath": input.target_artifact_path,
                            "maxDepth": GET_ARTIFACT_MAX_DEPTH,
                            "maxFiles": GET_ARTIFACT_MAX_FILES,
                            "discoveredRefs": refs,
                            "inlineMode": true
                        })),
                    }
                }
            }
            BuiltInDeTool::NotesCheck => run_notes_check(&loaded_content).0,
        };
        results.push(res);
    }

    let total_duration_ms = suite_start.elapsed().as_millis() as u64;
    let passed = results.iter().all(|r| r.passed);
    Ok(DeRunReport {
        target_artifact_path: input.target_artifact_path.to_string(),
        passed,
        cost_usd: 0.0,
        total_duration_ms,
        within_sla: total_duration_ms <= DE_NFA_SLA_MS,
        results,
    })
}
