// @story 11.2
// @implements AC-02 (Run-Scoped Outbound MCP Tool Bundles, Deterministic Banner Solver G1-G15 & Embedded Grounded Diagram Layered Solver)
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const MAX_NODES: usize = 256;
pub const MAX_EDGES: usize = 1024;
pub const MAX_BANNER_SLOTS: usize = 64;
const DEFAULT_NODE_WIDTH: f64 = 180.0;
const DEFAULT_NODE_HEIGHT: f64 = 64.0;
const DEFAULT_NODE_SPACING: f64 = 36.0;
const DEFAULT_LAYER_SPACING: f64 = 96.0;
const DIAGRAM_PADDING: f64 = 32.0;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SolverError {
    #[error("Unknown banner grid preset '{preset}'. Valid presets are G1 through G15.")]
    InvalidBannerPreset { preset: String },
    #[error("Invalid canvas dimensions ({width}x{height}) or spacing; width and height must be positive finite numbers.")]
    InvalidDimensions { width: String, height: String },
    #[error("Banner slot count {count} exceeds maximum allowed ({max}).")]
    MaxSlotsExceeded { count: usize, max: usize },
    #[error("Diagram node count {count} exceeds MAX_NODES DoS limit ({max}).")]
    MaxNodesExceeded { count: usize, max: usize },
    #[error("Diagram edge count {count} exceeds MAX_EDGES DoS limit ({max}).")]
    MaxEdgesExceeded { count: usize, max: usize },
    #[error("Duplicate node id '{id}' in diagram request.")]
    DuplicateNodeId { id: String },
    #[error("Edge references unknown node id '{id}'.")]
    UnknownEdgeEndpoint { id: String },
    #[error("Invalid layout direction '{direction}'. Use 'RIGHT' or 'DOWN'.")]
    InvalidDirection { direction: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunTaskKind {
    DesignReview,
    LayoutCompose,
    BannerGenerate,
    DiagramGround,
    SpecInspect,
    CodeSync,
    General,
}

impl RunTaskKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DesignReview => "design_review",
            Self::LayoutCompose => "layout_compose",
            Self::BannerGenerate => "banner_generate",
            Self::DiagramGround => "diagram_ground",
            Self::SpecInspect => "spec_inspect",
            Self::CodeSync => "code_sync",
            Self::General => "general",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "design_review" | "review" | "audit" => Self::DesignReview,
            "layout_compose" | "compose" | "ui_edit" | "batch" => Self::LayoutCompose,
            "banner_generate" | "banner" | "hero" | "marketing" => Self::BannerGenerate,
            "diagram_ground" | "diagram" | "architecture" | "elk" => Self::DiagramGround,
            "spec_inspect" | "inspect" | "read" => Self::SpecInspect,
            "code_sync" | "sync" | "handoff" => Self::CodeSync,
            _ => Self::General,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunScopedToolBundle {
    pub task_kind: String,
    pub allowed_tools: Vec<String>,
    pub built_in_de_tools: Vec<String>,
    pub dynamic_resources: Vec<String>,
    pub max_context_tokens: usize,
    pub excludes_human_only: bool,
}

/// Resolves a minimal, deterministic Run-Scoped Outbound MCP Tool Bundle for a given task kind.
/// Integrates with Story 08.5 built-in DE tools while strictly excluding all `HUMAN_ONLY` tools.
pub fn resolve_run_scoped_bundle(task_kind_raw: &str) -> RunScopedToolBundle {
    let kind = RunTaskKind::parse(task_kind_raw);
    let (allowed_tools, built_in_de_tools, dynamic_resources, max_context_tokens) = match kind {
        RunTaskKind::DesignReview => (
            vec!["get_app_state", "query_predicate"],
            vec!["de_canvas_inspect", "de_token_lint"],
            vec![
                "cowok-design://rules",
                "ui://cowok-design/brief-and-preview.html",
            ],
            4_000,
        ),
        RunTaskKind::LayoutCompose => (
            vec!["get_app_state", "query_predicate", "apply_batch"],
            vec!["de_canvas_inspect", "de_batch_apply", "de_token_lint"],
            vec![
                "cowok-design://rules",
                "cowok://skills/design-craft",
                "ui://cowok-design/brief-and-preview.html",
            ],
            6_000,
        ),
        RunTaskKind::BannerGenerate => (
            vec!["get_app_state", "banner_solver", "apply_batch"],
            vec!["de_canvas_inspect", "de_batch_apply"],
            vec!["cowok-design://rules", "cowok://skills/banner-solver"],
            5_000,
        ),
        RunTaskKind::DiagramGround => (
            vec!["get_app_state", "grounded_diagram_elk", "apply_batch"],
            vec!["de_repo_extract", "de_canvas_inspect"],
            vec!["cowok-design://rules", "cowok://skills/grounded-diagram"],
            6_000,
        ),
        RunTaskKind::SpecInspect => (
            vec!["get_app_state", "query_predicate", "resolve_run_bundle"],
            vec!["de_canvas_inspect"],
            vec!["cowok-design://rules"],
            3_000,
        ),
        RunTaskKind::CodeSync => (
            vec!["get_app_state", "query_predicate", "grounded_diagram_elk"],
            vec!["de_repo_extract", "de_token_lint"],
            vec!["cowok-design://rules", "cowok://skills/design-craft"],
            5_000,
        ),
        RunTaskKind::General => (
            vec![
                "get_app_state",
                "query_predicate",
                "apply_batch",
                "banner_solver",
                "grounded_diagram_elk",
                "resolve_run_bundle",
            ],
            vec![
                "de_canvas_inspect",
                "de_token_lint",
                "de_batch_apply",
                "de_repo_extract",
            ],
            vec![
                "cowok-design://rules",
                "cowok://skills/design-craft",
                "ui://cowok-design/brief-and-preview.html",
            ],
            8_000,
        ),
    };

    RunScopedToolBundle {
        task_kind: kind.as_str().to_string(),
        allowed_tools: allowed_tools.into_iter().map(str::to_string).collect(),
        built_in_de_tools: built_in_de_tools.into_iter().map(str::to_string).collect(),
        dynamic_resources: dynamic_resources.into_iter().map(str::to_string).collect(),
        max_context_tokens,
        excludes_human_only: true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridCellSpec {
    col: u32,
    row: u32,
    col_span: u32,
    row_span: u32,
}

#[derive(Debug, Clone, Copy)]
struct BannerPresetSpec {
    id: &'static str,
    name: &'static str,
    cols: u32,
    rows: u32,
    cells: &'static [GridCellSpec],
}

const fn cell(col: u32, row: u32, col_span: u32, row_span: u32) -> GridCellSpec {
    GridCellSpec {
        col,
        row,
        col_span,
        row_span,
    }
}

const BANNER_PRESETS: &[BannerPresetSpec] = &[
    BannerPresetSpec {
        id: "G1",
        name: "Hero Centered Single Focal",
        cols: 12,
        rows: 6,
        cells: &[cell(1, 0, 10, 4), cell(3, 4, 6, 2)],
    },
    BannerPresetSpec {
        id: "G2",
        name: "Balanced Split 50/50",
        cols: 12,
        rows: 6,
        cells: &[cell(0, 0, 6, 6), cell(6, 0, 6, 6)],
    },
    BannerPresetSpec {
        id: "G3",
        name: "Asymmetric Editorial 60/40",
        cols: 10,
        rows: 6,
        cells: &[cell(0, 0, 6, 6), cell(6, 0, 4, 6)],
    },
    BannerPresetSpec {
        id: "G4",
        name: "Asymmetric Media Lead 40/60",
        cols: 10,
        rows: 6,
        cells: &[cell(0, 0, 4, 6), cell(4, 0, 6, 6)],
    },
    BannerPresetSpec {
        id: "G5",
        name: "3-Column Feature Bento",
        cols: 12,
        rows: 6,
        cells: &[cell(0, 0, 4, 6), cell(4, 0, 4, 6), cell(8, 0, 4, 6)],
    },
    BannerPresetSpec {
        id: "G6",
        name: "Masthead + Dual Split",
        cols: 12,
        rows: 6,
        cells: &[cell(0, 0, 12, 2), cell(0, 2, 6, 4), cell(6, 2, 6, 4)],
    },
    BannerPresetSpec {
        id: "G7",
        name: "Left Rail + Stage + Footer",
        cols: 12,
        rows: 6,
        cells: &[cell(0, 0, 3, 6), cell(3, 0, 9, 4), cell(3, 4, 9, 2)],
    },
    BannerPresetSpec {
        id: "G8",
        name: "2x2 Quad Metric Grid",
        cols: 12,
        rows: 6,
        cells: &[
            cell(0, 0, 6, 3),
            cell(6, 0, 6, 3),
            cell(0, 3, 6, 3),
            cell(6, 3, 6, 3),
        ],
    },
    BannerPresetSpec {
        id: "G9",
        name: "Editorial Stepped Pair",
        cols: 12,
        rows: 6,
        cells: &[
            cell(0, 0, 7, 3),
            cell(7, 0, 5, 3),
            cell(0, 3, 5, 3),
            cell(5, 3, 7, 3),
        ],
    },
    BannerPresetSpec {
        id: "G10",
        name: "SaaS Masthead + 3 KPI Tiles",
        cols: 12,
        rows: 6,
        cells: &[
            cell(0, 0, 12, 3),
            cell(0, 3, 4, 3),
            cell(4, 3, 4, 3),
            cell(8, 3, 4, 3),
        ],
    },
    BannerPresetSpec {
        id: "G11",
        name: "Product Showcase + Stacked Spec Rail",
        cols: 12,
        rows: 6,
        cells: &[
            cell(0, 0, 8, 6),
            cell(8, 0, 4, 2),
            cell(8, 2, 4, 2),
            cell(8, 4, 4, 2),
        ],
    },
    BannerPresetSpec {
        id: "G12",
        name: "Stacked 3-Tier Ribbon",
        cols: 12,
        rows: 6,
        cells: &[cell(0, 0, 12, 2), cell(0, 2, 12, 2), cell(0, 4, 12, 2)],
    },
    BannerPresetSpec {
        id: "G13",
        name: "Spotlight + 3 Bottom Thumbnails",
        cols: 12,
        rows: 6,
        cells: &[
            cell(0, 0, 12, 4),
            cell(0, 4, 4, 2),
            cell(4, 4, 4, 2),
            cell(8, 4, 4, 2),
        ],
    },
    BannerPresetSpec {
        id: "G14",
        name: "Social 16:9 Split CTA Banner",
        cols: 12,
        rows: 6,
        cells: &[cell(0, 0, 7, 4), cell(0, 4, 7, 2), cell(7, 0, 5, 6)],
    },
    BannerPresetSpec {
        id: "G15",
        name: "Presentation 16:9 Cover + 4-Slot KPI Strip",
        cols: 12,
        rows: 6,
        cells: &[
            cell(0, 0, 12, 4),
            cell(0, 4, 3, 2),
            cell(3, 4, 3, 2),
            cell(6, 4, 3, 2),
            cell(9, 4, 3, 2),
        ],
    },
];

pub fn list_banner_presets() -> Vec<(&'static str, &'static str)> {
    BANNER_PRESETS.iter().map(|p| (p.id, p.name)).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BannerSlotInput {
    pub id: String,
    #[serde(default = "default_slot_role")]
    pub role: String,
}

fn default_slot_role() -> String {
    "content".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BannerSolveRequest {
    pub preset: String,
    #[serde(default = "default_banner_width")]
    pub width: f64,
    #[serde(default = "default_banner_height")]
    pub height: f64,
    #[serde(default = "default_banner_padding")]
    pub padding: f64,
    #[serde(default = "default_banner_gap")]
    pub gap: f64,
    #[serde(default)]
    pub slots: Vec<BannerSlotInput>,
}

fn default_banner_width() -> f64 {
    1200.0
}
fn default_banner_height() -> f64 {
    630.0
}
fn default_banner_padding() -> f64 {
    32.0
}
fn default_banner_gap() -> f64 {
    16.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BannerSlotPlacement {
    pub id: String,
    pub role: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub col_span: u32,
    pub row_span: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BannerSolveResult {
    pub preset: String,
    pub preset_name: String,
    pub grid_cols: u32,
    pub grid_rows: u32,
    pub canvas_width: f64,
    pub canvas_height: f64,
    pub placements: Vec<BannerSlotPlacement>,
    pub collision_free: bool,
    pub deterministic_hash: String,
}

fn round2(val: f64) -> f64 {
    (val * 100.0).round() / 100.0
}

fn rectangles_overlap(a: &BannerSlotPlacement, b: &BannerSlotPlacement) -> bool {
    const EPS: f64 = 0.01;
    let x_overlap = a.x + a.width > b.x + EPS && b.x + b.width > a.x + EPS;
    let y_overlap = a.y + a.height > b.y + EPS && b.y + b.height > a.y + EPS;
    x_overlap && y_overlap
}

fn sha256_hex_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

/// Deterministic Banner Solver G1–G15 producing guaranteed collision-free coordinates.
pub fn solve_banner_grid(req: &BannerSolveRequest) -> Result<BannerSolveResult, SolverError> {
    let preset_key = req.preset.trim().to_ascii_uppercase();
    let spec = BANNER_PRESETS
        .iter()
        .find(|p| p.id == preset_key)
        .ok_or_else(|| SolverError::InvalidBannerPreset {
            preset: req.preset.clone(),
        })?;

    if !req.width.is_finite()
        || !req.height.is_finite()
        || !req.padding.is_finite()
        || !req.gap.is_finite()
        || req.width <= 0.0
        || req.height <= 0.0
        || req.padding < 0.0
        || req.gap < 0.0
    {
        return Err(SolverError::InvalidDimensions {
            width: req.width.to_string(),
            height: req.height.to_string(),
        });
    }

    if req.slots.len() > MAX_BANNER_SLOTS {
        return Err(SolverError::MaxSlotsExceeded {
            count: req.slots.len(),
            max: MAX_BANNER_SLOTS,
        });
    }

    let inner_w = (req.width - req.padding * 2.0).max(1.0);
    let inner_h = (req.height - req.padding * 2.0).max(1.0);
    let total_gap_x = req.gap * f64::from(spec.cols.saturating_sub(1));
    let total_gap_y = req.gap * f64::from(spec.rows.saturating_sub(1));
    let col_unit = ((inner_w - total_gap_x) / f64::from(spec.cols)).max(1.0);
    let row_unit = ((inner_h - total_gap_y) / f64::from(spec.rows)).max(1.0);

    let effective_slots: Vec<BannerSlotInput> = if req.slots.is_empty() {
        (0..spec.cells.len())
            .map(|idx| BannerSlotInput {
                id: format!("slot-{}", idx + 1),
                role: if idx == 0 {
                    "primary".to_string()
                } else {
                    "secondary".to_string()
                },
            })
            .collect()
    } else {
        req.slots.clone()
    };

    let mut placements = Vec::with_capacity(effective_slots.len().min(spec.cells.len()));
    for (idx, cell_spec) in spec.cells.iter().enumerate() {
        let Some(slot) = effective_slots.get(idx) else {
            break;
        };
        let x = req.padding + f64::from(cell_spec.col) * (col_unit + req.gap);
        let y = req.padding + f64::from(cell_spec.row) * (row_unit + req.gap);
        let w = f64::from(cell_spec.col_span) * col_unit
            + f64::from(cell_spec.col_span.saturating_sub(1)) * req.gap;
        let h = f64::from(cell_spec.row_span) * row_unit
            + f64::from(cell_spec.row_span.saturating_sub(1)) * req.gap;

        placements.push(BannerSlotPlacement {
            id: slot.id.clone(),
            role: slot.role.clone(),
            x: round2(x),
            y: round2(y),
            width: round2(w),
            height: round2(h),
            col_span: cell_spec.col_span,
            row_span: cell_spec.row_span,
        });
    }

    let mut collision_free = true;
    'outer: for i in 0..placements.len() {
        for j in (i + 1)..placements.len() {
            if rectangles_overlap(&placements[i], &placements[j]) {
                collision_free = false;
                break 'outer;
            }
        }
    }

    let deterministic_hash = sha256_hex_json(&(spec.id, &placements));
    Ok(BannerSolveResult {
        preset: spec.id.to_string(),
        preset_name: spec.name.to_string(),
        grid_cols: spec.cols,
        grid_rows: spec.rows,
        canvas_width: round2(req.width),
        canvas_height: round2(req.height),
        placements,
        collision_free,
        deterministic_hash,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramNodeInput {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub source_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramEdgeInput {
    #[serde(default)]
    pub id: Option<String>,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundedDiagramRequest {
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub node_spacing: Option<f64>,
    #[serde(default)]
    pub layer_spacing: Option<f64>,
    pub nodes: Vec<DiagramNodeInput>,
    #[serde(default)]
    pub edges: Vec<DiagramEdgeInput>,
    #[serde(default)]
    pub repo_extract_symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionedDiagramNode {
    pub id: String,
    pub label: String,
    pub layer: usize,
    pub order: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub grounded: bool,
    pub source_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedDiagramEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    pub points: Vec<DiagramPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundedDiagramResult {
    pub engine: String,
    pub direction: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub layer_count: usize,
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<PositionedDiagramNode>,
    pub edges: Vec<RoutedDiagramEdge>,
    pub deterministic_hash: String,
}

/// Embedded deterministic Sugiyama/ELK-compatible layered layout solver.
/// Enforces `MAX_NODES = 256` and `MAX_EDGES = 1024` DoS caps with zero external Node runtime dependency.
pub fn solve_grounded_diagram(
    req: &GroundedDiagramRequest,
) -> Result<GroundedDiagramResult, SolverError> {
    if req.nodes.len() > MAX_NODES {
        return Err(SolverError::MaxNodesExceeded {
            count: req.nodes.len(),
            max: MAX_NODES,
        });
    }
    if req.edges.len() > MAX_EDGES {
        return Err(SolverError::MaxEdgesExceeded {
            count: req.edges.len(),
            max: MAX_EDGES,
        });
    }

    let direction = req
        .direction
        .as_deref()
        .unwrap_or("RIGHT")
        .trim()
        .to_ascii_uppercase();
    if direction != "RIGHT" && direction != "DOWN" {
        return Err(SolverError::InvalidDirection { direction });
    }

    let node_spacing = req
        .node_spacing
        .unwrap_or(DEFAULT_NODE_SPACING)
        .clamp(8.0, 240.0);
    let layer_spacing = req
        .layer_spacing
        .unwrap_or(DEFAULT_LAYER_SPACING)
        .clamp(16.0, 320.0);

    let mut node_map: BTreeMap<String, &DiagramNodeInput> = BTreeMap::new();
    for node in &req.nodes {
        if node_map.insert(node.id.clone(), node).is_some() {
            return Err(SolverError::DuplicateNodeId {
                id: node.id.clone(),
            });
        }
    }

    let mut outgoing: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut indegree: BTreeMap<String, usize> = BTreeMap::new();
    for id in node_map.keys() {
        outgoing.insert(id.clone(), Vec::new());
        indegree.insert(id.clone(), 0);
    }

    for edge in &req.edges {
        if !node_map.contains_key(&edge.source) {
            return Err(SolverError::UnknownEdgeEndpoint {
                id: edge.source.clone(),
            });
        }
        if !node_map.contains_key(&edge.target) {
            return Err(SolverError::UnknownEdgeEndpoint {
                id: edge.target.clone(),
            });
        }
        if edge.source != edge.target {
            if let Some(list) = outgoing.get_mut(&edge.source) {
                list.push(edge.target.clone());
            }
            if let Some(deg) = indegree.get_mut(&edge.target) {
                *deg += 1;
            }
        }
    }

    // Deterministic Kahn topological layering with cycle-breaking fallback
    let mut layers: BTreeMap<String, usize> = BTreeMap::new();
    let mut queue: VecDeque<String> = indegree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut visited: BTreeSet<String> = BTreeSet::new();

    while visited.len() < node_map.len() {
        if queue.is_empty() {
            if let Some(next_unvisited) = node_map.keys().find(|id| !visited.contains(*id)).cloned()
            {
                queue.push_back(next_unvisited);
            }
        }
        while let Some(curr) = queue.pop_front() {
            if !visited.insert(curr.clone()) {
                continue;
            }
            let curr_layer = *layers.get(&curr).unwrap_or(&0);
            layers.entry(curr.clone()).or_insert(curr_layer);
            if let Some(children) = outgoing.get(&curr) {
                for child in children {
                    if !visited.contains(child) {
                        let entry = layers.entry(child.clone()).or_insert(0);
                        if *entry <= curr_layer {
                            *entry = curr_layer + 1;
                        }
                        if let Some(deg) = indegree.get_mut(child) {
                            *deg = deg.saturating_sub(1);
                            if *deg == 0 {
                                queue.push_back(child.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    let mut by_layer: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (id, layer) in &layers {
        by_layer.entry(*layer).or_default().push(id.clone());
    }
    for ids in by_layer.values_mut() {
        ids.sort();
    }

    let symbol_set: BTreeSet<String> = req
        .repo_extract_symbols
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();

    let mut positioned_map: BTreeMap<String, PositionedDiagramNode> = BTreeMap::new();
    let mut max_x: f64 = DIAGRAM_PADDING * 2.0;
    let mut max_y: f64 = DIAGRAM_PADDING * 2.0;
    let mut major_cursor = DIAGRAM_PADDING;

    for (layer_idx, ids) in &by_layer {
        let mut minor_cursor = DIAGRAM_PADDING;
        let mut layer_major_extent: f64 = 0.0;
        for (order_idx, id) in ids.iter().enumerate() {
            let input = node_map[id];
            let w = input.width.unwrap_or(DEFAULT_NODE_WIDTH).clamp(48.0, 800.0);
            let h = input
                .height
                .unwrap_or(DEFAULT_NODE_HEIGHT)
                .clamp(28.0, 400.0);
            let (x, y) = if direction == "RIGHT" {
                (major_cursor, minor_cursor)
            } else {
                (minor_cursor, major_cursor)
            };
            let label = if input.label.trim().is_empty() {
                input.id.clone()
            } else {
                input.label.clone()
            };
            let grounded = input.source_file.is_some()
                || symbol_set.contains(&input.id.to_ascii_lowercase())
                || symbol_set.contains(&label.to_ascii_lowercase());

            max_x = max_x.max(x + w + DIAGRAM_PADDING);
            max_y = max_y.max(y + h + DIAGRAM_PADDING);

            positioned_map.insert(
                id.clone(),
                PositionedDiagramNode {
                    id: id.clone(),
                    label,
                    layer: *layer_idx,
                    order: order_idx,
                    x: round2(x),
                    y: round2(y),
                    width: round2(w),
                    height: round2(h),
                    grounded,
                    source_file: input.source_file.clone(),
                },
            );

            if direction == "RIGHT" {
                minor_cursor += h + node_spacing;
                layer_major_extent = layer_major_extent.max(w);
            } else {
                minor_cursor += w + node_spacing;
                layer_major_extent = layer_major_extent.max(h);
            }
        }
        major_cursor += layer_major_extent + layer_spacing;
    }

    let nodes: Vec<PositionedDiagramNode> = positioned_map.values().cloned().collect();
    let mut edges = Vec::with_capacity(req.edges.len());
    for (idx, edge) in req.edges.iter().enumerate() {
        let src = &positioned_map[&edge.source];
        let dst = &positioned_map[&edge.target];
        let (p0, p3) = if direction == "RIGHT" {
            (
                DiagramPoint {
                    x: round2(src.x + src.width),
                    y: round2(src.y + src.height / 2.0),
                },
                DiagramPoint {
                    x: round2(dst.x),
                    y: round2(dst.y + dst.height / 2.0),
                },
            )
        } else {
            (
                DiagramPoint {
                    x: round2(src.x + src.width / 2.0),
                    y: round2(src.y + src.height),
                },
                DiagramPoint {
                    x: round2(dst.x + dst.width / 2.0),
                    y: round2(dst.y),
                },
            )
        };
        let mid_x = round2((p0.x + p3.x) / 2.0);
        let mid_y = round2((p0.y + p3.y) / 2.0);
        let points = if direction == "RIGHT" {
            vec![
                p0.clone(),
                DiagramPoint { x: mid_x, y: p0.y },
                DiagramPoint { x: mid_x, y: p3.y },
                p3,
            ]
        } else {
            vec![
                p0.clone(),
                DiagramPoint { x: p0.x, y: mid_y },
                DiagramPoint { x: p3.x, y: mid_y },
                p3,
            ]
        };
        edges.push(RoutedDiagramEdge {
            id: edge
                .id
                .clone()
                .unwrap_or_else(|| format!("edge-{}-{}", idx + 1, edge.source)),
            source: edge.source.clone(),
            target: edge.target.clone(),
            label: edge.label.clone(),
            points,
        });
    }

    let deterministic_hash = sha256_hex_json(&(&direction, &nodes, &edges));
    Ok(GroundedDiagramResult {
        engine: "cowok-elk-embedded-rust".to_string(),
        direction,
        node_count: nodes.len(),
        edge_count: edges.len(),
        layer_count: by_layer.len(),
        width: round2(max_x),
        height: round2(max_y),
        nodes,
        edges,
        deterministic_hash,
    })
}
