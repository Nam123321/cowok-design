// @story 04.1
// @implements AC-04, AC-06, AC-08
use chrono::Utc;
use rusqlite::{params, Transaction};
use spec_registry::map_sqlite_error;

use crate::cycle_detection::normalize_coord;
use crate::models::{
    BatchPositionItem, BatchUpdatePositionsResult, DeleteFlowNodeResult, FlowServiceError,
    MAX_BATCH_NODES,
};
use crate::queries::{ensure_project_and_graph, load_node_by_id_in_tx, load_nodes_in_tx};

pub(crate) fn batch_positions_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    items: &[BatchPositionItem],
    bump_graph: bool,
) -> Result<BatchUpdatePositionsResult, FlowServiceError> {
    if items.len() > MAX_BATCH_NODES {
        return Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            "Batch exceeds 250 nodes",
        ));
    }
    let hdr = ensure_project_and_graph(tx, tenant_id, project_id, true)?;
    let now_ms = Utc::now().timestamp_millis();

    for item in items {
        let existing = load_node_by_id_in_tx(tx, tenant_id, project_id, &item.node_id)?
            .ok_or_else(|| {
                FlowServiceError::new(
                    404,
                    "ENTITY_NOT_FOUND",
                    format!("Node '{}' not found", item.node_id),
                )
            })?;
        if existing.layout_version != item.expected_layout_version {
            return Err(FlowServiceError::new(
                409,
                "STALE_NODE_LAYOUT_VERSION",
                "Node layoutVersion mismatch",
            ));
        }
        let px = normalize_coord(item.pos_x, "posX")?;
        let py = normalize_coord(item.pos_y, "posY")?;
        tx.execute(
            "UPDATE flow_nodes SET pos_x = ?1, pos_y = ?2, layout_version = layout_version + 1, updated_at = ?3 \
             WHERE id = ?4 AND tenant_id = ?5 AND project_id = ?6 AND layout_version = ?7",
            params![px, py, now_ms, item.node_id, tenant_id, project_id, item.expected_layout_version],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "batch_update_pos")))?;
    }

    let next_lv = if bump_graph && !items.is_empty() {
        tx.execute(
            "UPDATE flow_graphs SET layout_version = layout_version + 1, updated_at = ?1 WHERE id = ?2",
            params![now_ms, hdr.id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "bump_batch_layout")))?;
        hdr.layout_version + 1
    } else {
        hdr.layout_version
    };
    Ok(BatchUpdatePositionsResult {
        layout_version: next_lv,
        nodes: load_nodes_in_tx(tx, tenant_id, project_id)?,
    })
}

pub(crate) fn delete_node_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    node_id: &str,
    expected_version: Option<i64>,
    expected_graph_version: Option<i64>,
    bump_graph: bool,
) -> Result<DeleteFlowNodeResult, FlowServiceError> {
    let hdr = ensure_project_and_graph(tx, tenant_id, project_id, true)?;
    if let Some(exp_gv) = expected_graph_version {
        if hdr.version != exp_gv {
            return Err(FlowServiceError::new(
                409,
                "STALE_GRAPH_VERSION",
                "Graph version mismatch",
            ));
        }
    }
    let existing = load_node_by_id_in_tx(tx, tenant_id, project_id, node_id)?.ok_or_else(|| {
        FlowServiceError::new(
            404,
            "ENTITY_NOT_FOUND",
            format!("Node '{node_id}' not found"),
        )
    })?;
    if let Some(exp_v) = expected_version {
        if existing.version != exp_v {
            return Err(FlowServiceError::new(
                409,
                "STALE_NODE_VERSION",
                "Node version mismatch",
            ));
        }
    }
    let cascade_ids = collect_adjacent_edge_ids(tx, tenant_id, project_id, node_id)?;
    tx.execute(
        "DELETE FROM flow_nodes WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3",
        params![node_id, tenant_id, project_id],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "delete_flow_node")))?;

    let next_gv = if bump_graph {
        tx.execute(
            "UPDATE flow_graphs SET version = version + 1, updated_at = ?1 WHERE id = ?2",
            params![Utc::now().timestamp_millis(), hdr.id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "bump_del_gv")))?;
        hdr.version + 1
    } else {
        hdr.version
    };
    Ok(DeleteFlowNodeResult {
        success: true,
        id: node_id.to_string(),
        graph_version: next_gv,
        cascade_deleted_edge_ids: cascade_ids,
    })
}

fn collect_adjacent_edge_ids(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    node_id: &str,
) -> Result<Vec<String>, FlowServiceError> {
    let mut stmt = tx
        .prepare("SELECT id FROM flow_edges WHERE tenant_id = ?1 AND project_id = ?2 AND (source_node_id = ?3 OR target_node_id = ?3) ORDER BY id ASC")
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "list_cascade_edges")))?;
    let rows = stmt
        .query_map(params![tenant_id, project_id, node_id], |r| {
            r.get::<_, String>(0)
        })
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "map_cascade_edges")))?;
    let mut ids = Vec::new();
    for r in rows {
        ids.push(
            r.map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "read_cascade_id")))?,
        );
    }
    Ok(ids)
}
