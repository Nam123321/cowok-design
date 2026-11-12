// @story 04.1
// @implements AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
//! Authoritative 2D Flow DAG Persistence & Topological Validation Engine (`crates/flow-graph`)
//! backed by `crates/spec-registry` SQLite WAL (`PD-01 = A`).

pub mod cycle_detection;
pub mod deploy_draft;
mod edge_mutations;
pub mod layout_sugiyama;
pub mod models;
mod mutations;
mod node_batch;
mod queries;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use rusqlite::{params, Transaction, TransactionBehavior};
use spec_registry::{is_sqlite_busy_or_locked, map_sqlite_error, SqlitePool};

pub use cycle_detection::{ensure_not_self_loop, normalize_coord, validate_acyclic_dag};
pub use deploy_draft::compute_canonical_draft_sha256;
pub use layout_sugiyama::compute_sugiyama_positions;
pub use models::{
    BatchPositionItem, BatchUpdatePositionsInput, BatchUpdatePositionsResult, CreateFlowNodeInput,
    CreateFlowTransitionInput, DeleteFlowNodeResult, DeleteFlowTransitionResult,
    DeployFlowDraftInput, DraftNodeDeleteItem, DraftNodeUpdateItem, DraftTransitionDeleteItem,
    DraftTransitionUpdateItem, FlowGraphSnapshotDto, FlowNodeDto, FlowServiceError,
    FlowTransitionDto, UpdateFlowNodeInput, UpdateFlowTransitionInput,
};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Executes a read-only `DEFERRED` transaction on a single SQLite WAL connection.
fn with_deferred_tx<T, F>(pool: &SqlitePool, op: F) -> Result<T, FlowServiceError>
where
    F: FnOnce(&Transaction<'_>) -> Result<T, FlowServiceError>,
{
    let mut conn = pool.connection().map_err(FlowServiceError::from_harness)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "begin_deferred_flow")))?;
    let out = op(&tx)?;
    tx.commit()
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "commit_deferred_flow")))?;
    Ok(out)
}

/// Executes a serialized `BEGIN IMMEDIATE` write transaction on a single SQLite WAL connection
/// with `25ms/50ms/100ms` jittered retry bounded strictly within `<= 5,000ms` total wall-clock time.
fn with_immediate_tx<T, F>(pool: &SqlitePool, mut op: F) -> Result<T, FlowServiceError>
where
    F: FnMut(&Transaction<'_>) -> Result<T, FlowServiceError>,
{
    const BACKOFF_MS: [u64; 3] = [25, 50, 100];
    const MAX_WALL_MS: u128 = 5_000;
    let started = Instant::now();
    let mut attempt = 0usize;

    let mut conn = pool.connection().map_err(FlowServiceError::from_harness)?;
    loop {
        match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
            Ok(tx) => {
                let res = op(&tx)?;
                tx.commit().map_err(|e| {
                    FlowServiceError::from_harness(map_sqlite_error(e, "commit_immediate_flow"))
                })?;
                return Ok(res);
            }
            Err(err) => {
                let elapsed = started.elapsed().as_millis();
                if is_sqlite_busy_or_locked(&err)
                    && attempt < BACKOFF_MS.len()
                    && elapsed < MAX_WALL_MS
                {
                    let jitter = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| u64::from(d.subsec_nanos() % 16))
                        .unwrap_or(0);
                    let sleep_ms =
                        (BACKOFF_MS[attempt] + jitter).min((MAX_WALL_MS - elapsed) as u64);
                    attempt += 1;
                    if sleep_ms > 0 {
                        std::thread::sleep(Duration::from_millis(sleep_ms));
                    }
                    continue;
                }
                return Err(FlowServiceError::from_harness(map_sqlite_error(
                    err,
                    "begin_immediate_flow",
                )));
            }
        }
    }
}

pub fn get_snapshot(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
) -> Result<FlowGraphSnapshotDto, FlowServiceError> {
    with_deferred_tx(pool, |tx| {
        queries::load_snapshot_in_tx(tx, tenant_id, project_id, false)
    })
}

pub fn list_nodes(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
) -> Result<Vec<FlowNodeDto>, FlowServiceError> {
    with_deferred_tx(pool, |tx| {
        queries::ensure_project_and_graph(tx, tenant_id, project_id, false)?;
        queries::load_nodes_in_tx(tx, tenant_id, project_id)
    })
}

pub fn create_node(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    input: &CreateFlowNodeInput,
) -> Result<FlowNodeDto, FlowServiceError> {
    with_immediate_tx(pool, |tx| {
        let hdr = queries::ensure_project_and_graph(tx, tenant_id, project_id, true)?;
        let now_ms = Utc::now().timestamp_millis();
        let nid = mutations::insert_node_in_tx(tx, tenant_id, project_id, &hdr.id, input, now_ms)?;
        tx.execute(
            "UPDATE flow_graphs SET version = version + 1, updated_at = ?1 WHERE id = ?2",
            params![now_ms, hdr.id],
        )
        .map_err(|e| FlowServiceError::from_harness(map_sqlite_error(e, "bump_create_node_gv")))?;
        queries::load_node_by_id_in_tx(tx, tenant_id, project_id, &nid)?
            .ok_or_else(|| FlowServiceError::new(404, "ENTITY_NOT_FOUND", "Created node missing"))
    })
}

pub fn update_node(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    node_id: &str,
    input: &UpdateFlowNodeInput,
) -> Result<FlowNodeDto, FlowServiceError> {
    with_immediate_tx(pool, |tx| {
        mutations::apply_node_patch_in_tx(tx, tenant_id, project_id, node_id, input, true)
    })
}

pub fn batch_update_positions(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    input: &BatchUpdatePositionsInput,
) -> Result<BatchUpdatePositionsResult, FlowServiceError> {
    with_immediate_tx(pool, |tx| {
        mutations::batch_positions_in_tx(tx, tenant_id, project_id, &input.positions, true)
    })
}

pub fn delete_node(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    node_id: &str,
    expected_version: Option<i64>,
    expected_graph_version: Option<i64>,
) -> Result<DeleteFlowNodeResult, FlowServiceError> {
    with_immediate_tx(pool, |tx| {
        mutations::delete_node_in_tx(
            tx,
            tenant_id,
            project_id,
            node_id,
            expected_version,
            expected_graph_version,
            true,
        )
    })
}

pub fn list_transitions(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
) -> Result<Vec<FlowTransitionDto>, FlowServiceError> {
    with_deferred_tx(pool, |tx| {
        queries::ensure_project_and_graph(tx, tenant_id, project_id, false)?;
        queries::load_transitions_in_tx(tx, tenant_id, project_id)
    })
}

pub fn create_transition(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    input: &CreateFlowTransitionInput,
) -> Result<FlowTransitionDto, FlowServiceError> {
    with_immediate_tx(pool, |tx| {
        edge_mutations::insert_transition_in_tx(tx, tenant_id, project_id, input, true, true)
    })
}

pub fn update_transition(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    transition_id: &str,
    input: &UpdateFlowTransitionInput,
) -> Result<FlowTransitionDto, FlowServiceError> {
    with_immediate_tx(pool, |tx| {
        edge_mutations::update_transition_in_tx(
            tx,
            tenant_id,
            project_id,
            transition_id,
            input,
            true,
            true,
        )
    })
}

pub fn delete_transition(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    transition_id: &str,
    expected_version: Option<i64>,
) -> Result<DeleteFlowTransitionResult, FlowServiceError> {
    with_immediate_tx(pool, |tx| {
        edge_mutations::delete_transition_in_tx(
            tx,
            tenant_id,
            project_id,
            transition_id,
            expected_version,
            true,
        )
    })
}

pub fn deploy_draft(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    input: &DeployFlowDraftInput,
) -> Result<FlowGraphSnapshotDto, FlowServiceError> {
    with_immediate_tx(pool, |tx| {
        deploy_draft::deploy_draft_in_tx(tx, tenant_id, project_id, input)
    })
}
