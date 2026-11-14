// @story 04.1
// @implements AC-04, AC-05, AC-06, AC-07, AC-08
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};
use spec_registry::map_sqlite_error;
use uuid::Uuid;

use crate::cycle_detection::normalize_coord;
use crate::edge_mutations::{
    assert_current_graph_acyclic, delete_transition_in_tx, insert_transition_in_tx,
    update_transition_in_tx,
};
use crate::models::{
    normalize_fixture_ref, normalize_viewport_preset, DeployFlowDraftInput, FlowGraphSnapshotDto,
    FlowServiceError, MAX_BATCH_EDGES, MAX_BATCH_NODES,
};
use crate::mutations::{
    apply_node_patch_in_tx, batch_positions_in_tx, delete_node_in_tx, insert_node_in_tx,
};
use crate::queries::{ensure_project_and_graph, load_snapshot_in_tx};

const DRAFT_TTL_MS: i64 = 24 * 60 * 60 * 1_000;

/// Computes a deterministic SHA-256 digest over canonicalized `DeployFlowDraftInput`
/// (`-0.0 -> 0.0`, `1/100 px` rounding, trimmed strings, deterministic key order).
pub fn compute_canonical_draft_sha256(
    input: &DeployFlowDraftInput,
) -> Result<String, FlowServiceError> {
    let mut normalized = input.clone();
    normalized.draft_id = normalized.draft_id.trim().to_string();
    for n in &mut normalized.created_nodes {
        n.id = n.id.as_ref().map(|v| v.trim().to_string());
        n.portal_id = n.portal_id.trim().to_string();
        n.screen_key = n.screen_key.trim().to_string();
        n.title = n.title.trim().to_string();
        n.node_type = n.node_type.trim().to_string();
        n.route_path = n.route_path.trim().to_string();
        n.component_entry_path = n.component_entry_path.trim().to_string();
        n.fixture_ref = normalize_fixture_ref(&n.fixture_ref)?;
        n.viewport_preset = normalize_viewport_preset(&n.viewport_preset)?;
        n.pos_x = normalize_coord(n.pos_x, "posX")?;
        n.pos_y = normalize_coord(n.pos_y, "posY")?;
    }
    for u in &mut normalized.updated_nodes {
        u.node_id = u.node_id.trim().to_string();
        u.patch.portal_id = u.patch.portal_id.as_ref().map(|v| v.trim().to_string());
        u.patch.title = u.patch.title.as_ref().map(|v| v.trim().to_string());
        u.patch.node_type = u.patch.node_type.as_ref().map(|v| v.trim().to_string());
        u.patch.route_path = u.patch.route_path.as_ref().map(|v| v.trim().to_string());
        u.patch.component_entry_path = u
            .patch
            .component_entry_path
            .as_ref()
            .map(|v| v.trim().to_string());
        if let Some(ref vp) = u.patch.viewport_preset {
            u.patch.viewport_preset = Some(normalize_viewport_preset(vp)?);
        }
        if let Some(ref fr) = u.patch.fixture_ref {
            u.patch.fixture_ref = Some(normalize_fixture_ref(fr)?);
        }
        if let Some(px) = u.patch.pos_x {
            u.patch.pos_x = Some(normalize_coord(px, "posX")?);
        }
        if let Some(py) = u.patch.pos_y {
            u.patch.pos_y = Some(normalize_coord(py, "posY")?);
        }
    }
    for d in &mut normalized.deleted_nodes {
        d.node_id = d.node_id.trim().to_string();
    }
    for e in &mut normalized.updated_transitions {
        e.transition_id = e.transition_id.trim().to_string();
        e.patch.flow_type = e.patch.flow_type.as_ref().map(|v| v.trim().to_string());
        e.patch.trigger_event = e.patch.trigger_event.as_ref().map(|v| v.trim().to_string());
    }
    for d in &mut normalized.deleted_transitions {
        d.transition_id = d.transition_id.trim().to_string();
    }
    for p in &mut normalized.updated_node_positions {
        p.node_id = p.node_id.trim().to_string();
        p.pos_x = normalize_coord(p.pos_x, "posX")?;
        p.pos_y = normalize_coord(p.pos_y, "posY")?;
    }
    for e in &mut normalized.created_transitions {
        e.id = e.id.as_ref().map(|v| v.trim().to_string());
        e.source_node_id = e.source_node_id.trim().to_string();
        e.target_node_id = e.target_node_id.trim().to_string();
        e.flow_type = e.flow_type.trim().to_string();
        e.trigger_event = e.trigger_event.trim().to_string();
    }
    let bytes = serde_json::to_vec(&normalized).map_err(|e| {
        FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            format!("Failed to serialize draft payload: {e}"),
        )
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn deploy_draft_in_tx(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    input: &DeployFlowDraftInput,
) -> Result<FlowGraphSnapshotDto, FlowServiceError> {
    let draft_uuid = Uuid::parse_str(input.draft_id.trim())
        .map_err(|_| {
            FlowServiceError::new(400, "VALIDATION_ERROR", "draftId must be a valid UUIDv4")
        })?
        .to_string();

    if input.created_nodes.len() + input.updated_nodes.len() + input.deleted_nodes.len()
        > MAX_BATCH_NODES
        || input.created_transitions.len()
            + input.updated_transitions.len()
            + input.deleted_transitions.len()
            > MAX_BATCH_EDGES
    {
        return Err(FlowServiceError::new(
            400,
            "VALIDATION_ERROR",
            "Draft batch exceeds node/edge limits",
        ));
    }

    let hdr = ensure_project_and_graph(tx, tenant_id, project_id, true)?;
    let now_ms = Utc::now().timestamp_millis();

    // Prune up to 100 expired idempotency rows (`expires_at < now_ms`)
    tx.execute(
        "DELETE FROM flow_draft_deployments WHERE rowid IN ( \
            SELECT rowid FROM flow_draft_deployments WHERE expires_at < ?1 LIMIT 100 \
         )",
        params![now_ms],
    )
    .map_err(|e| {
        FlowServiceError::from_harness(map_sqlite_error(e, "prune_expired_deployments"))
    })?;

    let req_sha = compute_canonical_draft_sha256(input)?;
    let existing_dep: Option<(String, String)> = tx
        .query_row(
            "SELECT request_sha256, response_json FROM flow_draft_deployments \
             WHERE tenant_id = ?1 AND project_id = ?2 AND draft_id = ?3 AND expires_at >= ?4",
            params![tenant_id, project_id, draft_uuid, now_ms],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| {
            FlowServiceError::from_harness(map_sqlite_error(e, "lookup_draft_idempotency"))
        })?;

    if let Some((saved_sha, saved_json)) = existing_dep {
        if saved_sha != req_sha {
            return Err(FlowServiceError::new(
                422,
                "DRAFT_ID_PAYLOAD_MISMATCH",
                format!("draftId '{draft_uuid}' was already committed with a different payload"),
            ));
        }
        return serde_json::from_str::<FlowGraphSnapshotDto>(&saved_json).map_err(|e| {
            FlowServiceError::new(
                500,
                "INTERNAL_ERROR",
                format!("Corrupt cached deployment snapshot: {e}"),
            )
        });
    }

    if hdr.version != input.expected_graph_version {
        return Err(FlowServiceError::new(
            409,
            "STALE_GRAPH_VERSION",
            format!(
                "expectedGraphVersion {} != actual {}",
                input.expected_graph_version, hdr.version
            ),
        ));
    }

    apply_draft_operations(tx, tenant_id, project_id, &hdr.id, input, now_ms)?;
    assert_current_graph_acyclic(tx, tenant_id, project_id)?;

    let next_gv = hdr.version + 1;
    let next_lv = if input.updated_node_positions.is_empty() {
        hdr.layout_version
    } else {
        hdr.layout_version + 1
    };
    tx.execute(
        "UPDATE flow_graphs SET version = ?1, layout_version = ?2, updated_at = ?3 WHERE id = ?4",
        params![next_gv, next_lv, now_ms, hdr.id],
    )
    .map_err(|e| {
        FlowServiceError::from_harness(map_sqlite_error(e, "commit_draft_graph_version"))
    })?;

    let snapshot = load_snapshot_in_tx(tx, tenant_id, project_id, false)?;
    let resp_json = serde_json::to_string(&snapshot).map_err(|e| {
        FlowServiceError::new(
            500,
            "INTERNAL_ERROR",
            format!("Snapshot serialize error: {e}"),
        )
    })?;
    tx.execute(
        "INSERT INTO flow_draft_deployments ( \
            tenant_id, project_id, draft_id, request_sha256, resulting_graph_version, \
            response_json, created_at, expires_at \
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            tenant_id,
            project_id,
            draft_uuid,
            req_sha,
            next_gv,
            resp_json,
            now_ms,
            now_ms + DRAFT_TTL_MS
        ],
    )
    .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "record_draft_deployment")))?;

    Ok(snapshot)
}

fn apply_draft_operations(
    tx: &Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    graph_id: &str,
    input: &DeployFlowDraftInput,
    now_ms: i64,
) -> Result<(), FlowServiceError> {
    for dt in &input.deleted_transitions {
        delete_transition_in_tx(
            tx,
            tenant_id,
            project_id,
            &dt.transition_id,
            Some(dt.expected_version),
            false,
        )?;
    }
    for dn in &input.deleted_nodes {
        delete_node_in_tx(
            tx,
            tenant_id,
            project_id,
            &dn.node_id,
            Some(dn.expected_version),
            None,
            false,
        )?;
    }
    for cn in &input.created_nodes {
        insert_node_in_tx(tx, tenant_id, project_id, graph_id, cn, now_ms)?;
    }
    for un in &input.updated_nodes {
        if un.patch.expected_version.is_none() {
            return Err(FlowServiceError::new(
                400,
                "VALIDATION_ERROR",
                "updatedNodes requires expectedVersion",
            ));
        }
        apply_node_patch_in_tx(tx, tenant_id, project_id, &un.node_id, &un.patch, false)?;
    }
    if !input.updated_node_positions.is_empty() {
        batch_positions_in_tx(
            tx,
            tenant_id,
            project_id,
            &input.updated_node_positions,
            false,
        )?;
    }
    for ct in &input.created_transitions {
        insert_transition_in_tx(tx, tenant_id, project_id, ct, false, false)?;
    }
    for ut in &input.updated_transitions {
        if ut.patch.expected_version.is_none() {
            return Err(FlowServiceError::new(
                400,
                "VALIDATION_ERROR",
                "updatedTransitions requires expectedVersion",
            ));
        }
        update_transition_in_tx(
            tx,
            tenant_id,
            project_id,
            &ut.transition_id,
            &ut.patch,
            false,
            false,
        )?;
    }
    Ok(())
}
