// @story 05.5
// @implements AC-01, AC-02, AC-03, AC-04 (3-Tier Interactive Flow DAG Synthesizer, VerdictShield 0% Float Leak, 3-Color DFS & 4-SubState Persistence)
use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::Utc;
use domain::decision::ConfidenceKind;
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::{map_sqlite_error, SqlitePool};
use uuid::Uuid;

use crate::models::FlowServiceError;
use crate::queries::ensure_project_and_graph;
use crate::{with_deferred_tx, with_immediate_tx};

pub const MAX_HAPPY_PATH_CLUSTER_NODES: u8 = 8;
pub const MIN_HAPPY_PATH_CLUSTER_NODES: u8 = 1;
pub const MAX_PROJECT_ID_LEN: usize = 128;
pub const MAX_BRIEF_SUMMARY_LEN: usize = 4_000;
pub const MAX_SCREENS_INPUT: usize = 64;
pub const MAX_CANDIDATE_EDGES_INPUT: usize = 256;
pub const MAX_SCREEN_KEY_INPUT_LEN: usize = 128;
pub const MAX_SCREEN_TITLE_INPUT_LEN: usize = 200;
pub const MAX_ROUTE_PATH_INPUT_LEN: usize = 256;
pub const MAX_PERSONA_DIMENSION_LEN: usize = 128;
pub const MAX_FMEA_FAILURE_MODE_LEN: usize = 500;
pub const MAX_EDGE_NODE_REF_LEN: usize = 128;
pub const MAX_TRIGGER_EVENT_LEN: usize = 128;
pub const MAX_GUARD_CONDITION_LEN: usize = 512;
const PRIMARY_STEP_X: i32 = 320;
const PRIMARY_BASE_X: i32 = 120;
const PRIMARY_BASE_Y: i32 = 140;
const EDGE_CASE_BASE_Y: i32 = 420;
const MAX_TITLE_LEN: usize = 100;

/// Canonical 8 dimensions from `/simulate-user`.
pub const SIMULATE_USER_8D: [&str; 8] = [
    "onboarding-first-run",
    "core-happy-path",
    "search-and-filtering",
    "creation-and-editing",
    "checkout-and-commitment",
    "collaboration-and-handoff",
    "recovery-and-offline",
    "accessibility-and-empty-states",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FlowTier {
    #[serde(rename = "PRIMARY")]
    Primary,
    #[serde(rename = "EDGE_CASE")]
    EdgeCase,
}

impl FlowTier {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Primary => "PRIMARY",
            Self::EdgeCase => "EDGE_CASE",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "PRIMARY" => Some(Self::Primary),
            "EDGE_CASE" => Some(Self::EdgeCase),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubStateVariant {
    #[serde(rename = "DEFAULT")]
    Default,
    #[serde(rename = "LOADING")]
    Loading,
    #[serde(rename = "EMPTY")]
    Empty,
    #[serde(rename = "ERROR")]
    Error,
}

impl SubStateVariant {
    pub const ALL: [Self; 4] = [Self::Default, Self::Loading, Self::Empty, Self::Error];

    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::Loading => "LOADING",
            Self::Empty => "EMPTY",
            Self::Error => "ERROR",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "DEFAULT" => Some(Self::Default),
            "LOADING" => Some(Self::Loading),
            "EMPTY" => Some(Self::Empty),
            "ERROR" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Discrete FMEA severity band — VerdictShield prohibits raw float RPN scores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SeverityBand {
    #[serde(rename = "LOW")]
    Low,
    #[serde(rename = "MEDIUM")]
    Medium,
    #[serde(rename = "HIGH")]
    High,
    #[serde(rename = "CRITICAL")]
    Critical,
}

impl SeverityBand {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
            Self::Critical => "CRITICAL",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "LOW" => Some(Self::Low),
            "MEDIUM" => Some(Self::Medium),
            "HIGH" => Some(Self::High),
            "CRITICAL" => Some(Self::Critical),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenSeedInput {
    pub screen_key: String,
    pub title: String,
    #[serde(default)]
    pub route_path: Option<String>,
    #[serde(default)]
    pub persona_dimension: Option<String>,
    #[serde(default)]
    pub fmea_failure_mode: Option<String>,
    #[serde(default)]
    pub severity_band: Option<SeverityBand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RawEdgeCandidate {
    pub source_node_id: String,
    pub target_node_id: String,
    pub flow_type: String,
    pub trigger_event: String,
    #[serde(default)]
    pub guard_condition: Option<String>,
    #[serde(default)]
    pub flow_tier: Option<FlowTier>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowSynthesizeRequest {
    pub project_id: String,
    pub brief_summary: String,
    #[serde(default = "default_max_cluster_nodes")]
    pub max_default_cluster_nodes: u8,
    #[serde(default = "default_true")]
    pub include_fmea_branches: bool,
    #[serde(default)]
    pub screens: Vec<ScreenSeedInput>,
    #[serde(default)]
    pub candidate_edges: Vec<RawEdgeCandidate>,
}

fn default_max_cluster_nodes() -> u8 {
    MAX_HAPPY_PATH_CLUSTER_NODES
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SynthesizedNode {
    pub id: String,
    pub screen_key: String,
    pub title: String,
    pub route_path: String,
    pub node_type: String,
    pub flow_tier: FlowTier,
    pub cluster_id: String,
    pub severity_band: SeverityBand,
    pub merged_screen_keys: Vec<String>,
    pub pos_x: i32,
    pub pos_y: i32,
    pub default_fixture_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SynthesizedEdge {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub flow_type: String,
    pub trigger_event: String,
    pub guard_condition: Option<String>,
    pub flow_tier: FlowTier,
    pub severity_band: SeverityBand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SynthesizedSubstateBinding {
    pub node_id: String,
    pub project_id: String,
    pub substate: SubStateVariant,
    pub fixture_id: String,
    pub flow_tier: FlowTier,
    pub cluster_id: String,
    pub severity_band: SeverityBand,
    pub payload_json: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SynthesizedFlowSummary {
    pub primary_cluster_count: u32,
    pub edge_case_branch_count: u32,
    pub substate_coverage_percent: u8,
    pub float_leak_count: u32,
    pub stripped_orphan_edge_count: u32,
    pub rejected_cycle_edge_count: u32,
    pub preserved_loopback_edge_count: u32,
    pub decision_kind: ConfidenceKind,
    pub highest_severity: SeverityBand,
    pub simulated_dimensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SynthesizedFlowPlan {
    pub project_id: String,
    pub summary: SynthesizedFlowSummary,
    pub nodes: Vec<SynthesizedNode>,
    pub edges: Vec<SynthesizedEdge>,
    pub substates: Vec<SynthesizedSubstateBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredFlowSubstateDto {
    pub node_id: String,
    pub project_id: String,
    pub substate: SubStateVariant,
    pub fixture_id: String,
    pub flow_tier: FlowTier,
    pub cluster_id: String,
    pub severity_band: SeverityBand,
    pub updated_at: i64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DfsColor {
    White,
    Gray,
    Black,
}

#[derive(Debug, Default)]
struct DfsGuardStats {
    stripped_orphans: u32,
    rejected_cycles: u32,
    preserved_loopbacks: u32,
}

fn deterministic_uuid(namespace: &str, key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update(b":");
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

fn sanitize_screen_key(raw: &str, fallback_idx: usize) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_dash = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        format!("screen-{}", fallback_idx + 1)
    } else {
        trimmed.chars().take(64).collect()
    }
}

fn truncate_title(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() <= MAX_TITLE_LEN {
        return trimmed.to_string();
    }
    trimmed.chars().take(MAX_TITLE_LEN).collect()
}

fn classify_fmea_severity(text: &str) -> SeverityBand {
    let lower = text.to_ascii_lowercase();
    if lower.contains("payment")
        || lower.contains("checkout")
        || lower.contains("auth")
        || lower.contains("billing")
        || lower.contains("security")
        || lower.contains("delete")
    {
        return SeverityBand::Critical;
    }
    if lower.contains("network")
        || lower.contains("timeout")
        || lower.contains("offline")
        || lower.contains("sync")
        || lower.contains("conflict")
        || lower.contains("submit")
    {
        return SeverityBand::High;
    }
    if lower.contains("empty")
        || lower.contains("search")
        || lower.contains("filter")
        || lower.contains("form")
        || lower.contains("validation")
    {
        return SeverityBand::Medium;
    }
    SeverityBand::Low
}

fn derive_default_screens_from_brief(brief: &str) -> Vec<ScreenSeedInput> {
    let lower = brief.to_ascii_lowercase();
    let mut seeds = vec![
        ScreenSeedInput {
            screen_key: "landing-onboarding".to_string(),
            title: "Onboarding & Entry".to_string(),
            route_path: Some("/".to_string()),
            persona_dimension: Some(SIMULATE_USER_8D[0].to_string()),
            fmea_failure_mode: Some("First-run session initialization timeout".to_string()),
            severity_band: Some(SeverityBand::Medium),
        },
        ScreenSeedInput {
            screen_key: "workspace-home".to_string(),
            title: "Primary Workspace Home".to_string(),
            route_path: Some("/workspace".to_string()),
            persona_dimension: Some(SIMULATE_USER_8D[1].to_string()),
            fmea_failure_mode: Some("Empty catalog on first workspace open".to_string()),
            severity_band: Some(SeverityBand::Medium),
        },
        ScreenSeedInput {
            screen_key: "catalog-search".to_string(),
            title: "Browse & Filter".to_string(),
            route_path: Some("/browse".to_string()),
            persona_dimension: Some(SIMULATE_USER_8D[2].to_string()),
            fmea_failure_mode: Some(
                "Zero search results returned for filter combination".to_string(),
            ),
            severity_band: Some(SeverityBand::Medium),
        },
        ScreenSeedInput {
            screen_key: "item-editor".to_string(),
            title: "Configure & Edit".to_string(),
            route_path: Some("/editor".to_string()),
            persona_dimension: Some(SIMULATE_USER_8D[3].to_string()),
            fmea_failure_mode: Some("Optimistic concurrency save conflict".to_string()),
            severity_band: Some(SeverityBand::High),
        },
        ScreenSeedInput {
            screen_key: "review-checkout".to_string(),
            title: if lower.contains("checkout") || lower.contains("pay") || lower.contains("order")
            {
                "Checkout & Payment".to_string()
            } else {
                "Review & Commit".to_string()
            },
            route_path: Some("/commit".to_string()),
            persona_dimension: Some(SIMULATE_USER_8D[4].to_string()),
            fmea_failure_mode: Some("Payment or commitment gateway failure".to_string()),
            severity_band: Some(SeverityBand::Critical),
        },
        ScreenSeedInput {
            screen_key: "confirmation-handoff".to_string(),
            title: "Confirmation & Handoff".to_string(),
            route_path: Some("/done".to_string()),
            persona_dimension: Some(SIMULATE_USER_8D[5].to_string()),
            fmea_failure_mode: Some("Offline reconnect after handoff".to_string()),
            severity_band: Some(SeverityBand::Low),
        },
    ];
    if lower.contains("admin") || lower.contains("analytics") || lower.contains("settings") {
        seeds.push(ScreenSeedInput {
            screen_key: "settings-governance".to_string(),
            title: "Settings & Governance".to_string(),
            route_path: Some("/settings".to_string()),
            persona_dimension: Some(SIMULATE_USER_8D[6].to_string()),
            fmea_failure_mode: Some("Permission denied on workspace settings".to_string()),
            severity_band: Some(SeverityBand::High),
        });
    }
    seeds
}

fn partition_screens_into_clusters(
    screens: &[ScreenSeedInput],
    max_clusters: usize,
) -> Vec<Vec<&ScreenSeedInput>> {
    if screens.is_empty() {
        return Vec::new();
    }
    let cluster_count = screens.len().min(max_clusters.max(1));
    let mut clusters: Vec<Vec<&ScreenSeedInput>> = vec![Vec::new(); cluster_count];
    let base_size = screens.len() / cluster_count;
    let remainder = screens.len() % cluster_count;

    let mut cursor = 0usize;
    for (idx, bucket) in clusters.iter_mut().enumerate() {
        let take = base_size + usize::from(idx < remainder);
        for item in &screens[cursor..cursor + take] {
            bucket.push(item);
        }
        cursor += take;
    }
    clusters
}

fn build_primary_nodes(
    project_id: &str,
    brief_summary: &str,
    screens: &[ScreenSeedInput],
    cap: usize,
) -> (Vec<SynthesizedNode>, HashMap<String, String>) {
    let clusters = partition_screens_into_clusters(screens, cap);
    let mut nodes = Vec::with_capacity(clusters.len());
    let mut key_to_node_id = HashMap::new();
    let mut used_keys = BTreeSet::new();

    for (idx, bucket) in clusters.iter().enumerate() {
        let Some(lead) = bucket.first() else {
            continue;
        };
        let cluster_id = format!("cluster-{}", idx + 1);
        let base_key = sanitize_screen_key(&lead.screen_key, idx);
        let screen_key = if used_keys.insert(base_key.clone()) {
            base_key
        } else {
            let unique_key = format!("{base_key}-{}", idx + 1);
            used_keys.insert(unique_key.clone());
            unique_key
        };

        let merged_keys: Vec<String> = bucket
            .iter()
            .enumerate()
            .map(|(j, s)| sanitize_screen_key(&s.screen_key, idx * 10 + j))
            .collect();

        let title = if bucket.len() > 1 {
            truncate_title(&format!(
                "{} (+{} screens)",
                lead.title.trim(),
                bucket.len() - 1
            ))
        } else {
            truncate_title(&lead.title)
        };

        let severity_band = bucket
            .iter()
            .map(|s| {
                s.severity_band.unwrap_or_else(|| {
                    let combined = format!(
                        "{} {} {}",
                        s.title,
                        s.fmea_failure_mode.as_deref().unwrap_or(""),
                        brief_summary
                    );
                    classify_fmea_severity(&combined)
                })
            })
            .max()
            .unwrap_or(SeverityBand::Low);

        let node_id = deterministic_uuid("flow-node", &format!("{project_id}:{screen_key}"));
        let default_fixture_id =
            deterministic_uuid("flow-fixture", &format!("{project_id}:{node_id}:DEFAULT"));

        for s in bucket {
            key_to_node_id.insert(s.screen_key.clone(), node_id.clone());
        }
        key_to_node_id.insert(screen_key.clone(), node_id.clone());

        nodes.push(SynthesizedNode {
            id: node_id,
            screen_key,
            title,
            route_path: lead
                .route_path
                .clone()
                .filter(|r| !r.trim().is_empty())
                .unwrap_or_else(|| format!("/cluster-{}", idx + 1)),
            node_type: "screen".to_string(),
            flow_tier: FlowTier::Primary,
            cluster_id,
            severity_band,
            merged_screen_keys: merged_keys,
            pos_x: PRIMARY_BASE_X + (idx as i32) * PRIMARY_STEP_X,
            pos_y: PRIMARY_BASE_Y,
            default_fixture_id,
        });
    }
    (nodes, key_to_node_id)
}

fn build_edge_case_nodes_and_edges(
    project_id: &str,
    primary_nodes: &[SynthesizedNode],
) -> (Vec<SynthesizedNode>, Vec<SynthesizedEdge>) {
    if primary_nodes.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut edge_nodes = Vec::new();
    let mut edge_links = Vec::new();

    // Anchor up to 3 high-priority FMEA recovery branches: EMPTY onboarding, ERROR network/retry, FALLBACK checkout/guard.
    let targets: Vec<(&SynthesizedNode, &str, &str, &str, SeverityBand)> = {
        let mut list = Vec::new();
        let first = &primary_nodes[0];
        list.push((
            first,
            "fmea-empty-onboarding-fallback",
            "Empty State Recovery Guide",
            "onEmptyDataset",
            SeverityBand::Medium,
        ));
        let mid = &primary_nodes[primary_nodes.len() / 2];
        list.push((
            mid,
            "fmea-network-error-recovery",
            "Network Timeout & Retry Fallback",
            "onRequestError",
            SeverityBand::High,
        ));
        if primary_nodes.len() >= 2 {
            let last = &primary_nodes[primary_nodes.len() - 1];
            list.push((
                last,
                "fmea-critical-abort-fallback",
                "Critical Transaction Fallback",
                "onFmeaGuardTrip",
                last.severity_band.max(SeverityBand::Critical),
            ));
        }
        list
    };

    for (idx, (anchor, key, title, trigger, severity)) in targets.into_iter().enumerate() {
        let node_id = deterministic_uuid("flow-node", &format!("{project_id}:{key}"));
        let default_fixture_id =
            deterministic_uuid("flow-fixture", &format!("{project_id}:{node_id}:DEFAULT"));
        edge_nodes.push(SynthesizedNode {
            id: node_id.clone(),
            screen_key: key.to_string(),
            title: title.to_string(),
            route_path: format!("/recovery/{key}"),
            node_type: "condition".to_string(),
            flow_tier: FlowTier::EdgeCase,
            cluster_id: anchor.cluster_id.clone(),
            severity_band: severity,
            merged_screen_keys: vec![key.to_string()],
            pos_x: anchor.pos_x,
            pos_y: EDGE_CASE_BASE_Y + ((idx % 2) as i32) * 32,
            default_fixture_id,
        });

        // Primary -> EdgeCase fallback branch (`if_not`)
        edge_links.push(SynthesizedEdge {
            id: deterministic_uuid(
                "flow-edge",
                &format!("{project_id}:{}:{node_id}:{trigger}", anchor.id),
            ),
            source_node_id: anchor.id.clone(),
            target_node_id: node_id.clone(),
            flow_type: "if_not".to_string(),
            trigger_event: trigger.to_string(),
            guard_condition: Some(format!("fmea_severity >= {}", severity.as_db_str())),
            flow_tier: FlowTier::EdgeCase,
            severity_band: severity,
        });

        // EdgeCase -> Primary valid Story 05.1 user recovery `loopback` arc!
        let retry_trigger = format!("onUserRetry{}", idx + 1);
        edge_links.push(SynthesizedEdge {
            id: deterministic_uuid(
                "flow-edge",
                &format!("{project_id}:{node_id}:{}:{retry_trigger}", anchor.id),
            ),
            source_node_id: node_id,
            target_node_id: anchor.id.clone(),
            flow_type: "loopback".to_string(),
            trigger_event: retry_trigger,
            guard_condition: Some("user_confirmed_retry == true".to_string()),
            flow_tier: FlowTier::EdgeCase,
            severity_band: severity,
        });
    }

    (edge_nodes, edge_links)
}

fn build_substate_fixture_payload(node: &SynthesizedNode, variant: SubStateVariant) -> String {
    let val = match variant {
        SubStateVariant::Default => serde_json::json!({
            "state": "DEFAULT",
            "screenKey": node.screen_key,
            "clusterId": node.cluster_id,
            "itemCount": 3,
            "ready": true
        }),
        SubStateVariant::Loading => serde_json::json!({
            "state": "LOADING",
            "screenKey": node.screen_key,
            "skeletonRows": 4,
            "isLoading": true
        }),
        SubStateVariant::Empty => serde_json::json!({
            "state": "EMPTY",
            "screenKey": node.screen_key,
            "itemCount": 0,
            "emptyCta": "create-first-record"
        }),
        SubStateVariant::Error => serde_json::json!({
            "state": "ERROR",
            "screenKey": node.screen_key,
            "severityBand": node.severity_band.as_db_str(),
            "errorCode": "FMEA_RECOVERABLE_FAULT",
            "retryable": true
        }),
    };
    val.to_string()
}

fn build_all_substate_bindings(
    project_id: &str,
    nodes: &[SynthesizedNode],
    now_ms: i64,
) -> Vec<SynthesizedSubstateBinding> {
    let mut bindings = Vec::with_capacity(nodes.len() * SubStateVariant::ALL.len());
    for node in nodes {
        for variant in SubStateVariant::ALL {
            let fixture_id = deterministic_uuid(
                "flow-fixture",
                &format!("{project_id}:{}:{}", node.id, variant.as_db_str()),
            );
            bindings.push(SynthesizedSubstateBinding {
                node_id: node.id.clone(),
                project_id: project_id.to_string(),
                substate: variant,
                fixture_id,
                flow_tier: node.flow_tier,
                cluster_id: node.cluster_id.clone(),
                severity_band: node.severity_band,
                payload_json: build_substate_fixture_payload(node, variant),
                updated_at: now_ms,
            });
        }
    }
    bindings
}

fn has_directed_cycle_3color(
    node_ids: &BTreeSet<String>,
    adj: &BTreeMap<String, BTreeSet<String>>,
) -> bool {
    let mut colors: BTreeMap<&str, DfsColor> = node_ids
        .iter()
        .map(|id| (id.as_str(), DfsColor::White))
        .collect();
    for id in node_ids {
        if colors.get(id.as_str()) == Some(&DfsColor::White)
            && dfs_detect_cycle(id.as_str(), adj, &mut colors)
        {
            return true;
        }
    }
    false
}

fn dfs_detect_cycle<'a>(
    node: &'a str,
    adj: &'a BTreeMap<String, BTreeSet<String>>,
    colors: &mut BTreeMap<&'a str, DfsColor>,
) -> bool {
    colors.insert(node, DfsColor::Gray);
    if let Some(neighbors) = adj.get(node) {
        for next in neighbors {
            match colors
                .get(next.as_str())
                .copied()
                .unwrap_or(DfsColor::White)
            {
                DfsColor::Gray => return true,
                DfsColor::White => {
                    if dfs_detect_cycle(next.as_str(), adj, colors) {
                        return true;
                    }
                }
                DfsColor::Black => {}
            }
        }
    }
    colors.insert(node, DfsColor::Black);
    false
}

fn sanitize_edges_with_3color_dfs(
    nodes: &[SynthesizedNode],
    raw_edges: Vec<SynthesizedEdge>,
) -> (Vec<SynthesizedEdge>, DfsGuardStats) {
    let valid_ids: BTreeSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let mut stats = DfsGuardStats::default();
    let mut accepted_edges = Vec::new();
    let mut loopback_edges = Vec::new();
    let mut adj: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut dedup_keys: BTreeSet<(String, String, String)> = BTreeSet::new();

    for edge in raw_edges {
        if !valid_ids.contains(&edge.source_node_id) || !valid_ids.contains(&edge.target_node_id) {
            stats.stripped_orphans += 1;
            continue;
        }
        if edge.source_node_id == edge.target_node_id {
            stats.rejected_cycles += 1;
            continue;
        }
        let dedup_key = (
            edge.source_node_id.clone(),
            edge.target_node_id.clone(),
            edge.trigger_event.clone(),
        );
        if dedup_keys.contains(&dedup_key) {
            continue;
        }

        // AC-04: Preserve valid Story 05.1 user loopback edges (`flow_type == "loopback"`).
        if edge.flow_type == "loopback" {
            dedup_keys.insert(dedup_key);
            stats.preserved_loopbacks += 1;
            loopback_edges.push(edge);
            continue;
        }

        // 3-Color DFS guard for all non-loopback transitions.
        let inserted = adj
            .entry(edge.source_node_id.clone())
            .or_default()
            .insert(edge.target_node_id.clone());

        if has_directed_cycle_3color(&valid_ids, &adj) {
            if inserted {
                if let Some(set) = adj.get_mut(&edge.source_node_id) {
                    set.remove(&edge.target_node_id);
                }
            }
            stats.rejected_cycles += 1;
            continue;
        }
        dedup_keys.insert(dedup_key);
        accepted_edges.push(edge);
    }

    accepted_edges.extend(loopback_edges);
    (accepted_edges, stats)
}

/// Recursively audits any `serde_json::Value` for floating-point numbers (`is_f64()` without integer representation).
pub fn count_json_floats(value: &serde_json::Value) -> u32 {
    match value {
        serde_json::Value::Number(n) => u32::from(n.as_i64().is_none() && n.as_u64().is_none()),
        serde_json::Value::Array(items) => items.iter().map(count_json_floats).sum(),
        serde_json::Value::Object(map) => map.values().map(count_json_floats).sum(),
        _ => 0,
    }
}

/// Validates DoS bounds and string field lengths for `FlowSynthesizeRequest`.
pub fn validate_synthesize_request(req: &FlowSynthesizeRequest) -> Result<(), FlowServiceError> {
    let project_id = req.project_id.trim();
    if project_id.is_empty() || project_id.len() > MAX_PROJECT_ID_LEN {
        return Err(FlowServiceError::new(
            422,
            "VALIDATION_ERROR",
            "projectId must be 1..=128 characters",
        ));
    }
    let brief = req.brief_summary.trim();
    if brief.is_empty() || brief.len() > MAX_BRIEF_SUMMARY_LEN {
        return Err(FlowServiceError::new(
            422,
            "VALIDATION_ERROR",
            "briefSummary must be 1..=4000 characters",
        ));
    }
    if req.max_default_cluster_nodes < MIN_HAPPY_PATH_CLUSTER_NODES
        || req.max_default_cluster_nodes > MAX_HAPPY_PATH_CLUSTER_NODES
    {
        return Err(FlowServiceError::new(
            422,
            "VALIDATION_ERROR",
            "maxDefaultClusterNodes must be between 1 and 8",
        ));
    }
    if req.screens.len() > MAX_SCREENS_INPUT {
        return Err(FlowServiceError::new(
            422,
            "VALIDATION_ERROR",
            "screens exceeds maximum bound of 64",
        ));
    }
    if req.candidate_edges.len() > MAX_CANDIDATE_EDGES_INPUT {
        return Err(FlowServiceError::new(
            422,
            "VALIDATION_ERROR",
            "candidateEdges exceeds maximum bound of 256",
        ));
    }
    for (idx, screen) in req.screens.iter().enumerate() {
        let sk = screen.screen_key.trim();
        if sk.is_empty() || sk.len() > MAX_SCREEN_KEY_INPUT_LEN {
            return Err(FlowServiceError::new(
                422,
                "VALIDATION_ERROR",
                format!(
                    "screens[{idx}].screenKey must be 1..={MAX_SCREEN_KEY_INPUT_LEN} characters"
                ),
            ));
        }
        let title = screen.title.trim();
        if title.is_empty() || title.len() > MAX_SCREEN_TITLE_INPUT_LEN {
            return Err(FlowServiceError::new(
                422,
                "VALIDATION_ERROR",
                format!("screens[{idx}].title must be 1..={MAX_SCREEN_TITLE_INPUT_LEN} characters"),
            ));
        }
        if let Some(ref rp) = screen.route_path {
            if rp.len() > MAX_ROUTE_PATH_INPUT_LEN {
                return Err(FlowServiceError::new(
                    422,
                    "VALIDATION_ERROR",
                    format!(
                        "screens[{idx}].routePath exceeds {MAX_ROUTE_PATH_INPUT_LEN} characters"
                    ),
                ));
            }
        }
        if let Some(ref pd) = screen.persona_dimension {
            if pd.len() > MAX_PERSONA_DIMENSION_LEN {
                return Err(FlowServiceError::new(
                    422,
                    "VALIDATION_ERROR",
                    format!("screens[{idx}].personaDimension exceeds {MAX_PERSONA_DIMENSION_LEN} characters"),
                ));
            }
        }
        if let Some(ref fm) = screen.fmea_failure_mode {
            if fm.len() > MAX_FMEA_FAILURE_MODE_LEN {
                return Err(FlowServiceError::new(
                    422,
                    "VALIDATION_ERROR",
                    format!("screens[{idx}].fmeaFailureMode exceeds {MAX_FMEA_FAILURE_MODE_LEN} characters"),
                ));
            }
        }
    }
    for (idx, edge) in req.candidate_edges.iter().enumerate() {
        let src = edge.source_node_id.trim();
        let dst = edge.target_node_id.trim();
        let trig = edge.trigger_event.trim();
        if src.is_empty() || src.len() > MAX_EDGE_NODE_REF_LEN {
            return Err(FlowServiceError::new(
                422,
                "VALIDATION_ERROR",
                format!("candidateEdges[{idx}].sourceNodeId must be 1..={MAX_EDGE_NODE_REF_LEN} characters"),
            ));
        }
        if dst.is_empty() || dst.len() > MAX_EDGE_NODE_REF_LEN {
            return Err(FlowServiceError::new(
                422,
                "VALIDATION_ERROR",
                format!("candidateEdges[{idx}].targetNodeId must be 1..={MAX_EDGE_NODE_REF_LEN} characters"),
            ));
        }
        if trig.is_empty() || trig.len() > MAX_TRIGGER_EVENT_LEN {
            return Err(FlowServiceError::new(
                422,
                "VALIDATION_ERROR",
                format!("candidateEdges[{idx}].triggerEvent must be 1..={MAX_TRIGGER_EVENT_LEN} characters"),
            ));
        }
        if let Some(ref gc) = edge.guard_condition {
            if gc.len() > MAX_GUARD_CONDITION_LEN {
                return Err(FlowServiceError::new(
                    422,
                    "VALIDATION_ERROR",
                    format!("candidateEdges[{idx}].guardCondition exceeds {MAX_GUARD_CONDITION_LEN} characters"),
                ));
            }
        }
    }
    Ok(())
}

/// Pure deterministic 3-Tier Flow DAG Synthesizer (`/simulate-user` 8D + FMEA + VerdictShield + 3-Color DFS)
/// accepting a verified `ConfidenceKind` emitted by `bot_fleet::DecisionCascadeClient` (`09.2`).
pub fn synthesize_3tier_flow_plan_with_decision(
    req: &FlowSynthesizeRequest,
    decision_kind: ConfidenceKind,
) -> Result<SynthesizedFlowPlan, FlowServiceError> {
    validate_synthesize_request(req)?;
    let project_id = req.project_id.trim();

    let cluster_cap =
        req.max_default_cluster_nodes
            .clamp(MIN_HAPPY_PATH_CLUSTER_NODES, MAX_HAPPY_PATH_CLUSTER_NODES) as usize;

    let screen_seeds = if req.screens.is_empty() {
        derive_default_screens_from_brief(&req.brief_summary)
    } else {
        req.screens.clone()
    };

    let (primary_nodes, mut key_to_node_id) =
        build_primary_nodes(project_id, &req.brief_summary, &screen_seeds, cluster_cap);

    let mut raw_edges: Vec<SynthesizedEdge> = Vec::new();
    for pair in primary_nodes.windows(2) {
        let src = &pair[0];
        let dst = &pair[1];
        let trigger = format!("onProceedTo_{}", dst.screen_key);
        raw_edges.push(SynthesizedEdge {
            id: deterministic_uuid(
                "flow-edge",
                &format!("{project_id}:{}:{}:{trigger}", src.id, dst.id),
            ),
            source_node_id: src.id.clone(),
            target_node_id: dst.id.clone(),
            flow_type: "main".to_string(),
            trigger_event: trigger,
            guard_condition: None,
            flow_tier: FlowTier::Primary,
            severity_band: dst.severity_band,
        });
    }

    let mut all_nodes = primary_nodes.clone();
    if req.include_fmea_branches {
        let (fmea_nodes, fmea_edges) = build_edge_case_nodes_and_edges(project_id, &primary_nodes);
        for fnode in &fmea_nodes {
            key_to_node_id.insert(fnode.screen_key.clone(), fnode.id.clone());
        }
        all_nodes.extend(fmea_nodes);
        raw_edges.extend(fmea_edges);
    }

    // Merge caller-supplied candidate edges (supports resolving by UUID, primary screen_key, or FMEA screen_key).
    for cand in &req.candidate_edges {
        let resolved_src = key_to_node_id
            .get(cand.source_node_id.trim())
            .cloned()
            .unwrap_or_else(|| cand.source_node_id.trim().to_string());
        let resolved_dst = key_to_node_id
            .get(cand.target_node_id.trim())
            .cloned()
            .unwrap_or_else(|| cand.target_node_id.trim().to_string());
        let normalized_flow_type = match cand.flow_type.trim() {
            "main" | "if_else" | "if_not" | "loopback" => cand.flow_type.trim().to_string(),
            _ => "main".to_string(),
        };
        let trigger = if cand.trigger_event.trim().is_empty() {
            "onAction".to_string()
        } else {
            cand.trigger_event.trim().to_string()
        };
        let tier = cand.flow_tier.unwrap_or_else(|| {
            if normalized_flow_type == "main" {
                FlowTier::Primary
            } else {
                FlowTier::EdgeCase
            }
        });
        raw_edges.push(SynthesizedEdge {
            id: deterministic_uuid(
                "flow-edge",
                &format!("{project_id}:{resolved_src}:{resolved_dst}:{trigger}"),
            ),
            source_node_id: resolved_src,
            target_node_id: resolved_dst,
            flow_type: normalized_flow_type,
            trigger_event: trigger,
            guard_condition: cand.guard_condition.clone(),
            flow_tier: tier,
            severity_band: SeverityBand::Medium,
        });
    }

    let (edges, dfs_stats) = sanitize_edges_with_3color_dfs(&all_nodes, raw_edges);
    let now_ms = Utc::now().timestamp_millis();
    let substates = build_all_substate_bindings(project_id, &all_nodes, now_ms);

    let highest_severity = all_nodes
        .iter()
        .map(|n| n.severity_band)
        .max()
        .unwrap_or(SeverityBand::Low);

    let edge_case_branch_count = all_nodes
        .iter()
        .filter(|n| n.flow_tier == FlowTier::EdgeCase)
        .count() as u32;

    let mut plan = SynthesizedFlowPlan {
        project_id: project_id.to_string(),
        summary: SynthesizedFlowSummary {
            primary_cluster_count: primary_nodes.len() as u32,
            edge_case_branch_count,
            substate_coverage_percent: if all_nodes.is_empty() { 0 } else { 100 },
            float_leak_count: 0,
            stripped_orphan_edge_count: dfs_stats.stripped_orphans,
            rejected_cycle_edge_count: dfs_stats.rejected_cycles,
            preserved_loopback_edge_count: dfs_stats.preserved_loopbacks,
            decision_kind,
            highest_severity,
            simulated_dimensions: SIMULATE_USER_8D.iter().map(|s| (*s).to_string()).collect(),
        },
        nodes: all_nodes,
        edges,
        substates,
    };

    // VerdictShield self-audit: guarantees 0 float leaks before returning.
    if let Ok(json_val) = serde_json::to_value(&plan) {
        plan.summary.float_leak_count = count_json_floats(&json_val);
    }

    Ok(plan)
}

/// Convenience wrapper for pure local synthesis defaulting to `ExactMatch` (when screens provided)
/// or `DeterministicFallback`. HTTP callers MUST pass the verified `ConfidenceKind` from `DecisionCascadeClient`.
pub fn synthesize_3tier_flow_plan(
    req: &FlowSynthesizeRequest,
) -> Result<SynthesizedFlowPlan, FlowServiceError> {
    let default_kind = if !req.screens.is_empty() {
        ConfidenceKind::ExactMatch
    } else {
        ConfidenceKind::DeterministicFallback
    };
    synthesize_3tier_flow_plan_with_decision(req, default_kind)
}

fn upsert_node_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    graph_id: &str,
    node: &SynthesizedNode,
    now_ms: i64,
) -> Result<String, FlowServiceError> {
    let note_val = format!(
        "tier:{} severity:{}",
        node.flow_tier.as_db_str(),
        node.severity_band.as_db_str()
    );
    let existing_id: Option<String> = tx
        .query_row(
            "SELECT id FROM flow_nodes WHERE tenant_id = ?1 AND project_id = ?2 AND screen_key = ?3",
            params![tenant_id, project_id, node.screen_key],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "lookup_synth_node")))?;

    if let Some(ex_id) = existing_id {
        tx.execute(
            "UPDATE flow_nodes SET title = ?1, route_path = ?2, node_type = ?3, note = ?4, \
             pos_x = ?5, pos_y = ?6, portal_group = ?7, updated_at = ?8, version = version + 1 WHERE id = ?9",
            params![
                node.title,
                node.route_path,
                node.node_type,
                note_val,
                f64::from(node.pos_x),
                f64::from(node.pos_y),
                node.cluster_id,
                now_ms,
                ex_id
            ],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "update_synth_node")))?;
        return Ok(ex_id);
    }

    tx.execute(
        "INSERT INTO flow_nodes ( \
            id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type, \
            route_path, component_entry_path, fixture_ref, fixture_json, note, viewport_preset, \
            pos_x, pos_y, version, layout_version, created_at, updated_at, \
            portal_group, shared_app_shell_id, mini_skeleton_html, cta_pills_json, revision \
         ) VALUES (?1, ?2, ?3, ?4, 'default', ?5, ?6, ?7, ?8, '', 'DEFAULT', '{}', ?9, 'desktop-1440', \
                   ?10, ?11, 1, 1, ?12, ?12, ?13, NULL, NULL, '[]', 1)",
        params![
            node.id,
            graph_id,
            tenant_id,
            project_id,
            node.screen_key,
            node.title,
            node.node_type,
            node.route_path,
            note_val,
            f64::from(node.pos_x),
            f64::from(node.pos_y),
            now_ms,
            node.cluster_id
        ],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "insert_synth_node")))?;

    Ok(node.id.clone())
}

fn upsert_edge_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    graph_id: &str,
    edge: &mut SynthesizedEdge,
    now_ms: i64,
) -> Result<(), FlowServiceError> {
    let existing_edge_id: Option<String> = tx
        .query_row(
            "SELECT id FROM flow_edges \
             WHERE project_id = ?1 AND source_node_id = ?2 AND target_node_id = ?3 AND trigger_event = ?4",
            params![
                project_id,
                edge.source_node_id,
                edge.target_node_id,
                edge.trigger_event
            ],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "lookup_synth_edge")))?;

    if let Some(ex_id) = existing_edge_id {
        tx.execute(
            "UPDATE flow_edges SET flow_type = ?1, guard_condition = ?2, updated_at = ?3, \
             version = version + 1, revision = revision + 1 WHERE id = ?4",
            params![edge.flow_type, edge.guard_condition, now_ms, ex_id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "update_synth_edge")))?;
        edge.id = ex_id;
        return Ok(());
    }

    // Recompute deterministic id against final remapped endpoints; avoid PK collision with any existing row.
    let mut candidate_id = deterministic_uuid(
        "flow-edge",
        &format!(
            "{project_id}:{}:{}:{}",
            edge.source_node_id, edge.target_node_id, edge.trigger_event
        ),
    );
    let pk_taken: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM flow_edges WHERE id = ?1",
            params![candidate_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "check_synth_edge_pk")))?;
    if pk_taken.is_some() {
        candidate_id = Uuid::new_v4().to_string();
    }
    edge.id = candidate_id;

    tx.execute(
        "INSERT INTO flow_edges ( \
            id, graph_id, tenant_id, project_id, source_node_id, target_node_id, \
            flow_type, trigger_event, guard_condition, version, \
            source_cta_ff_id, max_loop_iterations, revision, created_at, updated_at \
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, NULL, 3, 1, ?10, ?10)",
        params![
            edge.id,
            graph_id,
            tenant_id,
            project_id,
            edge.source_node_id,
            edge.target_node_id,
            edge.flow_type,
            edge.trigger_event,
            edge.guard_condition,
            now_ms
        ],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "insert_synth_edge")))?;

    Ok(())
}

fn collect_previously_synthesized_node_ids(
    tx: &Transaction<'_>,
    project_id: &str,
) -> Result<BTreeSet<String>, FlowServiceError> {
    let mut stmt = tx
        .prepare("SELECT DISTINCT node_id FROM flow_node_substates WHERE project_id = ?1")
        .map_err(|e| {
            FlowServiceError::from_harness(map_sqlite_error(e, "prepare_prev_synth_nodes"))
        })?;
    let rows = stmt
        .query_map(params![project_id], |r| r.get::<_, String>(0))
        .map_err(|e| {
            FlowServiceError::from_harness(map_sqlite_error(e, "query_prev_synth_nodes"))
        })?;
    let mut out = BTreeSet::new();
    for item in rows {
        out.insert(item.map_err(|e| {
            FlowServiceError::from_harness(map_sqlite_error(e, "read_prev_synth_node"))
        })?);
    }
    Ok(out)
}

fn prune_stale_synthesized_nodes_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    previously_synthesized_ids: &BTreeSet<String>,
    active_node_ids: &BTreeSet<String>,
) -> Result<(), FlowServiceError> {
    for stale_id in previously_synthesized_ids.difference(active_node_ids) {
        tx.execute(
            "DELETE FROM flow_node_substates WHERE project_id = ?1 AND node_id = ?2",
            params![project_id, stale_id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "prune_stale_substate")))?;
        tx.execute(
            "DELETE FROM screen_fixtures WHERE tenant_id = ?1 AND project_id = ?2 AND node_id = ?3",
            params![tenant_id, project_id, stale_id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "prune_stale_fixture")))?;
        tx.execute(
            "DELETE FROM flow_edges WHERE tenant_id = ?1 AND project_id = ?2 \
             AND (source_node_id = ?3 OR target_node_id = ?3)",
            params![tenant_id, project_id, stale_id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "prune_stale_edges")))?;
        tx.execute(
            "DELETE FROM flow_nodes WHERE tenant_id = ?1 AND project_id = ?2 AND id = ?3",
            params![tenant_id, project_id, stale_id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "prune_stale_node")))?;
    }
    Ok(())
}

fn upsert_fixture_and_substate_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    binding: &mut SynthesizedSubstateBinding,
    now_ms: i64,
) -> Result<(), FlowServiceError> {
    let existing_fixture_id: Option<String> = tx
        .query_row(
            "SELECT id FROM screen_fixtures \
             WHERE tenant_id = ?1 AND project_id = ?2 AND node_id = ?3 AND variant = ?4",
            params![
                tenant_id,
                project_id,
                binding.node_id,
                binding.substate.as_db_str()
            ],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "lookup_synth_fixture")))?;

    let final_fixture_id = if let Some(ex_fid) = existing_fixture_id {
        tx.execute(
            "UPDATE screen_fixtures SET payload_json = ?1, updated_at = ?2, revision = revision + 1 WHERE id = ?3",
            params![binding.payload_json, now_ms, ex_fid],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "update_synth_fixture")))?;
        ex_fid
    } else {
        tx.execute(
            "INSERT INTO screen_fixtures ( \
                id, tenant_id, project_id, node_id, variant, payload_json, is_default, \
                preset_key, sample_rows_json, revision, created_at, updated_at \
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '[]', 1, ?9, ?9)",
            params![
                binding.fixture_id,
                tenant_id,
                project_id,
                binding.node_id,
                binding.substate.as_db_str(),
                binding.payload_json,
                i32::from(binding.substate == SubStateVariant::Default),
                Some(format!(
                    "synth-{}",
                    binding.substate.as_db_str().to_ascii_lowercase()
                )),
                now_ms
            ],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "insert_synth_fixture")))?;
        binding.fixture_id.clone()
    };

    binding.fixture_id = final_fixture_id.clone();
    binding.updated_at = now_ms;

    tx.execute(
        "INSERT INTO flow_node_substates ( \
            node_id, project_id, substate, fixture_id, flow_tier, cluster_id, severity_band, updated_at \
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(node_id, substate) DO UPDATE SET \
            fixture_id = excluded.fixture_id, \
            flow_tier = excluded.flow_tier, \
            cluster_id = excluded.cluster_id, \
            severity_band = excluded.severity_band, \
            updated_at = excluded.updated_at",
        params![
            binding.node_id,
            project_id,
            binding.substate.as_db_str(),
            final_fixture_id,
            binding.flow_tier.as_db_str(),
            binding.cluster_id,
            binding.severity_band.as_db_str(),
            now_ms
        ],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "upsert_flow_node_substate")))?;

    Ok(())
}

/// Synthesizes a 3-Tier Flow DAG and persists `flow_nodes`, `flow_edges`, `screen_fixtures`,
/// and `flow_node_substates` atomically inside `TransactionBehavior::Immediate`.
pub fn synthesize_and_persist_flow(
    pool: &SqlitePool,
    tenant_id: &str,
    req: &FlowSynthesizeRequest,
    decision_kind: ConfidenceKind,
) -> Result<SynthesizedFlowPlan, FlowServiceError> {
    let mut plan = synthesize_3tier_flow_plan_with_decision(req, decision_kind)?;
    let project_id = plan.project_id.clone();

    with_immediate_tx(pool, |tx| {
        let hdr = ensure_project_and_graph(tx, tenant_id, &project_id, true)?;
        let now_ms = Utc::now().timestamp_millis();
        let previously_synthesized = collect_previously_synthesized_node_ids(tx, &project_id)?;

        let mut id_remap: HashMap<String, String> = HashMap::new();
        let mut active_node_ids: BTreeSet<String> = BTreeSet::new();
        for node in &mut plan.nodes {
            let old_id = node.id.clone();
            let actual_id = upsert_node_in_tx(tx, tenant_id, &project_id, &hdr.id, node, now_ms)?;
            if actual_id != old_id {
                id_remap.insert(old_id, actual_id.clone());
                node.id = actual_id.clone();
            }
            active_node_ids.insert(actual_id);
        }

        prune_stale_synthesized_nodes_in_tx(
            tx,
            tenant_id,
            &project_id,
            &previously_synthesized,
            &active_node_ids,
        )?;

        for edge in &mut plan.edges {
            if let Some(mapped) = id_remap.get(&edge.source_node_id) {
                edge.source_node_id = mapped.clone();
            }
            if let Some(mapped) = id_remap.get(&edge.target_node_id) {
                edge.target_node_id = mapped.clone();
            }
            upsert_edge_in_tx(tx, tenant_id, &project_id, &hdr.id, edge, now_ms)?;
        }

        let mut default_fixture_by_node: HashMap<String, String> = HashMap::new();
        for binding in &mut plan.substates {
            if let Some(mapped) = id_remap.get(&binding.node_id) {
                binding.node_id = mapped.clone();
            }
            upsert_fixture_and_substate_in_tx(tx, tenant_id, &project_id, binding, now_ms)?;
            if binding.substate == SubStateVariant::Default {
                default_fixture_by_node.insert(binding.node_id.clone(), binding.fixture_id.clone());
            }
        }

        for node in &mut plan.nodes {
            if let Some(fid) = default_fixture_by_node.get(&node.id) {
                node.default_fixture_id = fid.clone();
            }
        }

        tx.execute(
            "UPDATE flow_graphs SET version = version + 1, updated_at = ?1 WHERE id = ?2",
            params![now_ms, hdr.id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "bump_synth_graph")))?;

        Ok(())
    })?;

    Ok(plan)
}

/// Lists persisted `flow_node_substates` records for a project, optionally filtered by `FlowTier`.
pub fn list_flow_substates(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    flow_tier: Option<FlowTier>,
) -> Result<Vec<StoredFlowSubstateDto>, FlowServiceError> {
    with_deferred_tx(pool, |tx| {
        ensure_project_and_graph(tx, tenant_id, project_id, false)?;
        let (sql, tier_param): (&str, Option<&str>) = match flow_tier {
            Some(tier) => (
                "SELECT node_id, project_id, substate, fixture_id, flow_tier, cluster_id, severity_band, updated_at \
                 FROM flow_node_substates WHERE project_id = ?1 AND flow_tier = ?2 \
                 ORDER BY cluster_id ASC, node_id ASC, substate ASC",
                Some(tier.as_db_str()),
            ),
            None => (
                "SELECT node_id, project_id, substate, fixture_id, flow_tier, cluster_id, severity_band, updated_at \
                 FROM flow_node_substates WHERE project_id = ?1 \
                 ORDER BY cluster_id ASC, node_id ASC, substate ASC",
                None,
            ),
        };

        let mut stmt = tx.prepare(sql).map_err(|e| {
            FlowServiceError::from_harness(map_sqlite_error(e, "prepare_list_substates"))
        })?;

        let map_row = |r: &rusqlite::Row<'_>| -> rusqlite::Result<StoredFlowSubstateDto> {
            let substate_raw: String = r.get(2)?;
            let tier_raw: String = r.get(4)?;
            let severity_raw: String = r.get(6)?;
            Ok(StoredFlowSubstateDto {
                node_id: r.get(0)?,
                project_id: r.get(1)?,
                substate: SubStateVariant::from_db_str(&substate_raw)
                    .unwrap_or(SubStateVariant::Default),
                fixture_id: r.get(3)?,
                flow_tier: FlowTier::from_db_str(&tier_raw).unwrap_or(FlowTier::Primary),
                cluster_id: r.get(5)?,
                severity_band: SeverityBand::from_db_str(&severity_raw)
                    .unwrap_or(SeverityBand::Low),
                updated_at: r.get(7)?,
            })
        };

        let rows = if let Some(tier_str) = tier_param {
            stmt.query_map(params![project_id, tier_str], map_row)
        } else {
            stmt.query_map(params![project_id], map_row)
        }
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "query_list_substates")))?;

        let mut out = Vec::new();
        for item in rows {
            out.push(item.map_err(|e| {
                FlowServiceError::from_harness(map_sqlite_error(e, "read_substate_row"))
            })?);
        }
        Ok(out)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_12_screens() -> Vec<ScreenSeedInput> {
        (1..=12)
            .map(|i| ScreenSeedInput {
                screen_key: format!("screen-{i:02}"),
                title: format!("Screen Step {i}"),
                route_path: Some(format!("/step-{i}")),
                persona_dimension: Some(SIMULATE_USER_8D[i % SIMULATE_USER_8D.len()].to_string()),
                fmea_failure_mode: Some(if i == 10 {
                    "Payment timeout".to_string()
                } else {
                    "Network error".to_string()
                }),
                severity_band: None,
            })
            .collect()
    }

    #[test]
    fn ut_05_5_01_twelve_screens_cluster_into_at_most_8_primary_nodes_with_100pct_substates() {
        let req = FlowSynthesizeRequest {
            project_id: "11111111-1111-4111-8111-111111111111".to_string(),
            brief_summary: "12-screen fintech checkout and onboarding suite".to_string(),
            max_default_cluster_nodes: 8,
            include_fmea_branches: true,
            screens: sample_12_screens(),
            candidate_edges: Vec::new(),
        };

        let plan = synthesize_3tier_flow_plan(&req).expect("synthesize");
        assert!(plan.summary.primary_cluster_count <= 8);
        assert_eq!(plan.summary.primary_cluster_count, 8);
        assert!(plan.summary.edge_case_branch_count >= 2);
        assert_eq!(plan.summary.substate_coverage_percent, 100);
        assert_eq!(plan.substates.len(), plan.nodes.len() * 4);
        assert_eq!(plan.summary.decision_kind, ConfidenceKind::ExactMatch);
    }

    #[test]
    fn ut_05_5_02_zero_raw_float_leaks_and_deterministic_timeout_fallback() {
        let req = FlowSynthesizeRequest {
            project_id: "11111111-1111-4111-8111-111111111111".to_string(),
            brief_summary: "Payment checkout with timeout fallback".to_string(),
            max_default_cluster_nodes: 8,
            include_fmea_branches: true,
            screens: Vec::new(),
            candidate_edges: Vec::new(),
        };

        let plan =
            synthesize_3tier_flow_plan_with_decision(&req, ConfidenceKind::DeterministicFallback)
                .expect("synthesize");
        assert_eq!(
            plan.summary.decision_kind,
            ConfidenceKind::DeterministicFallback
        );
        assert_eq!(plan.summary.float_leak_count, 0);

        let serialized = serde_json::to_value(&plan).expect("to_value");
        assert_eq!(count_json_floats(&serialized), 0);
    }

    #[test]
    fn ut_05_5_03_three_color_dfs_strips_orphans_rejects_cycles_and_preserves_loopbacks() {
        let req = FlowSynthesizeRequest {
            project_id: "11111111-1111-4111-8111-111111111111".to_string(),
            brief_summary: "Cycle guard regression check".to_string(),
            max_default_cluster_nodes: 4,
            include_fmea_branches: false,
            screens: vec![
                ScreenSeedInput {
                    screen_key: "step-a".to_string(),
                    title: "Step A".to_string(),
                    route_path: Some("/a".to_string()),
                    persona_dimension: None,
                    fmea_failure_mode: None,
                    severity_band: None,
                },
                ScreenSeedInput {
                    screen_key: "step-b".to_string(),
                    title: "Step B".to_string(),
                    route_path: Some("/b".to_string()),
                    persona_dimension: None,
                    fmea_failure_mode: None,
                    severity_band: None,
                },
                ScreenSeedInput {
                    screen_key: "step-c".to_string(),
                    title: "Step C".to_string(),
                    route_path: Some("/c".to_string()),
                    persona_dimension: None,
                    fmea_failure_mode: None,
                    severity_band: None,
                },
            ],
            candidate_edges: vec![
                // Orphan edge pointing to missing node
                RawEdgeCandidate {
                    source_node_id: "step-a".to_string(),
                    target_node_id: "ghost-node-999".to_string(),
                    flow_type: "main".to_string(),
                    trigger_event: "onGhost".to_string(),
                    guard_condition: None,
                    flow_tier: Some(FlowTier::Primary),
                },
                // Unguarded auto-cycle (step-c -> step-a with flow_type = main, sharing trigger with loopback below)
                RawEdgeCandidate {
                    source_node_id: "step-c".to_string(),
                    target_node_id: "step-a".to_string(),
                    flow_type: "main".to_string(),
                    trigger_event: "onBackToStart".to_string(),
                    guard_condition: None,
                    flow_tier: Some(FlowTier::Primary),
                },
                // Valid Story 05.1 user loopback edge (step-c -> step-a with flow_type = loopback)
                // Must NOT be suppressed by the rejected cycle edge above!
                RawEdgeCandidate {
                    source_node_id: "step-c".to_string(),
                    target_node_id: "step-a".to_string(),
                    flow_type: "loopback".to_string(),
                    trigger_event: "onBackToStart".to_string(),
                    guard_condition: Some("user_clicked_back".to_string()),
                    flow_tier: Some(FlowTier::EdgeCase),
                },
            ],
        };

        let plan = synthesize_3tier_flow_plan(&req).expect("synthesize");
        assert_eq!(plan.summary.stripped_orphan_edge_count, 1);
        assert_eq!(plan.summary.rejected_cycle_edge_count, 1);
        assert_eq!(plan.summary.preserved_loopback_edge_count, 1);
        assert!(plan
            .edges
            .iter()
            .any(|e| e.flow_type == "loopback" && e.trigger_event == "onBackToStart"));
        assert!(!plan
            .edges
            .iter()
            .any(|e| e.flow_type == "main" && e.trigger_event == "onBackToStart"));
        assert!(!plan.edges.iter().any(|e| e.trigger_event == "onGhost"));
    }

    #[test]
    fn ut_05_5_04_resolves_fmea_screen_keys_and_enforces_dos_bounds() {
        let req_fmea = FlowSynthesizeRequest {
            project_id: "11111111-1111-4111-8111-111111111111".to_string(),
            brief_summary: "FMEA key resolution check".to_string(),
            max_default_cluster_nodes: 4,
            include_fmea_branches: true,
            screens: vec![
                ScreenSeedInput {
                    screen_key: "step-a".to_string(),
                    title: "Step A".to_string(),
                    route_path: Some("/a".to_string()),
                    persona_dimension: None,
                    fmea_failure_mode: None,
                    severity_band: None,
                },
                ScreenSeedInput {
                    screen_key: "step-b".to_string(),
                    title: "Step B".to_string(),
                    route_path: Some("/b".to_string()),
                    persona_dimension: None,
                    fmea_failure_mode: None,
                    severity_band: None,
                },
            ],
            candidate_edges: vec![RawEdgeCandidate {
                source_node_id: "step-a".to_string(),
                target_node_id: "fmea-network-error-recovery".to_string(),
                flow_type: "if_not".to_string(),
                trigger_event: "onManualFmeaJump".to_string(),
                guard_condition: Some("offline == true".to_string()),
                flow_tier: Some(FlowTier::EdgeCase),
            }],
        };

        let plan = synthesize_3tier_flow_plan(&req_fmea).expect("synthesize");
        assert_eq!(plan.summary.stripped_orphan_edge_count, 0);
        assert!(plan
            .edges
            .iter()
            .any(|e| e.trigger_event == "onManualFmeaJump"));

        // DoS bound: >64 screens rejected
        let too_many_screens: Vec<ScreenSeedInput> = (0..65)
            .map(|idx| ScreenSeedInput {
                screen_key: format!("s-{idx}"),
                title: format!("Screen {idx}"),
                route_path: None,
                persona_dimension: None,
                fmea_failure_mode: None,
                severity_band: None,
            })
            .collect();
        let req_dos = FlowSynthesizeRequest {
            project_id: "11111111-1111-4111-8111-111111111111".to_string(),
            brief_summary: "Too many screens".to_string(),
            max_default_cluster_nodes: 8,
            include_fmea_branches: false,
            screens: too_many_screens,
            candidate_edges: Vec::new(),
        };
        assert!(validate_synthesize_request(&req_dos).is_err());

        // DoS bound: >4000 chars briefSummary rejected
        let req_long_brief = FlowSynthesizeRequest {
            project_id: "11111111-1111-4111-8111-111111111111".to_string(),
            brief_summary: "x".repeat(MAX_BRIEF_SUMMARY_LEN + 1),
            max_default_cluster_nodes: 8,
            include_fmea_branches: false,
            screens: Vec::new(),
            candidate_edges: Vec::new(),
        };
        assert!(validate_synthesize_request(&req_long_brief).is_err());
    }
}
