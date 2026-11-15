// @story 04.4
// @implements AC-02 (Managed Worker 5s Heartbeat, Startup Orphan Reaper & Lease Expiry Sweeper)
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use chrono::Utc;
use domain::HarnessError;
use rusqlite::{params, types::Value, TransactionBehavior};
use spec_registry::{retry_on_busy, SqlitePool};
use tokio::sync::{broadcast, watch};
use uuid::Uuid;

use super::execution_coordinator_sql::{promote_next_queued_tx, LEASE_TTL_MS};
use super::execution_reaper::{reap_job_worker_and_shadow, ActiveWorkerRegistry};

struct ReapOutcome {
    interrupted_jobs: Vec<(String, String)>,
    notify_projects: Vec<String>,
}

async fn refresh_lease_step(pool: &SqlitePool, job_id: &str, fencing_token: i64) -> bool {
    let pool = pool.clone();
    let jid = job_id.to_string();
    tokio::task::spawn_blocking(move || {
        let Ok(conn) = pool.connection() else { return false; };
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE node_execution_jobs SET stale_after_timestamp = ?1, updated_at = ?2 WHERE id = ?3 AND fencing_token = ?4 AND status = 'RUNNING'",
            params![now_ms + LEASE_TTL_MS, now_ms, jid, fencing_token],
        )
        .map(|rows| rows > 0)
        .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

pub fn spawn_managed_worker_heartbeat(
    pool: SqlitePool,
    registry: ActiveWorkerRegistry,
    job_id: String,
    fencing_token: i64,
) -> tokio::task::JoinHandle<()> {
    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    registry.register_heartbeat(&job_id, fencing_token, cancel_tx);
    tokio::spawn(async move {
        let period = Duration::from_secs(5);
        let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        loop {
            tokio::select! {
                changed = cancel_rx.changed() => {
                    if changed.is_err() || *cancel_rx.borrow() { break; }
                }
                _ = ticker.tick() => {
                    if *cancel_rx.borrow() { break; }
                    if !refresh_lease_step(&pool, &job_id, fencing_token).await { break; }
                }
            }
        }
        registry.cancel_heartbeat(&job_id, fencing_token);
    })
}

fn mark_jobs_interrupted_tx(
    conn: &mut rusqlite::Connection,
    sql_select: &str,
    select_param: Value,
    reason_code: &str,
    live_nonce: &str,
    now_ms: i64,
) -> Result<ReapOutcome, rusqlite::Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut targets = Vec::new();
    {
        let mut stmt = tx.prepare(sql_select)?;
        let rows = stmt.query_map([select_param], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?;
        for r in rows {
            targets.push(r?);
        }
    }
    let mut interrupted_jobs = Vec::new();
    let mut notify_projects = Vec::new();
    for (jid, tid, pid, nid, scope, fence) in targets {
        let next_fence = fence + 1;
        let payload =
            serde_json::json!({ "reason": reason_code, "fencingToken": next_fence }).to_string();
        let updated = tx.execute(
            "UPDATE node_execution_jobs SET status = 'INTERRUPTED', fencing_token = ?1, stale_after_timestamp = NULL, error_json = COALESCE(error_json, ?2), updated_at = ?3 WHERE id = ?4 AND fencing_token = ?5 AND composite_scope_key = ?6",
            params![next_fence, payload, now_ms, jid, fence, scope],
        )?;
        if updated == 0 {
            continue;
        }
        let next_seq: i64 = tx.query_row(
            "SELECT COALESCE(MAX(seq_no), 0) + 1 FROM execution_events WHERE job_id = ?1",
            params![jid],
            |r| r.get(0),
        )?;
        let next_pseq: i64 = tx.query_row(
            "SELECT COALESCE(MAX(project_seq), 0) + 1 FROM execution_events WHERE project_id = ?1",
            params![pid],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO execution_events (id, tenant_id, project_id, job_id, node_id, composite_scope_key, event_kind, payload_text, seq_no, project_seq, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'interrupted', ?7, ?8, ?9, ?10)",
            params![Uuid::new_v4().to_string(), tid, pid, jid, nid, scope, payload, next_seq, next_pseq, now_ms],
        )?;
        interrupted_jobs.push((pid.clone(), jid));
        notify_projects.push(pid);
    }
    if !live_nonce.is_empty() {
        for (_, pid) in promote_next_queued_tx(&tx, live_nonce, now_ms)? {
            notify_projects.push(pid);
        }
    }
    tx.commit()?;
    Ok(ReapOutcome {
        interrupted_jobs,
        notify_projects,
    })
}

pub fn cleanup_orphan_shadow_dirs(pool: &SqlitePool, workspace_root: &Path) {
    let shadow_root = workspace_root.join(".cowok-design").join("shadow");
    let Ok(proj_entries) = std::fs::read_dir(&shadow_root) else {
        return;
    };
    let Ok(conn) = pool.connection() else {
        return;
    };
    let mut active = HashSet::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT project_id, id FROM node_execution_jobs WHERE status IN ('RUNNING', 'PAUSED')",
    ) {
        if let Ok(rows) =
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        {
            for row in rows.flatten() {
                active.insert(row);
            }
        }
    }
    for proj_dir in proj_entries.flatten() {
        let p_path = proj_dir.path();
        if !p_path.is_dir() {
            continue;
        }
        let pid = proj_dir.file_name().to_string_lossy().into_owned();
        if let Ok(job_entries) = std::fs::read_dir(&p_path) {
            for job_dir in job_entries.flatten() {
                let j_path = job_dir.path();
                let jid = job_dir.file_name().to_string_lossy().into_owned();
                if j_path.is_dir() && !active.contains(&(pid.clone(), jid)) {
                    let _ = std::fs::remove_dir_all(&j_path);
                }
            }
        }
        if std::fs::read_dir(&p_path)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false)
        {
            let _ = std::fs::remove_dir(&p_path);
        }
    }
}

async fn run_reap_query(
    pool: &SqlitePool,
    label: &'static str,
    sql: &'static str,
    param: Value,
    reason_code: &'static str,
    live_nonce: String,
) -> Result<ReapOutcome, HarnessError> {
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool.connection()?;
        let now_ms = Utc::now().timestamp_millis();
        retry_on_busy(label, || {
            mark_jobs_interrupted_tx(
                &mut conn,
                sql,
                param.clone(),
                reason_code,
                &live_nonce,
                now_ms,
            )
        })
    })
    .await
    .map_err(|e| HarnessError::ValidationFailed {
        reason: format!("{label}_JOIN_FAILED: {e}"),
    })?
}

pub async fn reap_orphan_jobs_with_workers(
    pool: &SqlitePool,
    workers: &ActiveWorkerRegistry,
    workspace_root: Option<&Path>,
    live_nonce: &str,
) -> Result<usize, HarnessError> {
    let sql = "SELECT id, tenant_id, project_id, node_id, composite_scope_key, fencing_token FROM node_execution_jobs WHERE status IN ('RUNNING', 'PAUSED') AND daemon_nonce != ?1";
    let outcome = run_reap_query(
        pool,
        "reap_orphan_jobs_on_startup",
        sql,
        Value::Text(live_nonce.to_string()),
        "DAEMON_RESTART_ORPHAN",
        live_nonce.to_string(),
    )
    .await?;
    for (pid, jid) in &outcome.interrupted_jobs {
        if let Some(root) = workspace_root {
            let _ = reap_job_worker_and_shadow(workers, root, pid, jid);
        } else {
            for entry in workers.take_all_for_job(jid) {
                let _ = super::execution_reaper::terminate_pgid_checked(
                    entry.pid,
                    entry.pgid,
                    &entry.start_time,
                );
            }
        }
    }
    if let Some(root) = workspace_root {
        cleanup_orphan_shadow_dirs(pool, root);
    }
    Ok(outcome.interrupted_jobs.len())
}

pub async fn reap_orphan_jobs_on_startup(
    pool: &SqlitePool,
    live_nonce: &str,
) -> Result<usize, HarnessError> {
    reap_orphan_jobs_with_workers(pool, &ActiveWorkerRegistry::new(), None, live_nonce).await
}

pub async fn sweep_expired_leases_with_workers(
    pool: &SqlitePool,
    workers: &ActiveWorkerRegistry,
    workspace_root: Option<&Path>,
    live_nonce: &str,
    sse_tx: &broadcast::Sender<String>,
) -> Result<usize, HarnessError> {
    let sql = "SELECT id, tenant_id, project_id, node_id, composite_scope_key, fencing_token FROM node_execution_jobs WHERE status = 'RUNNING' AND stale_after_timestamp IS NOT NULL AND stale_after_timestamp < ?1";
    let outcome = run_reap_query(
        pool,
        "sweep_expired_leases",
        sql,
        Value::Integer(Utc::now().timestamp_millis()),
        "LEASE_EXPIRED",
        live_nonce.to_string(),
    )
    .await?;
    for (pid, jid) in &outcome.interrupted_jobs {
        if let Some(root) = workspace_root {
            let _ = reap_job_worker_and_shadow(workers, root, pid, jid);
        } else {
            for entry in workers.take_all_for_job(jid) {
                let _ = super::execution_reaper::terminate_pgid_checked(
                    entry.pid,
                    entry.pgid,
                    &entry.start_time,
                );
            }
        }
    }
    for pid in &outcome.notify_projects {
        let _ = sse_tx.send(pid.clone());
    }
    Ok(outcome.interrupted_jobs.len())
}

pub async fn sweep_expired_leases(
    pool: &SqlitePool,
    sse_tx: &broadcast::Sender<String>,
) -> Result<usize, HarnessError> {
    sweep_expired_leases_with_workers(pool, &ActiveWorkerRegistry::new(), None, "", sse_tx).await
}
