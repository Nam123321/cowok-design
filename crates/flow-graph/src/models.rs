// @story 04.1
// @implements AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
use domain::HarnessError;
use serde::{Deserialize, Serialize};

pub const MAX_FIXTURE_JSON_BYTES: usize = 32_768;
pub const MAX_BATCH_NODES: usize = 250;
pub const MAX_BATCH_EDGES: usize = 500;
pub const MIN_COORD: f64 = -50_000.0;
pub const MAX_COORD: f64 = 50_000.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowNodeDto {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub portal_id: String,
    pub screen_key: String,
    pub title: String,
    pub node_type: String,
    pub route_path: String,
    pub component_entry_path: String,
    pub fixture_ref: String,
    pub fixture_json: String,
    pub note: String,
    pub viewport_preset: String,
    pub pos_x: f64,
    pub pos_y: f64,
    pub version: i64,
    pub layout_version: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub portal_group: Option<String>,
    #[serde(default)]
    pub shared_app_shell_id: Option<String>,
    #[serde(default)]
    pub mini_skeleton_html: Option<String>,
    #[serde(default = "default_cta_pills")]
    pub cta_pills_json: String,
    #[serde(default = "default_revision")]
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowTransitionDto {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub flow_type: String,
    pub trigger_event: String,
    pub guard_condition: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub source_cta_ff_id: Option<String>,
    #[serde(default = "default_max_loop_iterations")]
    pub max_loop_iterations: i64,
    #[serde(default = "default_revision")]
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowGraphSnapshotDto {
    pub graph_id: String,
    pub graph_version: i64,
    pub layout_version: i64,
    #[serde(default = "default_revision")]
    pub revision: i64,
    pub nodes: Vec<FlowNodeDto>,
    pub transitions: Vec<FlowTransitionDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFlowNodeInput {
    pub id: Option<String>,
    #[serde(default = "default_portal")]
    pub portal_id: String,
    pub screen_key: String,
    pub title: String,
    #[serde(default = "default_node_type")]
    pub node_type: String,
    #[serde(default = "default_route_path")]
    pub route_path: String,
    #[serde(default)]
    pub component_entry_path: String,
    #[serde(default = "default_fixture_ref")]
    pub fixture_ref: String,
    #[serde(default = "default_fixture_json")]
    pub fixture_json: String,
    #[serde(default)]
    pub note: String,
    #[serde(default = "default_viewport")]
    pub viewport_preset: String,
    #[serde(default)]
    pub pos_x: f64,
    #[serde(default)]
    pub pos_y: f64,
    #[serde(default)]
    pub portal_group: Option<String>,
    #[serde(default)]
    pub shared_app_shell_id: Option<String>,
    #[serde(default)]
    pub mini_skeleton_html: Option<String>,
    #[serde(default = "default_cta_pills")]
    pub cta_pills_json: String,
    #[serde(default = "default_revision")]
    pub revision: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFlowNodeInput {
    pub title: Option<String>,
    pub portal_id: Option<String>,
    pub node_type: Option<String>,
    pub route_path: Option<String>,
    pub component_entry_path: Option<String>,
    pub fixture_ref: Option<String>,
    pub fixture_json: Option<String>,
    pub note: Option<String>,
    pub viewport_preset: Option<String>,
    pub pos_x: Option<f64>,
    pub pos_y: Option<f64>,
    pub expected_version: Option<i64>,
    pub expected_layout_version: Option<i64>,
    pub portal_group: Option<String>,
    pub shared_app_shell_id: Option<String>,
    pub mini_skeleton_html: Option<String>,
    pub cta_pills_json: Option<String>,
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPositionItem {
    pub node_id: String,
    pub pos_x: f64,
    pub pos_y: f64,
    pub expected_layout_version: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUpdatePositionsInput {
    pub positions: Vec<BatchPositionItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUpdatePositionsResult {
    pub layout_version: i64,
    pub nodes: Vec<FlowNodeDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFlowNodeResult {
    pub success: bool,
    pub id: String,
    pub graph_version: i64,
    pub cascade_deleted_edge_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFlowTransitionResult {
    pub success: bool,
    pub id: String,
    pub graph_version: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFlowTransitionInput {
    pub id: Option<String>,
    pub source_node_id: String,
    pub target_node_id: String,
    #[serde(default = "default_flow_type")]
    pub flow_type: String,
    #[serde(default = "default_trigger_event")]
    pub trigger_event: String,
    pub guard_condition: Option<String>,
    #[serde(default)]
    pub source_cta_ff_id: Option<String>,
    #[serde(default = "default_max_loop_iterations")]
    pub max_loop_iterations: i64,
    #[serde(default)]
    pub allow_confirmed_loopback: Option<bool>,
    #[serde(default = "default_revision")]
    pub revision: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFlowTransitionInput {
    pub flow_type: Option<String>,
    pub trigger_event: Option<String>,
    #[serde(default)]
    pub guard_condition: Option<Option<String>>,
    pub reversed: Option<bool>,
    pub expected_version: Option<i64>,
    pub source_cta_ff_id: Option<Option<String>>,
    pub max_loop_iterations: Option<i64>,
    pub allow_confirmed_loopback: Option<bool>,
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftNodeUpdateItem {
    pub node_id: String,
    pub patch: UpdateFlowNodeInput,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftNodeDeleteItem {
    pub node_id: String,
    pub expected_version: i64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftTransitionUpdateItem {
    pub transition_id: String,
    pub patch: UpdateFlowTransitionInput,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftTransitionDeleteItem {
    pub transition_id: String,
    pub expected_version: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployFlowDraftInput {
    pub draft_id: String,
    pub expected_graph_version: i64,
    #[serde(default)]
    pub created_nodes: Vec<CreateFlowNodeInput>,
    #[serde(default)]
    pub updated_nodes: Vec<DraftNodeUpdateItem>,
    #[serde(default)]
    pub deleted_nodes: Vec<DraftNodeDeleteItem>,
    #[serde(default)]
    pub updated_node_positions: Vec<BatchPositionItem>,
    #[serde(default)]
    pub created_transitions: Vec<CreateFlowTransitionInput>,
    #[serde(default)]
    pub updated_transitions: Vec<DraftTransitionUpdateItem>,
    #[serde(default)]
    pub deleted_transitions: Vec<DraftTransitionDeleteItem>,
    #[serde(default)]
    pub allow_confirmed_loopback: Option<bool>,
    #[serde(default)]
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct FlowServiceError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl FlowServiceError {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn from_harness(err: HarnessError) -> Self {
        match err {
            HarnessError::SqliteBusy { details } => Self::new(503, "REGISTRY_BUSY", details),
            HarnessError::ValidationFailed { reason }
                if reason.starts_with("UNIQUE_CONSTRAINT") =>
            {
                let code = if reason.contains("screen_key") {
                    "DUPLICATE_SCREEN_KEY"
                } else {
                    "DUPLICATE_TRANSITION"
                };
                Self::new(409, code, reason)
            }
            HarnessError::ValidationFailed { reason }
                if reason.starts_with("REFERENTIAL_INTEGRITY") =>
            {
                Self::new(422, "INVALID_NODE_REFERENCE", reason)
            }
            HarnessError::ValidationFailed { reason } if reason.starts_with("CHECK_CONSTRAINT") => {
                Self::new(400, "VALIDATION_ERROR", reason)
            }
            other => Self::new(400, "VALIDATION_ERROR", other.to_string()),
        }
    }
}

pub fn normalize_fixture_ref(raw: &str) -> Result<String, FlowServiceError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok("DEFAULT".to_string());
    }
    match trimmed.to_ascii_uppercase().as_str() {
        "DEFAULT" => Ok("DEFAULT".to_string()),
        "LOADING" => Ok("LOADING".to_string()),
        "EMPTY" => Ok("EMPTY".to_string()),
        "ERROR" => Ok("ERROR".to_string()),
        other => Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            format!("Invalid fixtureRef '{other}' (expected DEFAULT|LOADING|EMPTY|ERROR)"),
        )),
    }
}

pub fn normalize_viewport_preset(raw: &str) -> Result<String, FlowServiceError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok("desktop-1440".to_string());
    }
    match trimmed.to_ascii_lowercase().as_str() {
        "desktop" | "desktop-1440" => Ok("desktop-1440".to_string()),
        "tablet" | "tablet-768" => Ok("tablet-768".to_string()),
        "fold-inner" | "fold-inner-712" => Ok("fold-inner-712".to_string()),
        "mobile" | "mobile-390" => Ok("mobile-390".to_string()),
        "fold-outer" | "fold-outer-344" => Ok("fold-outer-344".to_string()),
        other => Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            format!(
                "Invalid viewportPreset '{other}' (expected desktop-1440|tablet-768|fold-inner-712|mobile-390|fold-outer-344)"
            ),
        )),
    }
}

fn default_portal() -> String {
    "default".to_string()
}
fn default_node_type() -> String {
    "screen".to_string()
}
fn default_route_path() -> String {
    "/".to_string()
}
fn default_fixture_ref() -> String {
    "DEFAULT".to_string()
}
fn default_fixture_json() -> String {
    "{}".to_string()
}
fn default_viewport() -> String {
    "desktop-1440".to_string()
}
fn default_flow_type() -> String {
    "main".to_string()
}
fn default_trigger_event() -> String {
    "onClick".to_string()
}
fn default_max_loop_iterations() -> i64 {
    3
}
fn default_revision() -> i64 {
    1
}
fn default_cta_pills() -> String {
    "[]".to_string()
}

impl Default for CreateFlowNodeInput {
    fn default() -> Self {
        Self {
            id: None,
            portal_id: default_portal(),
            screen_key: String::new(),
            title: String::new(),
            node_type: default_node_type(),
            route_path: default_route_path(),
            component_entry_path: String::new(),
            fixture_ref: default_fixture_ref(),
            fixture_json: default_fixture_json(),
            note: String::new(),
            viewport_preset: default_viewport(),
            pos_x: 0.0,
            pos_y: 0.0,
            portal_group: None,
            shared_app_shell_id: None,
            mini_skeleton_html: None,
            cta_pills_json: default_cta_pills(),
            revision: 1,
        }
    }
}

impl Default for CreateFlowTransitionInput {
    fn default() -> Self {
        Self {
            id: None,
            source_node_id: String::new(),
            target_node_id: String::new(),
            flow_type: default_flow_type(),
            trigger_event: default_trigger_event(),
            guard_condition: None,
            source_cta_ff_id: None,
            max_loop_iterations: default_max_loop_iterations(),
            allow_confirmed_loopback: None,
            revision: 1,
        }
    }
}

impl Default for DeployFlowDraftInput {
    fn default() -> Self {
        Self {
            draft_id: String::new(),
            expected_graph_version: 1,
            created_nodes: Vec::new(),
            updated_nodes: Vec::new(),
            deleted_nodes: Vec::new(),
            updated_node_positions: Vec::new(),
            created_transitions: Vec::new(),
            updated_transitions: Vec::new(),
            deleted_transitions: Vec::new(),
            allow_confirmed_loopback: None,
            expected_revision: None,
        }
    }
}
