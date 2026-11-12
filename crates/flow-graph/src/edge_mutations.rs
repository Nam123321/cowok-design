// @story 04.1
// @implements AC-04, AC-06, AC-07, AC-08
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};
use spec_registry::map_sqlite_error;
use uuid::Uuid;

use crate::cycle_detection::{ensure_not_self_loop, validate_acyclic_dag};
use crate::models::{
    CreateFlowTransitionInput, DeleteFlowTransitionResult, FlowServiceError, FlowTransitionDto,
    UpdateFlowTransitionInput,
};
use crate::queries::{
    ensure_project_and_graph, load_node_by_id_in_tx, load_nodes_in_tx, load_transitions_in_tx,
    map_transition_row, validate_flow_type,
};

pub(crate) fn load_transition_by_id_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    transition_id: &str,
) -> Result<Option<FlowTransitionDto>, FlowServiceError> {
    tx.query_row(
        "SELECT id, tenant_id, project_id, source_node_id, target_node_id, flow_type, \
                trigger_event, guard_condition, version, created_at, updated_at \
         FROM flow_edges WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3",
        params![transition_id, tenant_id, project_id],
        map_transition_row,
    )
    .optional()
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "load_transition_by_id")))
}

pub(crate) fn insert_transition_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    input: &CreateFlowTransitionInput,
    verify_dag: bool,
    bump_graph: bool,
) -> Result<FlowTransitionDto, FlowServiceError> {
    ensure_not_self_loop(&input.source_node_id, &input.target_node_id)?;
    let flow_type = validate_flow_type(input.flow_type.trim())?;
    let trigger_event = input.trigger_event.trim();
    if trigger_event.is_empty() {
        return Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            "triggerEvent is required",
        ));
    }

    let hdr = ensure_project_and_graph(tx, tenant_id, project_id, true)?;
    if load_node_by_id_in_tx(tx, tenant_id, project_id, input.source_node_id.trim())?.is_none()
        || load_node_by_id_in_tx(tx, tenant_id, project_id, input.target_node_id.trim())?.is_none()
    {
        return Err(FlowServiceError::new(
            422,
            "INVALID_NODE_REFERENCE",
            "sourceNodeId and targetNodeId must exist in the same tenant and project",
        ));
    }

    let edge_id = match &input.id {
        Some(raw) if !raw.trim().is_empty() => Uuid::parse_str(raw.trim())
            .map_err(|_| {
                FlowServiceError::new(
                    400,
                    "VALIDATION_ERROR",
                    "transition id must be a valid UUID",
                )
            })?
            .to_string(),
        _ => Uuid::new_v4().to_string(),
    };
    let now_ms = Utc::now().timestamp_millis();

    tx.execute(
        "INSERT INTO flow_edges ( \
            id, graph_id, tenant_id, project_id, source_node_id, target_node_id, \
            flow_type, trigger_event, guard_condition, version, created_at, updated_at \
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?10)",
        params![
            edge_id,
            hdr.id,
            tenant_id,
            project_id,
            input.source_node_id.trim(),
            input.target_node_id.trim(),
            flow_type,
            trigger_event,
            input.guard_condition,
            now_ms
        ],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "insert_flow_edge")))?;

    if verify_dag {
        assert_current_graph_acyclic(tx, tenant_id, project_id)?;
    }
    if bump_graph {
        tx.execute(
            "UPDATE flow_graphs SET version = version + 1, updated_at = ?1 WHERE id = ?2",
            params![now_ms, hdr.id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "bump_edge_create_gv")))?;
    }

    load_transition_by_id_in_tx(tx, tenant_id, project_id, &edge_id)?.ok_or_else(|| {
        FlowServiceError::new(404, "ENTITY_NOT_FOUND", "Inserted transition missing")
    })
}

pub(crate) fn update_transition_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    transition_id: &str,
    input: &UpdateFlowTransitionInput,
    verify_dag: bool,
    bump_graph: bool,
) -> Result<FlowTransitionDto, FlowServiceError> {
    let hdr = ensure_project_and_graph(tx, tenant_id, project_id, true)?;
    let existing = load_transition_by_id_in_tx(tx, tenant_id, project_id, transition_id)?
        .ok_or_else(|| {
            FlowServiceError::new(
                404,
                "ENTITY_NOT_FOUND",
                format!("Transition '{transition_id}' not found"),
            )
        })?;

    let exp_v = input.expected_version.unwrap_or(existing.version);
    if existing.version != exp_v {
        return Err(FlowServiceError::new(
            409,
            "STALE_EDGE_VERSION",
            format!(
                "Transition '{transition_id}' version mismatch (expected {exp_v}, actual {})",
                existing.version
            ),
        ));
    }

    let (src_id, dst_id) = if input.reversed.unwrap_or(false) {
        (
            existing.target_node_id.clone(),
            existing.source_node_id.clone(),
        )
    } else {
        (
            existing.source_node_id.clone(),
            existing.target_node_id.clone(),
        )
    };
    ensure_not_self_loop(&src_id, &dst_id)?;

    let flow_type = validate_flow_type(
        input
            .flow_type
            .as_deref()
            .unwrap_or(&existing.flow_type)
            .trim(),
    )?;
    let trigger_event = input
        .trigger_event
        .as_deref()
        .unwrap_or(&existing.trigger_event)
        .trim();
    if trigger_event.is_empty() {
        return Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            "triggerEvent cannot be empty",
        ));
    }
    let guard_cond = match &input.guard_condition {
        Some(inner) => inner.clone(),
        None => existing.guard_condition.clone(),
    };
    let now_ms = Utc::now().timestamp_millis();

    tx.execute(
        "UPDATE flow_edges SET source_node_id = ?1, target_node_id = ?2, flow_type = ?3, \
         trigger_event = ?4, guard_condition = ?5, version = version + 1, updated_at = ?6 \
         WHERE id = ?7 AND tenant_id = ?8 AND project_id = ?9 AND version = ?10",
        params![
            src_id,
            dst_id,
            flow_type,
            trigger_event,
            guard_cond,
            now_ms,
            transition_id,
            tenant_id,
            project_id,
            exp_v
        ],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "update_flow_edge")))?;

    if verify_dag {
        assert_current_graph_acyclic(tx, tenant_id, project_id)?;
    }
    if bump_graph {
        tx.execute(
            "UPDATE flow_graphs SET version = version + 1, updated_at = ?1 WHERE id = ?2",
            params![now_ms, hdr.id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "bump_edge_update_gv")))?;
    }

    load_transition_by_id_in_tx(tx, tenant_id, project_id, transition_id)?
        .ok_or_else(|| FlowServiceError::new(404, "ENTITY_NOT_FOUND", "Updated transition missing"))
}

pub(crate) fn delete_transition_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    transition_id: &str,
    expected_version: Option<i64>,
    bump_graph: bool,
) -> Result<DeleteFlowTransitionResult, FlowServiceError> {
    let hdr = ensure_project_and_graph(tx, tenant_id, project_id, true)?;
    let existing = load_transition_by_id_in_tx(tx, tenant_id, project_id, transition_id)?
        .ok_or_else(|| {
            FlowServiceError::new(
                404,
                "ENTITY_NOT_FOUND",
                format!("Transition '{transition_id}' not found"),
            )
        })?;

    if let Some(exp_v) = expected_version {
        if existing.version != exp_v {
            return Err(FlowServiceError::new(
                409,
                "STALE_EDGE_VERSION",
                format!(
                    "Transition '{transition_id}' version mismatch (expected {exp_v}, actual {})",
                    existing.version
                ),
            ));
        }
    }

    tx.execute(
        "DELETE FROM flow_edges WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3",
        params![transition_id, tenant_id, project_id],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "delete_flow_edge")))?;

    let next_gv = if bump_graph {
        let now_ms = Utc::now().timestamp_millis();
        tx.execute(
            "UPDATE flow_graphs SET version = version + 1, updated_at = ?1 WHERE id = ?2",
            params![now_ms, hdr.id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "bump_edge_delete_gv")))?;
        hdr.version + 1
    } else {
        hdr.version
    };

    Ok(DeleteFlowTransitionResult {
        success: true,
        id: transition_id.to_string(),
        graph_version: next_gv,
    })
}

pub(crate) fn assert_current_graph_acyclic(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
) -> Result<(), FlowServiceError> {
    let nodes = load_nodes_in_tx(tx, tenant_id, project_id)?;
    let edges = load_transitions_in_tx(tx, tenant_id, project_id)?;
    let node_ids: Vec<String> = nodes.into_iter().map(|n| n.id).collect();
    let edge_pairs: Vec<(String, String)> = edges
        .into_iter()
        .map(|e| (e.source_node_id, e.target_node_id))
        .collect();
    validate_acyclic_dag(&node_ids, &edge_pairs)
}
