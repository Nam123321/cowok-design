// @story 08.5
// @implements AC-04, AC-05 (System-1 LEARN Loop: V20 gate_failure_ledger, Jev Noul dedupe, hit_count>=3, hysteresis 0.95/0.85, smuggling block, IMMEDIATE transaction atomicity)
use std::sync::OnceLock;

use chrono::Utc;
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::repackager::scan_skill_markdown_safety;

pub const LEARN_MIN_HIT_COUNT: i64 = 3;
pub const LEARN_PROMOTE_SLPR_THRESHOLD: f64 = 0.95;
pub const LEARN_RETIRE_SLPR_THRESHOLD: f64 = 0.85;
pub const LEARN_SCOPE_MAX_CARDS: i64 = 1_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LearnFilterError {
    #[error("LEARN_SMUGGLING_BLOCKED [{code}]: {reason}")]
    SmugglingBlocked { code: String, reason: String },
    #[error("LEARN_SCOPE_CAPACITY_EXCEEDED: active cards {current} >= cap {cap}")]
    ScopeCapacityExceeded { current: i64, cap: i64 },
    #[error("LEARN_INVALID_INPUT: {0}")]
    InvalidInput(String),
    #[error("LEARN_STORAGE_ERROR: {0}")]
    Storage(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LedgerStatus {
    Accumulating,
    Proposed,
    Promoted,
    Retired,
}

impl LedgerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Accumulating => "ACCUMULATING",
            Self::Proposed => "PROPOSED",
            Self::Promoted => "PROMOTED",
            Self::Retired => "RETIRED",
        }
    }

    pub fn parse_str(raw: &str) -> Self {
        match raw.trim().to_ascii_uppercase().as_str() {
            "PROPOSED" => Self::Proposed,
            "PROMOTED" => Self::Promoted,
            "RETIRED" => Self::Retired,
            _ => Self::Accumulating,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordGateFailureInput {
    pub project_id: String,
    pub rule_id: String,
    pub failure_message: String,
    pub slpr_score: f64,
    #[serde(default)]
    pub source_evidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateFailureLedgerEntry {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub rule_id: String,
    pub fingerprint_sha256: String,
    pub sample_message: String,
    pub hit_count: i64,
    pub slpr_score: f64,
    pub promoted_card_id: Option<String>,
    pub status: LedgerStatus,
    pub last_seen_at: i64,
    pub qualifies_for_proposal: bool,
}

fn uuid_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b")
            .expect("uuid regex")
    })
}

fn hex_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(?:0x)?[0-9a-f]{8,}\b").expect("hex regex"))
}

fn num_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b\d+(?:\.\d+)?\b").expect("num regex"))
}

fn normalize_for_smuggling_scan(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let cp = ch as u32;
        if matches!(
            cp,
            0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x206F | 0xFEFF
        ) {
            continue;
        }
        let folded = if (0xFF01..=0xFF5E).contains(&cp) {
            char::from_u32(cp - 0xFEE0).unwrap_or(ch)
        } else if cp == 0x3000 {
            ' '
        } else {
            ch
        };
        if folded == '\\' {
            out.push('/');
        } else {
            out.push(folded);
        }
    }
    out
}

fn smuggling_patterns() -> &'static [(&'static str, &'static str, Regex)] {
    static RULES: OnceLock<Vec<(&'static str, &'static str, Regex)>> = OnceLock::new();
    RULES.get_or_init(|| {
        vec![
            (
                "LEARN-INJ-HUMAN-ONLY",
                "Attempt to manipulate or bypass HUMAN_ONLY approval gate",
                Regex::new(r"(?i)(human_only|bypass\s+human|auto[- ]?approve|skip\s+human|humanworkbenchesession|forge\s+token)")
                    .expect("human-only smuggling regex"),
            ),
            (
                "LEARN-INJ-SEALED-EVAL",
                "Attempt to reference or smuggle tests/eval/sealed/ benchmark data",
                Regex::new(r"(?i)(tests/eval/sealed|eval/sealed|sealed_eval|sealed-eval)")
                    .expect("sealed eval smuggling regex"),
            ),
            (
                "LEARN-INJ-PROMPT",
                "Prompt injection, XML breakout, or XSS payload detected in LEARN candidate",
                Regex::new(r"(?i)(ignore\s+(?:all\s+)?(?:previous|prior|above)\s+instructions|disregard\s+(?:all\s+)?(?:system|previous)\s+prompt|you\s+are\s+now\s+dan|<system\b|</system>|</?learned_cards_data\b|<script\b|javascript:|!\[[^\]]*\]\(https?://)")
                    .expect("prompt injection regex"),
            ),
        ]
    })
}

/// Blocks prompt injection, Unicode zero-width/fullwidth evasion, `HUMAN_ONLY` manipulation,
/// XML prompt breakout (`<system>`, `</learned_cards_data>`), and `tests/eval/sealed/` smuggling.
pub fn check_learn_smuggling_safety(text: &str) -> Result<(), LearnFilterError> {
    if let Err(err) = scan_skill_markdown_safety("learn_candidate", text) {
        return Err(LearnFilterError::SmugglingBlocked {
            code: "LEARN-INJ-MARKDOWN".to_string(),
            reason: err.to_string(),
        });
    }
    let normalized = normalize_for_smuggling_scan(text);
    for candidate in [text, normalized.as_str()] {
        for (code, reason, re) in smuggling_patterns() {
            if re.is_match(candidate) {
                return Err(LearnFilterError::SmugglingBlocked {
                    code: (*code).to_string(),
                    reason: (*reason).to_string(),
                });
            }
        }
    }
    Ok(())
}

/// Computes normalized `Jev Noul` dedupe text and deterministic SHA-256 fingerprint.
pub fn compute_jev_noul_fingerprint(rule_id: &str, message: &str) -> (String, String) {
    let lower = message.trim().to_ascii_lowercase();
    let step1 = uuid_re().replace_all(&lower, "<uuid>");
    let step2 = hex_re().replace_all(&step1, "<hex>");
    let step3 = num_re().replace_all(&step2, "<num>");
    let normalized_noul = step3.split_whitespace().collect::<Vec<_>>().join(" ");
    let canonical_key = format!(
        "{}::{}",
        rule_id.trim().to_ascii_uppercase(),
        normalized_noul
    );
    let sha = format!("{:x}", Sha256::digest(canonical_key.as_bytes()));
    (normalized_noul, sha)
}

/// Hysteresis state machine (`promote >= 0.95` with `hit_count >= 3`, `auto-retire < 0.85`).
/// Dead-band `[0.85, 0.95)` retains current status to prevent statistical oscillation.
pub fn evaluate_hysteresis_transition(
    current: LedgerStatus,
    hit_count: i64,
    slpr_score: f64,
) -> LedgerStatus {
    match current {
        LedgerStatus::Accumulating => {
            if hit_count >= LEARN_MIN_HIT_COUNT && slpr_score >= LEARN_PROMOTE_SLPR_THRESHOLD {
                LedgerStatus::Proposed
            } else {
                LedgerStatus::Accumulating
            }
        }
        LedgerStatus::Proposed | LedgerStatus::Promoted => {
            if slpr_score < LEARN_RETIRE_SLPR_THRESHOLD {
                LedgerStatus::Retired
            } else {
                current
            }
        }
        LedgerStatus::Retired => {
            if hit_count >= LEARN_MIN_HIT_COUNT && slpr_score >= LEARN_PROMOTE_SLPR_THRESHOLD {
                LedgerStatus::Proposed
            } else {
                LedgerStatus::Retired
            }
        }
    }
}

struct ImmediateTxGuard<'a> {
    conn: &'a Connection,
    committed: bool,
}

impl<'a> ImmediateTxGuard<'a> {
    fn begin(conn: &'a Connection) -> Result<Self, LearnFilterError> {
        conn.execute_batch("BEGIN IMMEDIATE;")
            .map_err(|e| LearnFilterError::Storage(e.to_string()))?;
        Ok(Self {
            conn,
            committed: false,
        })
    }

    fn commit(mut self) -> Result<(), LearnFilterError> {
        self.conn
            .execute_batch("COMMIT;")
            .map_err(|e| LearnFilterError::Storage(e.to_string()))?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for ImmediateTxGuard<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.conn.execute_batch("ROLLBACK;");
        }
    }
}

/// Records a gate failure in `V20 gate_failure_ledger` inside an atomic `BEGIN IMMEDIATE` transaction,
/// applies Jev Noul dedupe & hysteresis, and creates a `PROPOSED` card in `knowledge_cards`
/// (capped at `1,000`/scope) for `08.4` `HUMAN_ONLY` review.
pub fn record_gate_failure_and_evaluate(
    conn: &Connection,
    tenant_id: &str,
    input: &RecordGateFailureInput,
) -> Result<GateFailureLedgerEntry, LearnFilterError> {
    let rule_id = input.rule_id.trim();
    let project_id = input.project_id.trim();
    if rule_id.is_empty() || project_id.is_empty() {
        return Err(LearnFilterError::InvalidInput(
            "projectId and ruleId are required".to_string(),
        ));
    }
    if !input.slpr_score.is_finite() || !(0.0..=1.0).contains(&input.slpr_score) {
        return Err(LearnFilterError::InvalidInput(
            "slprScore must be within [0.0, 1.0]".to_string(),
        ));
    }

    check_learn_smuggling_safety(rule_id)?;
    check_learn_smuggling_safety(&input.failure_message)?;
    if let Some(ref ev) = input.source_evidence {
        check_learn_smuggling_safety(ev)?;
    }

    let (noul_text, fingerprint_sha256) =
        compute_jev_noul_fingerprint(rule_id, &input.failure_message);
    let now = Utc::now().timestamp_millis();

    // H-04: Serialize read-check-insert-upsert inside SQLite BEGIN IMMEDIATE transaction
    let tx = ImmediateTxGuard::begin(conn)?;

    let existing: Option<(String, i64, String, Option<String>)> = conn
        .query_row(
            "SELECT id, hit_count, status, promoted_card_id
             FROM gate_failure_ledger
             WHERE tenant_id = ?1 AND project_id = ?2 AND fingerprint_sha256 = ?3",
            params![tenant_id, project_id, fingerprint_sha256],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(|e| LearnFilterError::Storage(e.to_string()))?;

    let (ledger_id, next_hits, prev_status, mut promoted_card_id) = match existing {
        Some((id, hits, status_str, card_id)) => (
            id,
            hits.saturating_add(1),
            LedgerStatus::parse_str(&status_str),
            card_id,
        ),
        None => (
            Uuid::new_v4().to_string(),
            1,
            LedgerStatus::Accumulating,
            None,
        ),
    };

    let next_status = evaluate_hysteresis_transition(prev_status, next_hits, input.slpr_score);

    // If transitioning into PROPOSED and no card linked yet, insert a strictly PROPOSED card
    // into knowledge_cards for Story 08.4 HUMAN_ONLY approval, verifying non-REJECTED state.
    if next_status == LedgerStatus::Proposed && promoted_card_id.is_none() {
        let active_cards: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_cards
                 WHERE tenant_id = ?1 AND project_id = ?2 AND status != 'REJECTED'",
                params![tenant_id, project_id],
                |r| r.get(0),
            )
            .map_err(|e| LearnFilterError::Storage(e.to_string()))?;
        if active_cards >= LEARN_SCOPE_MAX_CARDS {
            return Err(LearnFilterError::ScopeCapacityExceeded {
                current: active_cards,
                cap: LEARN_SCOPE_MAX_CARDS,
            });
        }

        let card_id = format!("learn-{}", &fingerprint_sha256[..16]);
        let title = format!("Prevent recurring {rule_id} gate failure");
        let content = format!(
            "Observed {next_hits} deduplicated occurrences (SLPR {:.2}) for rule `{rule_id}`: {}",
            input.slpr_score, noul_text
        );
        let content_sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
        let idem_key = format!("ledger:{fingerprint_sha256}");
        let source_evidence = input
            .source_evidence
            .clone()
            .unwrap_or_else(|| format!("gate_failure_ledger:{rule_id}"));

        let inserted = conn
            .execute(
                "INSERT OR IGNORE INTO knowledge_cards (
                    id, tenant_id, project_id, idempotency_key, category, title, content,
                    content_sha256, source_evidence, status, revision, created_at
                 ) VALUES (?1, ?2, ?3, ?4, 'RULE', ?5, ?6, ?7, ?8, 'PROPOSED', 1, ?9)",
                params![
                    card_id,
                    tenant_id,
                    project_id,
                    idem_key,
                    title,
                    content,
                    content_sha256,
                    source_evidence,
                    now
                ],
            )
            .map_err(|e| LearnFilterError::Storage(e.to_string()))?;

        if inserted > 0 {
            promoted_card_id = Some(card_id);
        } else {
            let existing_non_rejected: Option<String> = conn
                .query_row(
                    "SELECT id FROM knowledge_cards
                     WHERE tenant_id = ?1 AND project_id = ?2
                       AND (id = ?3 OR idempotency_key = ?4)
                       AND status != 'REJECTED'
                     LIMIT 1",
                    params![tenant_id, project_id, card_id, idem_key],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| LearnFilterError::Storage(e.to_string()))?;
            promoted_card_id = existing_non_rejected;
        }
    }

    conn.execute(
        "INSERT INTO gate_failure_ledger (
            id, tenant_id, project_id, rule_id, fingerprint_sha256, sample_message,
            hit_count, slpr_score, promoted_card_id, status, last_seen_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10)
         ON CONFLICT(tenant_id, project_id, fingerprint_sha256) DO UPDATE SET
            sample_message = excluded.sample_message,
            hit_count = gate_failure_ledger.hit_count + 1,
            slpr_score = excluded.slpr_score,
            promoted_card_id = COALESCE(excluded.promoted_card_id, gate_failure_ledger.promoted_card_id),
            status = excluded.status,
            last_seen_at = excluded.last_seen_at",
        params![
            ledger_id,
            tenant_id,
            project_id,
            rule_id,
            fingerprint_sha256,
            input.failure_message.trim(),
            input.slpr_score,
            promoted_card_id.as_deref(),
            next_status.as_str(),
            now
        ],
    )
    .map_err(|e| LearnFilterError::Storage(e.to_string()))?;

    let persisted_hits: i64 = conn
        .query_row(
            "SELECT hit_count FROM gate_failure_ledger
             WHERE tenant_id = ?1 AND project_id = ?2 AND fingerprint_sha256 = ?3",
            params![tenant_id, project_id, fingerprint_sha256],
            |r| r.get(0),
        )
        .unwrap_or(next_hits);

    tx.commit()?;

    Ok(GateFailureLedgerEntry {
        id: ledger_id,
        tenant_id: tenant_id.to_string(),
        project_id: project_id.to_string(),
        rule_id: rule_id.to_string(),
        fingerprint_sha256,
        sample_message: input.failure_message.trim().to_string(),
        hit_count: persisted_hits,
        slpr_score: input.slpr_score,
        promoted_card_id,
        status: next_status,
        last_seen_at: now,
        qualifies_for_proposal: persisted_hits >= LEARN_MIN_HIT_COUNT
            && input.slpr_score >= LEARN_PROMOTE_SLPR_THRESHOLD,
    })
}

pub fn list_gate_failure_ledger_scoped(
    conn: &Connection,
    tenant_id: &str,
    project_id: &str,
    qualifying_only: bool,
) -> Result<Vec<GateFailureLedgerEntry>, LearnFilterError> {
    let sql = if qualifying_only {
        "SELECT id, tenant_id, project_id, rule_id, fingerprint_sha256, sample_message,
                hit_count, slpr_score, promoted_card_id, status, last_seen_at
         FROM gate_failure_ledger
         WHERE tenant_id = ?1 AND project_id = ?2 AND hit_count >= ?3 AND slpr_score >= ?4
         ORDER BY last_seen_at DESC, id DESC"
    } else {
        "SELECT id, tenant_id, project_id, rule_id, fingerprint_sha256, sample_message,
                hit_count, slpr_score, promoted_card_id, status, last_seen_at
         FROM gate_failure_ledger
         WHERE tenant_id = ?1 AND project_id = ?2
         ORDER BY last_seen_at DESC, id DESC"
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| LearnFilterError::Storage(e.to_string()))?;

    let mapper = |r: &rusqlite::Row<'_>| -> rusqlite::Result<GateFailureLedgerEntry> {
        let hit_count: i64 = r.get(6)?;
        let slpr_score: f64 = r.get(7)?;
        let status_str: String = r.get(9)?;
        Ok(GateFailureLedgerEntry {
            id: r.get(0)?,
            tenant_id: r.get(1)?,
            project_id: r.get(2)?,
            rule_id: r.get(3)?,
            fingerprint_sha256: r.get(4)?,
            sample_message: r.get(5)?,
            hit_count,
            slpr_score,
            promoted_card_id: r.get(8)?,
            status: LedgerStatus::parse_str(&status_str),
            last_seen_at: r.get(10)?,
            qualifies_for_proposal: hit_count >= LEARN_MIN_HIT_COUNT
                && slpr_score >= LEARN_PROMOTE_SLPR_THRESHOLD,
        })
    };

    let rows = if qualifying_only {
        stmt.query_map(
            params![
                tenant_id,
                project_id,
                LEARN_MIN_HIT_COUNT,
                LEARN_PROMOTE_SLPR_THRESHOLD
            ],
            mapper,
        )
    } else {
        stmt.query_map(params![tenant_id, project_id], mapper)
    }
    .map_err(|e| LearnFilterError::Storage(e.to_string()))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| LearnFilterError::Storage(e.to_string()))?);
    }
    Ok(out)
}
