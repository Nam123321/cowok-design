// @story 04.1
// @implements AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
use chrono::Utc;
use rusqlite::{params, Transaction};
use spec_registry::map_sqlite_error;
use uuid::Uuid;

use crate::cycle_detection::normalize_coord;
use crate::models::{
    CreateFlowNodeInput, FlowNodeDto, FlowServiceError, UpdateFlowNodeInput, MAX_FIXTURE_JSON_BYTES,
};
use crate::queries::{
    ensure_project_and_graph, load_node_by_id_in_tx, validate_node_payload, validate_node_type,
};

pub(crate) use crate::node_batch::{batch_positions_in_tx, delete_node_in_tx};

pub(crate) fn insert_node_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    graph_id: &str,
    input: &CreateFlowNodeInput,
    now_ms: i64,
) -> Result<String, FlowServiceError> {
    validate_node_payload(input)?;
    let node_id = match &input.id {
        Some(raw) if !raw.trim().is_empty() => Uuid::parse_str(raw.trim())
            .map_err(|_| {
                FlowServiceError::new(400, "VALIDATION_ERROR", "node id must be a valid UUID")
            })?
            .to_string(),
        _ => Uuid::new_v4().to_string(),
    };
    let pos_x = normalize_coord(input.pos_x, "posX")?;
    let pos_y = normalize_coord(input.pos_y, "posY")?;
    tx.execute(
        "INSERT INTO flow_nodes ( \
            id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type, \
            route_path, component_entry_path, fixture_ref, fixture_json, note, viewport_preset, \
            pos_x, pos_y, version, layout_version, created_at, updated_at \
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 1, 1, ?17, ?17)",
        params![
            node_id, graph_id, tenant_id, project_id, input.portal_id.trim(),
            input.screen_key.trim(), input.title.trim(), input.node_type.trim(),
            input.route_path.trim(), input.component_entry_path.trim(), input.fixture_ref.trim(),
            input.fixture_json, input.note, input.viewport_preset.trim(), pos_x, pos_y, now_ms
        ],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "insert_flow_node")))?;
    Ok(node_id)
}

pub(crate) fn apply_node_patch_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    node_id: &str,
    input: &UpdateFlowNodeInput,
    bump_graph: bool,
) -> Result<FlowNodeDto, FlowServiceError> {
    let hdr = ensure_project_and_graph(tx, tenant_id, project_id, true)?;
    let existing = load_node_by_id_in_tx(tx, tenant_id, project_id, node_id)?.ok_or_else(|| {
        FlowServiceError::new(
            404,
            "ENTITY_NOT_FOUND",
            format!("Node '{node_id}' not found"),
        )
    })?;

    let is_layout_only = input.expected_layout_version.is_some()
        && input.title.is_none()
        && input.portal_id.is_none()
        && input.node_type.is_none()
        && input.route_path.is_none()
        && input.component_entry_path.is_none()
        && input.fixture_ref.is_none()
        && input.fixture_json.is_none()
        && input.note.is_none()
        && input.viewport_preset.is_none();

    let now_ms = Utc::now().timestamp_millis();
    if is_layout_only {
        apply_layout_patch(
            tx, tenant_id, project_id, &hdr.id, &existing, input, now_ms, bump_graph,
        )?;
    } else {
        apply_semantic_patch(
            tx, tenant_id, project_id, &hdr.id, &existing, input, now_ms, bump_graph,
        )?;
    }

    load_node_by_id_in_tx(tx, tenant_id, project_id, node_id)?
        .ok_or_else(|| FlowServiceError::new(404, "ENTITY_NOT_FOUND", "Updated node missing"))
}

#[allow(clippy::too_many_arguments)]
fn apply_layout_patch(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    graph_id: &str,
    existing: &FlowNodeDto,
    input: &UpdateFlowNodeInput,
    now_ms: i64,
    bump_graph: bool,
) -> Result<(), FlowServiceError> {
    let exp_lv = input
        .expected_layout_version
        .unwrap_or(existing.layout_version);
    if existing.layout_version != exp_lv {
        return Err(FlowServiceError::new(
            409,
            "STALE_NODE_LAYOUT_VERSION",
            format!(
                "Node '{}' layoutVersion mismatch (expected {exp_lv}, actual {})",
                existing.id, existing.layout_version
            ),
        ));
    }
    let px = normalize_coord(input.pos_x.unwrap_or(existing.pos_x), "posX")?;
    let py = normalize_coord(input.pos_y.unwrap_or(existing.pos_y), "posY")?;
    tx.execute(
        "UPDATE flow_nodes SET pos_x = ?1, pos_y = ?2, layout_version = layout_version + 1, updated_at = ?3 \
         WHERE id = ?4 AND tenant_id = ?5 AND project_id = ?6 AND layout_version = ?7",
        params![px, py, now_ms, existing.id, tenant_id, project_id, exp_lv],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "update_node_layout")))?;
    if bump_graph {
        tx.execute(
            "UPDATE flow_graphs SET layout_version = layout_version + 1, updated_at = ?1 WHERE id = ?2",
            params![now_ms, graph_id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "bump_graph_lv")))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_semantic_patch(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    graph_id: &str,
    existing: &FlowNodeDto,
    input: &UpdateFlowNodeInput,
    now_ms: i64,
    bump_graph: bool,
) -> Result<(), FlowServiceError> {
    let exp_v = input.expected_version.unwrap_or(existing.version);
    if existing.version != exp_v {
        return Err(FlowServiceError::new(
            409,
            "STALE_NODE_VERSION",
            format!(
                "Node '{}' version mismatch (expected {exp_v}, actual {})",
                existing.id, existing.version
            ),
        ));
    }
    let title = input.title.as_deref().unwrap_or(&existing.title).trim();
    if title.is_empty() || title.len() > 100 {
        return Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            "title must be 1..=100 chars",
        ));
    }
    let node_type = validate_node_type(
        input
            .node_type
            .as_deref()
            .unwrap_or(&existing.node_type)
            .trim(),
    )?;
    let fixture_json = input
        .fixture_json
        .as_deref()
        .unwrap_or(&existing.fixture_json);
    if fixture_json.len() > MAX_FIXTURE_JSON_BYTES {
        return Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            "fixtureJson exceeds 32KB",
        ));
    }
    let px = normalize_coord(input.pos_x.unwrap_or(existing.pos_x), "posX")?;
    let py = normalize_coord(input.pos_y.unwrap_or(existing.pos_y), "posY")?;
    tx.execute(
        "UPDATE flow_nodes SET title = ?1, portal_id = ?2, node_type = ?3, route_path = ?4, \
         component_entry_path = ?5, fixture_ref = ?6, fixture_json = ?7, note = ?8, \
         viewport_preset = ?9, pos_x = ?10, pos_y = ?11, version = version + 1, updated_at = ?12 \
         WHERE id = ?13 AND tenant_id = ?14 AND project_id = ?15 AND version = ?16",
        params![
            title,
            input
                .portal_id
                .as_deref()
                .unwrap_or(&existing.portal_id)
                .trim(),
            node_type,
            input
                .route_path
                .as_deref()
                .unwrap_or(&existing.route_path)
                .trim(),
            input
                .component_entry_path
                .as_deref()
                .unwrap_or(&existing.component_entry_path)
                .trim(),
            input
                .fixture_ref
                .as_deref()
                .unwrap_or(&existing.fixture_ref)
                .trim(),
            fixture_json,
            input.note.as_deref().unwrap_or(&existing.note),
            input
                .viewport_preset
                .as_deref()
                .unwrap_or(&existing.viewport_preset)
                .trim(),
            px,
            py,
            now_ms,
            existing.id,
            tenant_id,
            project_id,
            exp_v
        ],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "update_node_semantic")))?;
    if bump_graph {
        tx.execute(
            "UPDATE flow_graphs SET version = version + 1, updated_at = ?1 WHERE id = ?2",
            params![now_ms, graph_id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "bump_graph_v")))?;
    }
    Ok(())
}
