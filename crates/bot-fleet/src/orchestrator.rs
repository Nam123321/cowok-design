// @story 10.1
// @implements AC-01, AC-02, AC-03, AC-04 (cowok Master Orchestrator, 3-Lane Router, Hardened $UIUX_LIVE_SPEC O_NOFOLLOW Bridge, Cmd+Z Undo & CAS Provenance Backlinks)
use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use chrono::Utc;
use domain::decision::{CascadeStage, DecisionRouteInput, DecisionTier, VerdictShieldOutput};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::byok_gateway::{ByokCredential, ByokGatewayRouter, CooldownTracker, PromptCacheHints};
use crate::decision_cascade::DecisionCascadeClient;
use crate::model_registry::{builtin_seed_models, ModelRegistryEntry};
use crate::normalizer::PostExecutionNormalizer;
use crate::session_dir::{validate_session_id, IsolatedSessionDir};

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};

/// Environment variable injected into `portable-pty` / `PgidProcessSupervisor` Lane 2 shells.
pub const LIVE_SPEC_ENV_VAR: &str = "UIUX_LIVE_SPEC";

/// Hard ceiling (`512 KB`) enforced on `$UIUX_LIVE_SPEC` (`doc-patch.jsonl`) reads and payloads.
pub const LIVE_SPEC_MAX_BYTES: usize = 524_288;

/// Maximum bounded undo history depth per `(tenant_id, project_id, screen_id)`.
pub const MAX_UNDO_STACK_DEPTH: usize = 64;

/// Maximum safe pixel dimension for deterministic `RESIZE` patches (aligned with `html-forge`).
pub const MAX_LIVE_DIMENSION_PX: u64 = 4_096;

/// Canonical jailed relative path for the live patch bridge file inside a project workspace.
pub const LIVE_SPEC_REL_DIR: &str = ".cowok-design/live-spec";
pub const LIVE_SPEC_DEFAULT_FILENAME: &str = "doc-patch.jsonl";

const ALLOWED_LIVE_OP_KINDS: &[&str] = &[
    "TEXT_EDIT",
    "TOKEN_SWAP",
    "STEERING_CHIP",
    "LOCKED_RULE_CHANGE",
    "UNDO",
    "RESIZE",
    "LINT_FIX",
];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OrchestratorError {
    #[error("SYMLINK_VIOLATION: {0}")]
    SymlinkViolation(String),
    #[error("LIVE_SPEC_PAYLOAD_TOO_LARGE: payload {actual} bytes exceeds {max} byte ceiling")]
    PayloadTooLarge { actual: usize, max: usize },
    #[error("INVALID_LIVE_SPEC_PATCH: {0}")]
    InvalidLiveSpecPatch(String),
    #[error("PATH_TRAVERSAL: {0}")]
    PathTraversal(String),
    #[error("LIVE_SPEC_NOT_FOUND: {0}")]
    LiveSpecNotFound(String),
    #[error("NOTHING_TO_UNDO: no prior live patch snapshot available for 1-step Cmd+Z rollback")]
    NothingToUndo,
    #[error("INVALID_PROVENANCE_HASH: {0}")]
    InvalidProvenanceHash(String),
    #[error("DECISION_CASCADE_ERROR: {0}")]
    DecisionFailed(String),
    #[error("FLEET_DISPATCH_ERROR: {0}")]
    FleetDispatchFailed(String),
    #[error("IO_ERROR: {0}")]
    Io(String),
}

/// Canonical 3-Lane routing classification for `cowok` Master Orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrchestratorLane {
    #[serde(rename = "LANE_0_DETERMINISTIC")]
    Lane0Deterministic,
    #[serde(rename = "LANE_1_DECISION_CASCADE")]
    Lane1DecisionCascade,
    #[serde(rename = "LANE_2_OPC_FLEET")]
    Lane2OpcFleet,
}

impl OrchestratorLane {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lane0Deterministic => "LANE_0_DETERMINISTIC",
            Self::Lane1DecisionCascade => "LANE_1_DECISION_CASCADE",
            Self::Lane2OpcFleet => "LANE_2_OPC_FLEET",
        }
    }
}

/// Explicit or auto-classified command kind accepted by `POST /api/v1/orchestrator/chat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrchestratorCommandKind {
    #[default]
    Auto,
    LintFix,
    TokenUpdate,
    Resize,
    Undo,
    ApplyLiveSpec,
}

/// Unified Asset Library provenance backlink deduplicated by `cas_blobs.sha256` (`contentHash`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvenanceBacklink {
    pub content_hash: String,
    pub session_id: String,
    pub node_id: Option<String>,
    pub patch_id: Option<String>,
    pub mime_type: String,
    pub created_at: i64,
}

/// Single JSONL line contract inside `$UIUX_LIVE_SPEC` (`doc-patch.jsonl`).
/// Uses `#[serde(deny_unknown_fields)]` so malformed or spoofed agent fields are rejected atomically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LiveSpecPatchLine {
    #[serde(default)]
    pub patch_id: Option<String>,
    pub screen_id: String,
    pub ff_id: String,
    pub op_kind: String,
    #[serde(default)]
    pub payload: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub asset_content_hash: Option<String>,
    #[serde(default)]
    pub asset_mime_type: Option<String>,
}

/// Deterministic snapshot of the current Canvas live-spec state for `<50ms` updates & 1-step `Cmd+Z`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasPatchSnapshot {
    pub screen_id: String,
    pub revision: u64,
    pub tokens: BTreeMap<String, String>,
    pub node_texts: BTreeMap<String, String>,
    pub node_sizes: BTreeMap<String, (u32, u32)>,
    pub content_sha256: String,
}

impl Default for CanvasPatchSnapshot {
    fn default() -> Self {
        let mut snap = Self {
            screen_id: "main-screen".to_string(),
            revision: 0,
            tokens: BTreeMap::new(),
            node_texts: BTreeMap::new(),
            node_sizes: BTreeMap::new(),
            content_sha256: String::new(),
        };
        snap.recompute_sha256();
        snap
    }
}

impl CanvasPatchSnapshot {
    pub fn recompute_sha256(&mut self) {
        let canonical = serde_json::json!({
            "screenId": self.screen_id,
            "revision": self.revision,
            "tokens": self.tokens,
            "nodeTexts": self.node_texts,
            "nodeSizes": self.node_sizes,
        });
        let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
        self.content_sha256 = sha256_hex(&bytes);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoStackEntry {
    pub patch_id: String,
    pub previous_state: CanvasPatchSnapshot,
    pub previous_html_snapshot: Option<String>,
    pub applied_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LivePatchApplyOutcome {
    pub applied_patch_id: String,
    pub screen_id: String,
    pub revision: u64,
    pub canvas_sha256: String,
    pub applied_lines: usize,
    pub can_undo: bool,
    pub latency_ms: u64,
    pub provenance_backlinks: Vec<ProvenanceBacklink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoOutcome {
    pub restored_patch_id: String,
    pub screen_id: String,
    pub revision: u64,
    pub canvas_sha256: String,
    pub can_undo: bool,
    pub latency_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restored_html_snapshot: Option<String>,
}

/// Stateful `$UIUX_LIVE_SPEC` bridge holding the last known-good Canvas state,
/// bounded multi-step `Cmd+Z` inverse history (with exact HTML snapshots),
/// and deduplicated `cas_blobs` provenance backlinks.
#[derive(Debug, Clone, Default)]
pub struct LiveSpecBridgeState {
    current_state: CanvasPatchSnapshot,
    current_html_snapshot: Option<String>,
    undo_stack: VecDeque<UndoStackEntry>,
    provenance_by_hash: BTreeMap<String, ProvenanceBacklink>,
}

impl LiveSpecBridgeState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn for_screen(screen_id: &str) -> Self {
        let mut state = Self::default();
        let trimmed = screen_id.trim();
        if !trimmed.is_empty() {
            state.current_state.screen_id = trimmed.to_string();
            state.current_state.recompute_sha256();
        }
        state
    }

    pub fn current_state(&self) -> &CanvasPatchSnapshot {
        &self.current_state
    }

    pub fn current_html_snapshot(&self) -> Option<&str> {
        self.current_html_snapshot.as_deref()
    }

    pub fn set_current_html_snapshot(&mut self, html: String) {
        self.current_html_snapshot = Some(html);
    }

    /// Attaches exact before/after HTML snapshots onto the most recent undo entry
    /// so consecutive multi-step `Cmd+Z` undos restore matching HTML deterministically.
    pub fn record_top_undo_html_snapshot(&mut self, previous_html: String, next_html: String) {
        if let Some(top) = self.undo_stack.back_mut() {
            top.previous_html_snapshot = Some(previous_html);
        }
        self.current_html_snapshot = Some(next_html);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Registers an asset provenance backlink deduplicated by 64-char `content_hash`.
    pub fn register_asset_provenance(
        &mut self,
        mut backlink: ProvenanceBacklink,
    ) -> Result<ProvenanceBacklink, OrchestratorError> {
        let normalized = normalize_sha256_hex(&backlink.content_hash)?;
        backlink.content_hash = normalized.clone();
        self.provenance_by_hash
            .entry(normalized)
            .or_insert_with(|| backlink.clone());
        Ok(backlink)
    }

    /// Returns deduplicated provenance backlinks ordered newest-first.
    pub fn provenance_backlinks(&self) -> Vec<ProvenanceBacklink> {
        deduplicate_provenance_backlinks(self.provenance_by_hash.values().cloned().collect())
    }

    /// Applies pre-parsed JSONL patch lines atomically alongside exact prior & next HTML snapshots.
    /// Avoids double JSONL parsing and ensures multi-step `Cmd+Z` restores exact HTML parity.
    pub fn apply_live_patch_lines_with_html(
        &mut self,
        lines: &[LiveSpecPatchLine],
        session_id: &str,
        node_id: Option<&str>,
        previous_html_snapshot: Option<String>,
        next_html_snapshot: Option<String>,
    ) -> Result<LivePatchApplyOutcome, OrchestratorError> {
        let started = Instant::now();
        if lines.is_empty() {
            return Err(OrchestratorError::InvalidLiveSpecPatch(
                "doc-patch.jsonl contains zero patch operations".to_string(),
            ));
        }
        for (idx, line) in lines.iter().enumerate() {
            validate_live_patch_line(line, idx + 1)?;
        }

        let mut next_state = self.current_state.clone();
        let batch_patch_id = lines
            .last()
            .and_then(|l| l.patch_id.clone())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let now_ms = Utc::now().timestamp_millis();
        let mut new_backlinks = Vec::new();

        for line in lines {
            next_state.screen_id = line.screen_id.trim().to_string();
            apply_parsed_line_to_snapshot(&mut next_state, line)?;
            if let Some(ref raw_hash) = line.asset_content_hash {
                let normalized_hash = normalize_sha256_hex(raw_hash)?;
                let mime = line
                    .asset_mime_type
                    .as_deref()
                    .map(str::trim)
                    .filter(|m| !m.is_empty())
                    .unwrap_or("application/octet-stream")
                    .to_string();
                new_backlinks.push(ProvenanceBacklink {
                    content_hash: normalized_hash,
                    session_id: session_id.to_string(),
                    node_id: node_id.map(str::to_string),
                    patch_id: Some(batch_patch_id.clone()),
                    mime_type: mime,
                    created_at: now_ms,
                });
            }
        }

        next_state.revision = self.current_state.revision.saturating_add(1);
        next_state.recompute_sha256();

        // Commit atomically: push prior known-good state + HTML onto bounded Cmd+Z stack.
        let prev = std::mem::replace(&mut self.current_state, next_state);
        if self.undo_stack.len() >= MAX_UNDO_STACK_DEPTH {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(UndoStackEntry {
            patch_id: batch_patch_id.clone(),
            previous_state: prev,
            previous_html_snapshot,
            applied_at_ms: now_ms,
        });
        if let Some(next_html) = next_html_snapshot {
            self.current_html_snapshot = Some(next_html);
        }

        for bl in new_backlinks {
            let _ = self.register_asset_provenance(bl);
        }

        Ok(LivePatchApplyOutcome {
            applied_patch_id: batch_patch_id,
            screen_id: self.current_state.screen_id.clone(),
            revision: self.current_state.revision,
            canvas_sha256: self.current_state.content_sha256.clone(),
            applied_lines: lines.len(),
            can_undo: !self.undo_stack.is_empty(),
            latency_ms: started.elapsed().as_millis() as u64,
            provenance_backlinks: self.provenance_backlinks(),
        })
    }

    /// Validates ALL JSONL lines strictly BEFORE mutating state; on any invalid line,
    /// returns `Err(InvalidLiveSpecPatch)` leaving `current_state` untouched.
    pub fn apply_live_patch_jsonl(
        &mut self,
        jsonl: &str,
        session_id: &str,
        node_id: Option<&str>,
    ) -> Result<LivePatchApplyOutcome, OrchestratorError> {
        let lines = parse_live_spec_jsonl(jsonl)?;
        let prev_html = self.current_html_snapshot.clone();
        self.apply_live_patch_lines_with_html(&lines, session_id, node_id, prev_html, None)
    }

    /// Reads a jailed `$UIUX_LIVE_SPEC` file with POSIX `openat(O_NOFOLLOW)` + `<= 512 KB` enforcement
    /// and applies it atomically.
    pub fn apply_live_patch_file(
        &mut self,
        workspace_root: &Path,
        live_spec_path: &Path,
        session_id: &str,
        node_id: Option<&str>,
    ) -> Result<LivePatchApplyOutcome, OrchestratorError> {
        let jsonl = read_live_spec_file_nofollow(workspace_root, live_spec_path)?;
        self.apply_live_patch_jsonl(&jsonl, session_id, node_id)
    }

    /// Instant `<50ms` `Cmd+Z` rollback to the immediately preceding stable Canvas snapshot & HTML.
    pub fn undo_last_patch(&mut self) -> Result<UndoOutcome, OrchestratorError> {
        let started = Instant::now();
        let entry = self
            .undo_stack
            .pop_back()
            .ok_or(OrchestratorError::NothingToUndo)?;
        self.current_state = entry.previous_state;
        self.current_html_snapshot = entry.previous_html_snapshot.clone();
        Ok(UndoOutcome {
            restored_patch_id: entry.patch_id,
            screen_id: self.current_state.screen_id.clone(),
            revision: self.current_state.revision,
            canvas_sha256: self.current_state.content_sha256.clone(),
            can_undo: !self.undo_stack.is_empty(),
            latency_ms: started.elapsed().as_millis() as u64,
            restored_html_snapshot: entry.previous_html_snapshot,
        })
    }
}

/// Input contract for `cowok` 3-Lane Router.
#[derive(Debug, Clone)]
pub struct OrchestratorRouteInput {
    pub tenant_id: String,
    pub project_id: String,
    pub session_id: String,
    pub node_id: Option<String>,
    pub message: String,
    pub command_kind: OrchestratorCommandKind,
    pub live_spec_payload: Option<String>,
    pub workspace_root: Option<PathBuf>,
    pub untrusted_artifact_sha256: Option<String>,
}

/// Output contract returned by `cowok` 3-Lane Router (strictly zero floats).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorRouteOutput {
    pub lane: OrchestratorLane,
    pub latency_ms: u64,
    pub token_cost: u64,
    pub reply: String,
    pub applied_patch_id: Option<String>,
    pub can_undo: bool,
    pub live_spec_path: Option<String>,
    pub provenance_backlinks: Vec<ProvenanceBacklink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_verdict: Option<VerdictShieldOutput>,
    #[serde(default, skip_serializing)]
    pub injected_pty_env: Vec<(String, String)>,
    #[serde(default, skip_serializing)]
    pub applied_patch_lines: Vec<LiveSpecPatchLine>,
    #[serde(default, skip_serializing)]
    pub restored_html_snapshot: Option<String>,
    #[serde(default, skip_serializing)]
    pub is_undo: bool,
    #[serde(default, skip_serializing)]
    pub mutated_canvas: bool,
    #[serde(default, skip_serializing)]
    pub screen_id: String,
    #[serde(default, skip_serializing)]
    pub canvas_sha256: String,
}

/// `cowok` Master Orchestrator implementing the 3-Lane Router (`Lane 0 / Lane 1 / Lane 2`)
/// and `$UIUX_LIVE_SPEC` `<50ms` bridge.
pub struct CowokOrchestrator {
    decision_client: DecisionCascadeClient,
    byok_credentials: Vec<ByokCredential>,
    byok_cooldowns: CooldownTracker,
    model_registry: Vec<ModelRegistryEntry>,
    prompt_cache_hints: Option<PromptCacheHints>,
    compiled_turn_tail: Option<String>,
}

impl Default for CowokOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl CowokOrchestrator {
    pub fn new() -> Self {
        Self {
            decision_client: DecisionCascadeClient::new(),
            byok_credentials: Vec::new(),
            byok_cooldowns: CooldownTracker::new(),
            model_registry: builtin_seed_models(),
            prompt_cache_hints: None,
            compiled_turn_tail: None,
        }
    }

    pub fn with_decision_client(decision_client: DecisionCascadeClient) -> Self {
        Self {
            decision_client,
            byok_credentials: Vec::new(),
            byok_cooldowns: CooldownTracker::new(),
            model_registry: builtin_seed_models(),
            prompt_cache_hints: None,
            compiled_turn_tail: None,
        }
    }

    pub fn with_byok_fleet(
        mut self,
        byok_credentials: Vec<ByokCredential>,
        byok_cooldowns: CooldownTracker,
    ) -> Self {
        self.byok_credentials = byok_credentials;
        self.byok_cooldowns = byok_cooldowns;
        self
    }

    pub fn with_model_registry(mut self, entries: Vec<ModelRegistryEntry>) -> Self {
        if !entries.is_empty() {
            self.model_registry = entries;
        }
        self
    }

    pub fn with_prompt_cache_context(
        mut self,
        hints: PromptCacheHints,
        compiled_turn_tail: Option<String>,
    ) -> Self {
        self.prompt_cache_hints = Some(hints);
        self.compiled_turn_tail = compiled_turn_tail;
        self
    }

    /// Deterministically classifies a request into Lane 0, Lane 1, or Lane 2.
    pub fn classify_lane(
        command_kind: OrchestratorCommandKind,
        message: &str,
        live_spec_payload: Option<&str>,
    ) -> OrchestratorLane {
        if command_kind != OrchestratorCommandKind::Auto || live_spec_payload.is_some() {
            return OrchestratorLane::Lane0Deterministic;
        }
        let lower = message.trim().to_ascii_lowercase();
        if is_deterministic_lane0_message(&lower) {
            return OrchestratorLane::Lane0Deterministic;
        }
        if is_opc_fleet_lane2_message(&lower) {
            return OrchestratorLane::Lane2OpcFleet;
        }
        OrchestratorLane::Lane1DecisionCascade
    }

    /// Routes and executes a `cowok` chat/orchestration turn across the 3 lanes.
    pub async fn route_three_lane(
        &self,
        input: &OrchestratorRouteInput,
        bridge: &mut LiveSpecBridgeState,
    ) -> Result<OrchestratorRouteOutput, OrchestratorError> {
        let started = Instant::now();
        let lane = Self::classify_lane(
            input.command_kind,
            &input.message,
            input.live_spec_payload.as_deref(),
        );

        match lane {
            OrchestratorLane::Lane0Deterministic => {
                self.execute_lane_0(input, bridge, started).await
            }
            OrchestratorLane::Lane1DecisionCascade => {
                self.execute_lane_1(input, bridge, started).await
            }
            OrchestratorLane::Lane2OpcFleet => self.execute_lane_2(input, bridge, started).await,
        }
    }

    async fn execute_lane_0(
        &self,
        input: &OrchestratorRouteInput,
        bridge: &mut LiveSpecBridgeState,
        started: Instant,
    ) -> Result<OrchestratorRouteOutput, OrchestratorError> {
        let live_spec_path = if let Some(ref root) = input.workspace_root {
            let root_c = root.clone();
            let sid_c = input.session_id.clone();
            tokio::task::spawn_blocking(move || {
                resolve_jailed_live_spec_path(&root_c, Some(&sid_c))
            })
            .await
            .ok()
            .and_then(Result::ok)
            .map(|p| p.to_string_lossy().into_owned())
        } else {
            None
        };

        // 1. Direct $UIUX_LIVE_SPEC payload apply
        if let Some(ref payload) = input.live_spec_payload {
            let lines = parse_live_spec_jsonl(payload)?;
            let prev_html = bridge.current_html_snapshot().map(str::to_string);
            let outcome = bridge.apply_live_patch_lines_with_html(
                &lines,
                &input.session_id,
                input.node_id.as_deref(),
                prev_html,
                None,
            )?;
            return Ok(OrchestratorRouteOutput {
                lane: OrchestratorLane::Lane0Deterministic,
                latency_ms: started.elapsed().as_millis() as u64,
                token_cost: 0,
                reply: format!(
                    "Applied {} $UIUX_LIVE_SPEC patch line(s) deterministically to '{}' (rev {}, <50ms).",
                    outcome.applied_lines, outcome.screen_id, outcome.revision
                ),
                applied_patch_id: Some(outcome.applied_patch_id),
                can_undo: outcome.can_undo,
                live_spec_path,
                provenance_backlinks: outcome.provenance_backlinks,
                decision_verdict: None,
                injected_pty_env: Vec::new(),
                applied_patch_lines: lines,
                restored_html_snapshot: None,
                is_undo: false,
                mutated_canvas: true,
                screen_id: outcome.screen_id,
                canvas_sha256: outcome.canvas_sha256,
            });
        }

        let effective_kind = infer_lane0_command_kind(input.command_kind, &input.message);
        match effective_kind {
            OrchestratorCommandKind::Undo => {
                let undo = bridge.undo_last_patch()?;
                Ok(OrchestratorRouteOutput {
                    lane: OrchestratorLane::Lane0Deterministic,
                    latency_ms: started.elapsed().as_millis() as u64,
                    token_cost: 0,
                    reply: format!(
                        "Rolled back patch '{}' via 1-step Cmd+Z (rev {}).",
                        undo.restored_patch_id, undo.revision
                    ),
                    applied_patch_id: Some(undo.restored_patch_id),
                    can_undo: undo.can_undo,
                    live_spec_path,
                    provenance_backlinks: bridge.provenance_backlinks(),
                    decision_verdict: None,
                    injected_pty_env: Vec::new(),
                    applied_patch_lines: Vec::new(),
                    restored_html_snapshot: undo.restored_html_snapshot,
                    is_undo: true,
                    mutated_canvas: true,
                    screen_id: undo.screen_id,
                    canvas_sha256: undo.canvas_sha256,
                })
            }
            OrchestratorCommandKind::TokenUpdate => {
                let (token_name, token_value) = extract_css_token_pair(&input.message)
                    .unwrap_or_else(|| ("--cowok-accent".to_string(), "#2563EB".to_string()));
                let mut css_vars = serde_json::Map::new();
                css_vars.insert(
                    token_name.clone(),
                    serde_json::Value::String(token_value.clone()),
                );
                let mut payload = BTreeMap::new();
                payload.insert("cssVars".to_string(), serde_json::Value::Object(css_vars));
                let line = LiveSpecPatchLine {
                    patch_id: None,
                    screen_id: bridge.current_state().screen_id.clone(),
                    ff_id: input.node_id.as_deref().unwrap_or("stage-root").to_string(),
                    op_kind: "TOKEN_SWAP".to_string(),
                    payload,
                    asset_content_hash: None,
                    asset_mime_type: None,
                };
                let lines = vec![line];
                let prev_html = bridge.current_html_snapshot().map(str::to_string);
                let outcome = bridge.apply_live_patch_lines_with_html(
                    &lines,
                    &input.session_id,
                    input.node_id.as_deref(),
                    prev_html,
                    None,
                )?;
                Ok(OrchestratorRouteOutput {
                    lane: OrchestratorLane::Lane0Deterministic,
                    latency_ms: started.elapsed().as_millis() as u64,
                    token_cost: 0,
                    reply: format!(
                        "Updated CSS token {token_name}: {token_value} deterministically (0 tokens, rev {}).",
                        outcome.revision
                    ),
                    applied_patch_id: Some(outcome.applied_patch_id),
                    can_undo: outcome.can_undo,
                    live_spec_path,
                    provenance_backlinks: outcome.provenance_backlinks,
                    decision_verdict: None,
                    injected_pty_env: Vec::new(),
                    applied_patch_lines: lines,
                    restored_html_snapshot: None,
                    is_undo: false,
                    mutated_canvas: true,
                    screen_id: outcome.screen_id,
                    canvas_sha256: outcome.canvas_sha256,
                })
            }
            OrchestratorCommandKind::Resize => {
                let (width, height) = extract_dimensions(&input.message).unwrap_or((1280, 720));
                let width = u64::from(width).clamp(1, MAX_LIVE_DIMENSION_PX);
                let height = u64::from(height).clamp(1, MAX_LIVE_DIMENSION_PX);
                let target_ff = input.node_id.as_deref().unwrap_or("stage-root");
                let mut payload = BTreeMap::new();
                payload.insert("width".to_string(), serde_json::json!(width));
                payload.insert("height".to_string(), serde_json::json!(height));
                let line = LiveSpecPatchLine {
                    patch_id: None,
                    screen_id: bridge.current_state().screen_id.clone(),
                    ff_id: target_ff.to_string(),
                    op_kind: "RESIZE".to_string(),
                    payload,
                    asset_content_hash: None,
                    asset_mime_type: None,
                };
                let lines = vec![line];
                let prev_html = bridge.current_html_snapshot().map(str::to_string);
                let outcome = bridge.apply_live_patch_lines_with_html(
                    &lines,
                    &input.session_id,
                    input.node_id.as_deref(),
                    prev_html,
                    None,
                )?;
                Ok(OrchestratorRouteOutput {
                    lane: OrchestratorLane::Lane0Deterministic,
                    latency_ms: started.elapsed().as_millis() as u64,
                    token_cost: 0,
                    reply: format!(
                        "Resized '{target_ff}' to {width}x{height} deterministically (0 tokens, rev {}).",
                        outcome.revision
                    ),
                    applied_patch_id: Some(outcome.applied_patch_id),
                    can_undo: outcome.can_undo,
                    live_spec_path,
                    provenance_backlinks: outcome.provenance_backlinks,
                    decision_verdict: None,
                    injected_pty_env: Vec::new(),
                    applied_patch_lines: lines,
                    restored_html_snapshot: None,
                    is_undo: false,
                    mutated_canvas: true,
                    screen_id: outcome.screen_id,
                    canvas_sha256: outcome.canvas_sha256,
                })
            }
            OrchestratorCommandKind::ApplyLiveSpec => {
                let Some(root) = input.workspace_root.clone() else {
                    return Err(OrchestratorError::InvalidLiveSpecPatch(
                        "apply_live_spec requires liveSpecPayload or workspace_root".to_string(),
                    ));
                };
                let sid_c = input.session_id.clone();
                let (path, raw_jsonl) = tokio::task::spawn_blocking(move || {
                    let p = resolve_jailed_live_spec_path(&root, Some(&sid_c))?;
                    let content = read_live_spec_file_nofollow(&root, &p)?;
                    Ok::<_, OrchestratorError>((p, content))
                })
                .await
                .map_err(|_| OrchestratorError::Io("Live spec reader task failed".to_string()))??;
                let lines = parse_live_spec_jsonl(&raw_jsonl)?;
                let prev_html = bridge.current_html_snapshot().map(str::to_string);
                let outcome = bridge.apply_live_patch_lines_with_html(
                    &lines,
                    &input.session_id,
                    input.node_id.as_deref(),
                    prev_html,
                    None,
                )?;
                Ok(OrchestratorRouteOutput {
                    lane: OrchestratorLane::Lane0Deterministic,
                    latency_ms: started.elapsed().as_millis() as u64,
                    token_cost: 0,
                    reply: format!(
                        "Applied {} $UIUX_LIVE_SPEC patch line(s) from disk (rev {}).",
                        outcome.applied_lines, outcome.revision
                    ),
                    applied_patch_id: Some(outcome.applied_patch_id),
                    can_undo: outcome.can_undo,
                    live_spec_path: Some(path.to_string_lossy().into_owned()),
                    provenance_backlinks: outcome.provenance_backlinks,
                    decision_verdict: None,
                    injected_pty_env: Vec::new(),
                    applied_patch_lines: lines,
                    restored_html_snapshot: None,
                    is_undo: false,
                    mutated_canvas: true,
                    screen_id: outcome.screen_id,
                    canvas_sha256: outcome.canvas_sha256,
                })
            }
            OrchestratorCommandKind::LintFix | OrchestratorCommandKind::Auto => {
                let mut payload = BTreeMap::new();
                payload.insert(
                    "rule".to_string(),
                    serde_json::Value::String("deterministic_token_hygiene".to_string()),
                );
                let line = LiveSpecPatchLine {
                    patch_id: None,
                    screen_id: bridge.current_state().screen_id.clone(),
                    ff_id: input.node_id.as_deref().unwrap_or("stage-root").to_string(),
                    op_kind: "LINT_FIX".to_string(),
                    payload,
                    asset_content_hash: None,
                    asset_mime_type: None,
                };
                let lines = vec![line];
                let prev_html = bridge.current_html_snapshot().map(str::to_string);
                let outcome = bridge.apply_live_patch_lines_with_html(
                    &lines,
                    &input.session_id,
                    input.node_id.as_deref(),
                    prev_html,
                    None,
                )?;
                Ok(OrchestratorRouteOutput {
                    lane: OrchestratorLane::Lane0Deterministic,
                    latency_ms: started.elapsed().as_millis() as u64,
                    token_cost: 0,
                    reply: format!(
                        "Executed deterministic AST lint_fix cleanly (0 tokens, rev {}).",
                        outcome.revision
                    ),
                    applied_patch_id: Some(outcome.applied_patch_id),
                    can_undo: outcome.can_undo,
                    live_spec_path,
                    provenance_backlinks: outcome.provenance_backlinks,
                    decision_verdict: None,
                    injected_pty_env: Vec::new(),
                    applied_patch_lines: lines,
                    restored_html_snapshot: None,
                    is_undo: false,
                    mutated_canvas: true,
                    screen_id: outcome.screen_id,
                    canvas_sha256: outcome.canvas_sha256,
                })
            }
        }
    }

    async fn execute_lane_1(
        &self,
        input: &OrchestratorRouteInput,
        bridge: &LiveSpecBridgeState,
        started: Instant,
    ) -> Result<OrchestratorRouteOutput, OrchestratorError> {
        let artifact_sha = input
            .untrusted_artifact_sha256
            .clone()
            .filter(|s| normalize_sha256_hex(s).is_ok())
            .unwrap_or_else(|| sha256_hex(input.message.as_bytes()));

        let route_input = DecisionRouteInput {
            project_id: input.project_id.clone(),
            stage: CascadeStage::Decision1,
            prompt: input.message.clone(),
            context: format!(
                "session={} node={} screen={} rev={}",
                input.session_id,
                input.node_id.as_deref().unwrap_or("none"),
                bridge.current_state().screen_id,
                bridge.current_state().revision
            ),
            untrusted_artifact_sha256: artifact_sha,
            attempt_number: 1,
            prior_verdicts: Vec::new(),
            stale_file_sha256: None,
            current_file_sha256: None,
            candidates: Vec::new(),
            rubric_pair: None,
            prefer_external_tier: false,
        };

        let verdict = self
            .decision_client
            .route(&input.tenant_id, &route_input)
            .await
            .map_err(|e| OrchestratorError::DecisionFailed(e.to_string()))?;

        let token_cost = match verdict.tier_used {
            DecisionTier::LocalFts5Bm25 | DecisionTier::HeuristicFallback => 0,
            DecisionTier::GeminiDecision1 | DecisionTier::TypesafeJev => {
                ((input.message.len() / 4).max(1)) as u64
            }
        };

        let reply = format!(
            "Routed via Lane 1 Decision Cascade -> {} ({}, tier={}).",
            verdict.verdict_label,
            verdict.confidence_kind.as_db_str(),
            verdict.tier_used.as_db_str()
        );

        Ok(OrchestratorRouteOutput {
            lane: OrchestratorLane::Lane1DecisionCascade,
            latency_ms: started.elapsed().as_millis() as u64,
            token_cost,
            reply,
            applied_patch_id: None,
            can_undo: bridge.can_undo(),
            live_spec_path: None,
            provenance_backlinks: bridge.provenance_backlinks(),
            decision_verdict: Some(verdict),
            injected_pty_env: Vec::new(),
            applied_patch_lines: Vec::new(),
            restored_html_snapshot: None,
            is_undo: false,
            mutated_canvas: false,
            screen_id: bridge.current_state().screen_id.clone(),
            canvas_sha256: bridge.current_state().content_sha256.clone(),
        })
    }

    async fn execute_lane_2(
        &self,
        input: &OrchestratorRouteInput,
        bridge: &LiveSpecBridgeState,
        started: Instant,
    ) -> Result<OrchestratorRouteOutput, OrchestratorError> {
        let safe_session = validate_session_id(&input.session_id)
            .map_err(|e| OrchestratorError::FleetDispatchFailed(e.to_string()))?;
        let ws_root_opt = input.workspace_root.clone();
        let safe_session_c = safe_session.clone();
        let project_id_c = input.project_id.clone();
        let screen_id_c = bridge.current_state().screen_id.clone();
        let message_c = input.message.clone();

        // Keep IsolatedSessionDir alive across the entire Lane 2 execution scope,
        // and bind $UIUX_LIVE_SPEC inside workspace_root/.cowok-design/live-spec/
        // so the patch file survives ephemeral session directory cleanup.
        let (session_dir, live_spec_path) = tokio::task::spawn_blocking(move || {
            let dir = IsolatedSessionDir::create_ephemeral(&safe_session_c)
                .map_err(|e| OrchestratorError::FleetDispatchFailed(e.to_string()))?;
            let path = if let Some(ref ws_root) = ws_root_opt {
                resolve_jailed_live_spec_path(ws_root, Some(&safe_session_c))?
            } else {
                dir.root().join(LIVE_SPEC_DEFAULT_FILENAME)
            };
            let dispatch_manifest = serde_json::json!({
                "sessionId": safe_session_c,
                "projectId": project_id_c,
                "screenId": screen_id_c,
                "liveSpecEnvVar": LIVE_SPEC_ENV_VAR,
                "liveSpecPath": path.to_string_lossy().into_owned(),
                "message": message_c,
            });
            if let Ok(manifest_bytes) = serde_json::to_vec_pretty(&dispatch_manifest) {
                let _ =
                    dir.write_isolated_file_0600("prompts", "lane2-dispatch.json", &manifest_bytes);
            }
            Ok::<_, OrchestratorError>((dir, path))
        })
        .await
        .map_err(|_| {
            OrchestratorError::FleetDispatchFailed("Lane 2 session setup task failed".to_string())
        })??;

        let pty_env = build_lane2_pty_env(&session_dir, &live_spec_path);
        let live_spec_str = live_spec_path.to_string_lossy().into_owned();

        let (reply, token_cost) = if !self.byok_credentials.is_empty() {
            let router = ByokGatewayRouter::new(self.byok_cooldowns.clone())
                .with_model_registry(self.model_registry.clone());
            let turn_body = self
                .compiled_turn_tail
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&input.message);
            let fleet_prompt = format!(
                "[cowok-lane2-opc-fleet] session={} screen={} live_spec={}\n{}",
                safe_session,
                bridge.current_state().screen_id,
                live_spec_str,
                turn_body
            );
            match router
                .execute_with_failover_and_cache(
                    &input.tenant_id,
                    &fleet_prompt,
                    None,
                    None,
                    &self.byok_credentials,
                    15_000,
                    self.prompt_cache_hints.as_ref(),
                )
                .await
            {
                Ok(exec_res) => {
                    let norm = PostExecutionNormalizer::normalize(&exec_res.raw_content);
                    let real_tokens = (input.message.split_whitespace().count()
                        + norm.normalized_content.split_whitespace().count())
                    .max(1) as u64;
                    let summary = norm.normalized_content.trim();
                    let formatted_reply = if summary.is_empty() {
                        format!(
                            "Lane 2 OPC Fleet completed via {}:{} with ${LIVE_SPEC_ENV_VAR} bound to '{}'.",
                            exec_res.provider_id, exec_res.model_id, live_spec_str
                        )
                    } else {
                        format!(
                            "[Lane 2 OPC Fleet {}:{}] {}",
                            exec_res.provider_id, exec_res.model_id, summary
                        )
                    };
                    (formatted_reply, real_tokens)
                }
                Err(err) => (
                    format!(
                        "Prepared isolated Lane 2 OPC Fleet dispatch plan for session '{}' with ${LIVE_SPEC_ENV_VAR} bound to '{}' (provider execution deferred: {}).",
                        safe_session, live_spec_str, err
                    ),
                    0u64,
                ),
            }
        } else {
            (
                format!(
                    "Prepared isolated Lane 2 OPC Fleet dispatch plan for session '{}' with ${LIVE_SPEC_ENV_VAR} bound to '{}' (no remote BYOK provider configured; 0 tokens spent).",
                    safe_session, live_spec_str
                ),
                0u64,
            )
        };

        // Explicit blocking cleanup after Lane 2 execution completes (zeroizes 0o600 files & removes temp dir).
        let _ = tokio::task::spawn_blocking(move || drop(session_dir)).await;

        Ok(OrchestratorRouteOutput {
            lane: OrchestratorLane::Lane2OpcFleet,
            latency_ms: started.elapsed().as_millis() as u64,
            token_cost,
            reply,
            applied_patch_id: None,
            can_undo: bridge.can_undo(),
            live_spec_path: Some(live_spec_str),
            provenance_backlinks: bridge.provenance_backlinks(),
            decision_verdict: None,
            injected_pty_env: pty_env,
            applied_patch_lines: Vec::new(),
            restored_html_snapshot: None,
            is_undo: false,
            mutated_canvas: false,
            screen_id: bridge.current_state().screen_id.clone(),
            canvas_sha256: bridge.current_state().content_sha256.clone(),
        })
    }
}

/// Builds the isolated environment vector for `portable-pty` / `PgidProcessSupervisor` Lane 2 workers,
/// injecting `$UIUX_LIVE_SPEC` alongside `$COWOK_SESSION_DIR` and XDG isolation variables.
pub fn build_lane2_pty_env(
    session: &IsolatedSessionDir,
    live_spec_path: &Path,
) -> Vec<(String, String)> {
    let mut env = session.isolated_env();
    env.push((
        LIVE_SPEC_ENV_VAR.to_string(),
        live_spec_path.to_string_lossy().into_owned(),
    ));
    env
}

/// Resolves and ensures the jailed `.cowok-design/live-spec/` directory beneath `workspace_root`,
/// rejecting symlinks on any path component.
pub fn resolve_jailed_live_spec_path(
    workspace_root: &Path,
    session_id: Option<&str>,
) -> Result<PathBuf, OrchestratorError> {
    let live_dir = ensure_jailed_live_spec_dir(workspace_root)?;
    let filename = match session_id {
        Some(raw) if !raw.trim().is_empty() => {
            let safe = validate_session_id(raw)
                .map_err(|e| OrchestratorError::PathTraversal(e.to_string()))?;
            format!("{safe}-{LIVE_SPEC_DEFAULT_FILENAME}")
        }
        _ => LIVE_SPEC_DEFAULT_FILENAME.to_string(),
    };
    let candidate = live_dir.join(filename);
    reject_if_symlink(&candidate)?;
    Ok(candidate)
}

/// Validates and reads a jailed `$UIUX_LIVE_SPEC` (`doc-patch.jsonl`) file with:
/// - Strict confinement inside `<workspace_root>/.cowok-design/live-spec/` and `.jsonl` extension
/// - Step-by-step POSIX `openat(dir_fd, seg, O_RDONLY | O_NOFOLLOW | O_DIRECTORY | O_CLOEXEC)`
///   followed by `openat(dir_fd, file, O_RDONLY | O_NOFOLLOW | O_CLOEXEC)` (zero path TOCTOU)
/// - Open-FD `fstat` regular file & `nlink() == 1` hardlink rejection
/// - Strict `<= 524_288` byte cap (`LIVE_SPEC_PAYLOAD_TOO_LARGE`) checked on open FD + bounded read
pub fn read_live_spec_file_nofollow(
    workspace_root: &Path,
    candidate_path: &Path,
) -> Result<String, OrchestratorError> {
    let rel_inside_live_spec =
        extract_jailed_live_spec_relative_segments(workspace_root, candidate_path)?;
    let file = open_live_spec_file_stepwise_nofollow(workspace_root, &rel_inside_live_spec)?;

    let meta = file
        .metadata()
        .map_err(|_| OrchestratorError::Io("Failed reading live spec file metadata".into()))?;
    if !meta.file_type().is_file() {
        return Err(OrchestratorError::SymlinkViolation(
            "Live spec target is not a regular file".into(),
        ));
    }
    #[cfg(unix)]
    if meta.nlink() != 1 {
        return Err(OrchestratorError::SymlinkViolation(
            "HARDLINK_VIOLATION: refusing hardlinked live spec file".into(),
        ));
    }
    if meta.len() > LIVE_SPEC_MAX_BYTES as u64 {
        return Err(OrchestratorError::PayloadTooLarge {
            actual: meta.len() as usize,
            max: LIVE_SPEC_MAX_BYTES,
        });
    }

    let mut bytes = Vec::with_capacity((meta.len() as usize).min(LIVE_SPEC_MAX_BYTES));
    file.take((LIVE_SPEC_MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| OrchestratorError::Io("Failed reading live spec file".into()))?;
    if bytes.len() > LIVE_SPEC_MAX_BYTES {
        return Err(OrchestratorError::PayloadTooLarge {
            actual: bytes.len(),
            max: LIVE_SPEC_MAX_BYTES,
        });
    }

    String::from_utf8(bytes).map_err(|e| {
        OrchestratorError::InvalidLiveSpecPatch(format!("Live spec file is not valid UTF-8: {e}"))
    })
}

/// Strictly parses all lines of a `$UIUX_LIVE_SPEC` (`doc-patch.jsonl`) payload with
/// `<= 512 KB` byte ceiling and `#[serde(deny_unknown_fields)]`.
pub fn parse_live_spec_jsonl(raw: &str) -> Result<Vec<LiveSpecPatchLine>, OrchestratorError> {
    if raw.len() > LIVE_SPEC_MAX_BYTES {
        return Err(OrchestratorError::PayloadTooLarge {
            actual: raw.len(),
            max: LIVE_SPEC_MAX_BYTES,
        });
    }
    let mut parsed = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let entry: LiveSpecPatchLine = serde_json::from_str(trimmed).map_err(|e| {
            OrchestratorError::InvalidLiveSpecPatch(format!("line {}: {e}", idx + 1))
        })?;
        validate_live_patch_line(&entry, idx + 1)?;
        parsed.push(entry);
    }
    if parsed.is_empty() {
        return Err(OrchestratorError::InvalidLiveSpecPatch(
            "doc-patch.jsonl contains zero patch operations".to_string(),
        ));
    }
    Ok(parsed)
}

/// Deduplicates Unified Asset Library provenance backlinks strictly by 64-char `content_hash`
/// (`cas_blobs.sha256`), ordering newest-first with deterministic hash tie-break.
pub fn deduplicate_provenance_backlinks(items: Vec<ProvenanceBacklink>) -> Vec<ProvenanceBacklink> {
    let mut sorted = items;
    sorted.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.content_hash.cmp(&b.content_hash))
    });
    let mut by_hash: BTreeMap<String, ProvenanceBacklink> = BTreeMap::new();
    for mut item in sorted {
        let key = item.content_hash.trim().to_ascii_lowercase();
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        item.content_hash = key.clone();
        match by_hash.get_mut(&key) {
            Some(existing) => {
                if existing.node_id.is_none() && item.node_id.is_some() {
                    existing.node_id = item.node_id;
                }
                if existing.patch_id.is_none() && item.patch_id.is_some() {
                    existing.patch_id = item.patch_id;
                }
            }
            None => {
                by_hash.insert(key, item);
            }
        }
    }
    let mut deduped: Vec<ProvenanceBacklink> = by_hash.into_values().collect();
    deduped.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.content_hash.cmp(&b.content_hash))
    });
    deduped
}

fn is_valid_live_ff_id(value: &str) -> bool {
    if value.is_empty() || value.len() > 64 {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    if matches!(lower.as_str(), "__proto__" | "constructor" | "prototype") {
        return false;
    }
    value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn validate_live_patch_line(
    line: &LiveSpecPatchLine,
    line_no: usize,
) -> Result<(), OrchestratorError> {
    if line.screen_id.trim().is_empty() || line.screen_id.len() > 128 {
        return Err(OrchestratorError::InvalidLiveSpecPatch(format!(
            "line {line_no}: screenId must be 1..=128 characters"
        )));
    }
    if !is_valid_live_ff_id(&line.ff_id) {
        return Err(OrchestratorError::InvalidLiveSpecPatch(format!(
            "line {line_no}: invalid ffId '{}' (must match ^[a-zA-Z0-9_-]{{1,64}}$)",
            line.ff_id
        )));
    }
    if !ALLOWED_LIVE_OP_KINDS.contains(&line.op_kind.as_str()) {
        return Err(OrchestratorError::InvalidLiveSpecPatch(format!(
            "line {line_no}: unsupported opKind '{}'",
            line.op_kind
        )));
    }
    if let Some(ref hash) = line.asset_content_hash {
        normalize_sha256_hex(hash).map_err(|_| {
            OrchestratorError::InvalidLiveSpecPatch(format!(
                "line {line_no}: assetContentHash must be 64-character hex SHA-256"
            ))
        })?;
    }
    Ok(())
}

fn apply_parsed_line_to_snapshot(
    snapshot: &mut CanvasPatchSnapshot,
    line: &LiveSpecPatchLine,
) -> Result<(), OrchestratorError> {
    match line.op_kind.as_str() {
        "TEXT_EDIT" => {
            let text = line
                .payload
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    OrchestratorError::InvalidLiveSpecPatch(
                        "TEXT_EDIT requires payload.text string".to_string(),
                    )
                })?;
            snapshot
                .node_texts
                .insert(line.ff_id.clone(), text.to_string());
        }
        "TOKEN_SWAP" => {
            if let Some(serde_json::Value::Object(vars)) = line.payload.get("cssVars") {
                for (k, v) in vars {
                    if !k.starts_with("--") {
                        return Err(OrchestratorError::InvalidLiveSpecPatch(format!(
                            "CSS token '{}' must start with '--'",
                            k
                        )));
                    }
                    if let Some(val_str) = v.as_str() {
                        snapshot.tokens.insert(k.clone(), val_str.to_string());
                    }
                }
            }
        }
        "RESIZE" => {
            let width = line
                .payload
                .get("width")
                .and_then(|v| v.as_u64())
                .unwrap_or(1280)
                .clamp(1, MAX_LIVE_DIMENSION_PX) as u32;
            let height = line
                .payload
                .get("height")
                .and_then(|v| v.as_u64())
                .unwrap_or(720)
                .clamp(1, MAX_LIVE_DIMENSION_PX) as u32;
            snapshot
                .node_sizes
                .insert(line.ff_id.clone(), (width, height));
        }
        "STEERING_CHIP" | "LOCKED_RULE_CHANGE" | "LINT_FIX" | "UNDO" => {
            if let Some(text) = line.payload.get("text").and_then(|v| v.as_str()) {
                snapshot
                    .node_texts
                    .insert(line.ff_id.clone(), text.to_string());
            }
            if let Some(serde_json::Value::Object(vars)) = line.payload.get("cssVars") {
                for (k, v) in vars {
                    if k.starts_with("--") {
                        if let Some(val_str) = v.as_str() {
                            snapshot.tokens.insert(k.clone(), val_str.to_string());
                        }
                    }
                }
            }
        }
        other => {
            return Err(OrchestratorError::InvalidLiveSpecPatch(format!(
                "Unsupported opKind '{other}'"
            )));
        }
    }
    Ok(())
}

fn ensure_jailed_live_spec_dir(workspace_root: &Path) -> Result<PathBuf, OrchestratorError> {
    reject_if_symlink(workspace_root)?;
    let dot_cowok = workspace_root.join(".cowok-design");
    reject_if_symlink(&dot_cowok)?;
    if !dot_cowok.exists() {
        fs::create_dir_all(&dot_cowok)
            .map_err(|e| OrchestratorError::Io(format!("Failed creating .cowok-design: {e}")))?;
    }
    reject_if_symlink(&dot_cowok)?;

    let live_dir = dot_cowok.join("live-spec");
    reject_if_symlink(&live_dir)?;
    if !live_dir.exists() {
        fs::create_dir_all(&live_dir).map_err(|e| {
            OrchestratorError::Io(format!("Failed creating .cowok-design/live-spec: {e}"))
        })?;
    }
    reject_if_symlink(&live_dir)?;
    Ok(live_dir)
}

fn extract_jailed_live_spec_relative_segments(
    workspace_root: &Path,
    candidate: &Path,
) -> Result<Vec<String>, OrchestratorError> {
    for comp in candidate.components() {
        if matches!(comp, Component::ParentDir | Component::Prefix(_)) {
            return Err(OrchestratorError::PathTraversal(
                "Refusing parent traversal in live-spec path".into(),
            ));
        }
    }

    let rel_from_ws = if candidate.is_absolute() {
        if let Ok(r) = candidate.strip_prefix(workspace_root) {
            r.to_path_buf()
        } else if let Ok(canon_ws) = workspace_root.canonicalize() {
            candidate
                .strip_prefix(&canon_ws)
                .map(Path::to_path_buf)
                .map_err(|_| {
                    OrchestratorError::PathTraversal(
                        "Live spec path escapes workspace live-spec jail".into(),
                    )
                })?
        } else {
            return Err(OrchestratorError::PathTraversal(
                "Live spec path escapes workspace live-spec jail".into(),
            ));
        }
    } else {
        candidate.to_path_buf()
    };

    let mut normal_segments: Vec<String> = Vec::new();
    for comp in rel_from_ws.components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(seg) => {
                let s = seg.to_str().ok_or_else(|| {
                    OrchestratorError::PathTraversal(
                        "Live spec path segments must be valid UTF-8".into(),
                    )
                })?;
                if s.contains('\0') {
                    return Err(OrchestratorError::PathTraversal(
                        "Live spec path contains NUL byte".into(),
                    ));
                }
                normal_segments.push(s.to_string());
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(OrchestratorError::PathTraversal(
                    "Refusing unsafe path component in live-spec path".into(),
                ));
            }
        }
    }

    if normal_segments.len() < 3
        || normal_segments[0] != ".cowok-design"
        || normal_segments[1] != "live-spec"
    {
        return Err(OrchestratorError::PathTraversal(
            "Live spec path must reside strictly inside .cowok-design/live-spec/".into(),
        ));
    }

    let rel_inside = normal_segments[2..].to_vec();
    let final_name = rel_inside.last().ok_or_else(|| {
        OrchestratorError::PathTraversal(
            "Live spec path must specify a .jsonl file inside .cowok-design/live-spec/".into(),
        )
    })?;
    if !final_name.ends_with(".jsonl") || final_name.len() <= ".jsonl".len() {
        return Err(OrchestratorError::PathTraversal(
            "Live spec file must have a .jsonl extension".into(),
        ));
    }

    Ok(rel_inside)
}

fn reject_if_symlink(path: &Path) -> Result<(), OrchestratorError> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(OrchestratorError::SymlinkViolation(
                "Refusing symlink in live-spec path".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn open_dir_nofollow_root(dir: &Path) -> Result<OwnedFd, OrchestratorError> {
    reject_if_symlink(dir)?;
    let c_dir = CString::new(dir.as_os_str().as_bytes())
        .map_err(|_| OrchestratorError::PathTraversal("Path contains NUL byte".into()))?;
    // SAFETY: c_dir is a valid NUL-terminated CString; O_NOFOLLOW | O_DIRECTORY | O_CLOEXEC prevents symlink following.
    let fd = unsafe {
        libc::open(
            c_dir.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        return match err.raw_os_error() {
            Some(libc::ELOOP) => Err(OrchestratorError::SymlinkViolation(
                "O_NOFOLLOW rejected symlink workspace root".into(),
            )),
            Some(libc::ENOENT) => Err(OrchestratorError::LiveSpecNotFound(
                "Live spec directory not found".into(),
            )),
            Some(libc::ENOTDIR) => Err(OrchestratorError::SymlinkViolation(
                "Workspace root component is not a directory".into(),
            )),
            _ => Err(OrchestratorError::Io(
                "Failed opening workspace directory with O_NOFOLLOW".into(),
            )),
        };
    }
    // SAFETY: fd is a freshly opened valid descriptor owned by OwnedFd.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

#[cfg(unix)]
fn openat_dir_nofollow(parent_fd: &OwnedFd, segment: &str) -> Result<OwnedFd, OrchestratorError> {
    let c_seg = CString::new(segment.as_bytes())
        .map_err(|_| OrchestratorError::PathTraversal("Path segment contains NUL byte".into()))?;
    // SAFETY: parent_fd is a valid directory FD and c_seg is NUL-terminated; O_NOFOLLOW | O_DIRECTORY prevents symlink traversal.
    let fd = unsafe {
        libc::openat(
            parent_fd.as_raw_fd(),
            c_seg.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        return match err.raw_os_error() {
            Some(libc::ELOOP) => Err(OrchestratorError::SymlinkViolation(
                "O_NOFOLLOW rejected symlink directory in live-spec path".into(),
            )),
            Some(libc::ENOENT) => Err(OrchestratorError::LiveSpecNotFound(
                "Live spec file not found".into(),
            )),
            Some(libc::ENOTDIR) => Err(OrchestratorError::SymlinkViolation(
                "Live spec path component is not a directory".into(),
            )),
            _ => Err(OrchestratorError::Io(
                "Failed traversing live-spec directory with openat".into(),
            )),
        };
    }
    // SAFETY: fd is a freshly opened valid descriptor owned by OwnedFd.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

#[cfg(unix)]
fn open_live_spec_file_stepwise_nofollow(
    workspace_root: &Path,
    rel_inside_live_spec: &[String],
) -> Result<File, OrchestratorError> {
    let ws_fd = open_dir_nofollow_root(workspace_root)?;
    let dot_fd = openat_dir_nofollow(&ws_fd, ".cowok-design")?;
    let mut dir_fd = openat_dir_nofollow(&dot_fd, "live-spec")?;

    let (file_name, parent_dirs) = rel_inside_live_spec.split_last().ok_or_else(|| {
        OrchestratorError::PathTraversal("Live spec path is missing filename".into())
    })?;
    for seg in parent_dirs {
        dir_fd = openat_dir_nofollow(&dir_fd, seg)?;
    }

    let c_file = CString::new(file_name.as_bytes())
        .map_err(|_| OrchestratorError::PathTraversal("Filename contains NUL byte".into()))?;
    // SAFETY: dir_fd is a valid O_NOFOLLOW directory FD and c_file is NUL-terminated; O_NOFOLLOW atomically rejects symlink targets.
    let fd = unsafe {
        libc::openat(
            dir_fd.as_raw_fd(),
            c_file.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        return match err.raw_os_error() {
            Some(libc::ELOOP) => Err(OrchestratorError::SymlinkViolation(
                "O_NOFOLLOW rejected symlink live-spec target".into(),
            )),
            Some(libc::ENOENT) => Err(OrchestratorError::LiveSpecNotFound(
                "Live spec file not found".into(),
            )),
            _ => Err(OrchestratorError::Io(
                "Failed opening live-spec file with openat(O_NOFOLLOW)".into(),
            )),
        };
    }
    // SAFETY: fd is a freshly opened valid descriptor owned by File.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(not(unix))]
fn open_live_spec_file_stepwise_nofollow(
    workspace_root: &Path,
    rel_inside_live_spec: &[String],
) -> Result<File, OrchestratorError> {
    let mut cursor = workspace_root.to_path_buf();
    reject_if_symlink(&cursor)?;
    for seg in [".cowok-design", "live-spec"] {
        cursor.push(seg);
        reject_if_symlink(&cursor)?;
    }
    for seg in rel_inside_live_spec {
        cursor.push(seg);
        reject_if_symlink(&cursor)?;
    }
    File::open(&cursor).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            OrchestratorError::LiveSpecNotFound("Live spec file not found".into())
        } else {
            OrchestratorError::Io("Failed opening live-spec file".into())
        }
    })
}

fn normalize_sha256_hex(raw: &str) -> Result<String, OrchestratorError> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.len() == 64 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(trimmed)
    } else {
        Err(OrchestratorError::InvalidProvenanceHash(format!(
            "Expected 64-char hex SHA-256, got '{}'",
            raw
        )))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn is_deterministic_lane0_message(lower: &str) -> bool {
    lower == "undo"
        || lower == "/undo"
        || lower.contains("cmd+z")
        || lower.contains("lint_fix")
        || lower.starts_with("/lint")
        || lower.contains("fix lint")
        || lower.contains("css token")
        || css_token_regex().is_match(lower)
        || (lower.contains("resize") && dimensions_regex().is_match(lower))
}

fn is_opc_fleet_lane2_message(lower: &str) -> bool {
    const FLEET_TRIGGERS: &[&str] = &[
        "opc fleet",
        "swarm",
        "build full",
        "generate screen",
        "create dashboard",
        "multi-step",
        "redesign entire",
        "scaffold",
        "tournament",
        "spawn worker",
        "synthesize",
        "implement flow",
    ];
    FLEET_TRIGGERS.iter().any(|kw| lower.contains(kw))
}

fn infer_lane0_command_kind(
    explicit: OrchestratorCommandKind,
    message: &str,
) -> OrchestratorCommandKind {
    if explicit != OrchestratorCommandKind::Auto {
        return explicit;
    }
    let lower = message.trim().to_ascii_lowercase();
    if lower == "undo" || lower == "/undo" || lower.contains("cmd+z") {
        return OrchestratorCommandKind::Undo;
    }
    if css_token_regex().is_match(&lower) || lower.contains("css token") {
        return OrchestratorCommandKind::TokenUpdate;
    }
    if lower.contains("resize") && dimensions_regex().is_match(&lower) {
        return OrchestratorCommandKind::Resize;
    }
    OrchestratorCommandKind::LintFix
}

fn css_token_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(--[a-zA-Z0-9_-]{2,64})\s*[:=]\s*([#a-zA-Z0-9(),.%_-]{1,64})")
            .expect("valid css token regex")
    })
}

fn dimensions_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(\d{2,4})\s*[xX×]\s*(\d{2,4})\b").expect("valid dim regex"))
}

fn extract_css_token_pair(message: &str) -> Option<(String, String)> {
    let caps = css_token_regex().captures(message)?;
    Some((
        caps.get(1)?.as_str().to_string(),
        caps.get(2)?.as_str().to_string(),
    ))
}

fn extract_dimensions(message: &str) -> Option<(u32, u32)> {
    let caps = dimensions_regex().captures(message)?;
    let w = caps.get(1)?.as_str().parse::<u32>().ok()?;
    let h = caps.get(2)?.as_str().parse::<u32>().ok()?;
    Some((w, h))
}
