// @story 04.1
// @implements AC-03, AC-04, AC-06, AC-07, AC-08
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};
use spec_registry::map_sqlite_error;
use uuid::Uuid;

use crate::models::{
    CreateFlowNodeInput, FlowGraphSnapshotDto, FlowNodeDto, FlowServiceError, FlowTransitionDto,
    MAX_FIXTURE_JSON_BYTES,
};

#[derive(Debug, Clone)]
pub(crate) struct GraphHeader {
    pub id: String,
    pub version: i64,
    pub layout_version: i64,
}

pub(crate) fn validate_node_type(node_type: &str) -> Result<&str, FlowServiceError> {
    match node_type {
        "screen" | "modal" | "condition" | "action" => Ok(node_type),
        other => Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            format!("Invalid nodeType '{other}' (expected screen|modal|condition|action)"),
        )),
    }
}

pub(crate) fn validate_flow_type(flow_type: &str) -> Result<&str, FlowServiceError> {
    match flow_type {
        "main" | "if_else" | "if_not" => Ok(flow_type),
        other => Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            format!("Invalid flowType '{other}' (expected main|if_else|if_not)"),
        )),
    }
}

pub(crate) fn validate_node_payload(input: &CreateFlowNodeInput) -> Result<(), FlowServiceError> {
    if input.screen_key.trim().is_empty()
        || input.title.trim().is_empty()
        || input.title.len() > 100
    {
        return Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            "screenKey and title (1..=100 chars) are required",
        ));
    }
    validate_node_type(input.node_type.trim())?;
    if input.fixture_json.len() > MAX_FIXTURE_JSON_BYTES {
        return Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            format!("fixtureJson exceeds maximum size of {MAX_FIXTURE_JSON_BYTES} bytes"),
        ));
    }
    Ok(())
}

/// Verifies `(tenant_id, project_id)` exists in `projects` (returning uniform `404 PROJECT_NOT_FOUND`
/// on foreign or missing project) and returns the default `flow_graphs` header, creating it if absent.
pub(crate) fn ensure_project_and_graph(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    allow_create: bool,
) -> Result<GraphHeader, FlowServiceError> {
    let exists: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM projects WHERE id = ?1 AND tenant_id = ?2",
            params![project_id, tenant_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "check_project_tenant")))?;

    if exists.is_none() {
        return Err(FlowServiceError::new(
            404,
            "PROJECT_NOT_FOUND",
            format!("Project '{project_id}' was not found"),
        ));
    }

    let header_opt: Option<GraphHeader> = tx
        .query_row(
            "SELECT id, version, layout_version FROM flow_graphs \
             WHERE tenant_id = ?1 AND project_id = ?2 AND portal_key = 'default'",
            params![tenant_id, project_id],
            |r| {
                Ok(GraphHeader {
                    id: r.get(0)?,
                    version: r.get(1)?,
                    layout_version: r.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "load_flow_graph")))?;

    if let Some(hdr) = header_opt {
        return Ok(hdr);
    }
    if !allow_create {
        return Ok(GraphHeader {
            id: format!("virtual-{project_id}"),
            version: 1,
            layout_version: 1,
        });
    }

    let gid = Uuid::new_v4().to_string();
    let now_ms = Utc::now().timestamp_millis();
    tx.execute(
        "INSERT INTO flow_graphs (id, tenant_id, project_id, portal_key, title, version, layout_version, updated_at) \
         VALUES (?1, ?2, ?3, 'default', 'Default Flow', 1, 1, ?4)",
        params![gid, tenant_id, project_id, now_ms],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "insert_default_flow_graph")))?;

    Ok(GraphHeader {
        id: gid,
        version: 1,
        layout_version: 1,
    })
}

pub(crate) fn load_nodes_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
) -> Result<Vec<FlowNodeDto>, FlowServiceError> {
    let mut stmt = tx
        .prepare(
            "SELECT id, tenant_id, project_id, portal_id, screen_key, title, node_type, \
                    route_path, component_entry_path, fixture_ref, fixture_json, note, \
                    viewport_preset, pos_x, pos_y, version, layout_version, created_at, updated_at \
             FROM flow_nodes WHERE tenant_id = ?1 AND project_id = ?2 ORDER BY created_at ASC, id ASC",
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "prepare_load_nodes")))?;

    let rows = stmt
        .query_map(params![tenant_id, project_id], map_node_row)
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "query_load_nodes")))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(
            r.map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "read_node_row")))?,
        );
    }
    Ok(out)
}

pub(crate) fn load_node_by_id_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    node_id: &str,
) -> Result<Option<FlowNodeDto>, FlowServiceError> {
    tx.query_row(
        "SELECT id, tenant_id, project_id, portal_id, screen_key, title, node_type, \
                route_path, component_entry_path, fixture_ref, fixture_json, note, \
                viewport_preset, pos_x, pos_y, version, layout_version, created_at, updated_at \
         FROM flow_nodes WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3",
        params![node_id, tenant_id, project_id],
        map_node_row,
    )
    .optional()
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "load_node_by_id")))
}

pub(crate) fn load_transitions_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
) -> Result<Vec<FlowTransitionDto>, FlowServiceError> {
    let mut stmt = tx
        .prepare(
            "SELECT id, tenant_id, project_id, source_node_id, target_node_id, flow_type, \
                    trigger_event, guard_condition, version, created_at, updated_at \
             FROM flow_edges WHERE tenant_id = ?1 AND project_id = ?2 ORDER BY created_at ASC, id ASC",
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "prepare_load_edges")))?;

    let rows = stmt
        .query_map(params![tenant_id, project_id], map_transition_row)
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "query_load_edges")))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(
            r.map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "read_edge_row")))?,
        );
    }
    Ok(out)
}

pub(crate) fn load_snapshot_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    allow_create: bool,
) -> Result<FlowGraphSnapshotDto, FlowServiceError> {
    let hdr = ensure_project_and_graph(tx, tenant_id, project_id, allow_create)?;
    let nodes = load_nodes_in_tx(tx, tenant_id, project_id)?;
    let transitions = load_transitions_in_tx(tx, tenant_id, project_id)?;
    Ok(FlowGraphSnapshotDto {
        graph_id: hdr.id,
        graph_version: hdr.version,
        layout_version: hdr.layout_version,
        nodes,
        transitions,
    })
}

pub(crate) fn map_node_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<FlowNodeDto> {
    let created_ms: i64 = r.get(17)?;
    let updated_ms: i64 = r.get(18)?;
    Ok(FlowNodeDto {
        id: r.get(0)?,
        tenant_id: r.get(1)?,
        project_id: r.get(2)?,
        portal_id: r.get(3)?,
        screen_key: r.get(4)?,
        title: r.get(5)?,
        node_type: r.get(6)?,
        route_path: r.get(7)?,
        component_entry_path: r.get(8)?,
        fixture_ref: r.get(9)?,
        fixture_json: r.get(10)?,
        note: r.get(11)?,
        viewport_preset: r.get(12)?,
        pos_x: r.get(13)?,
        pos_y: r.get(14)?,
        version: r.get(15)?,
        layout_version: r.get(16)?,
        created_at: created_ms.to_string(),
        updated_at: updated_ms.to_string(),
    })
}

pub(crate) fn map_transition_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<FlowTransitionDto> {
    let created_ms: i64 = r.get(9)?;
    let updated_ms: i64 = r.get(10)?;
    Ok(FlowTransitionDto {
        id: r.get(0)?,
        tenant_id: r.get(1)?,
        project_id: r.get(2)?,
        source_node_id: r.get(3)?,
        target_node_id: r.get(4)?,
        flow_type: r.get(5)?,
        trigger_event: r.get(6)?,
        guard_condition: r.get(7)?,
        version: r.get(8)?,
        created_at: created_ms.to_string(),
        updated_at: updated_ms.to_string(),
    })
}
