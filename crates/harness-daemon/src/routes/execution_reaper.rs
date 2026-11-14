// @story 04.4
// @implements AC-02 (ActiveWorkerRegistry, -PGID Reaper, Shadow Buffer & Startup/Lease Sweepers)
use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use domain::HarnessError;
use rusqlite::{params, types::Value, TransactionBehavior};
use spec_registry::{retry_on_busy, SqlitePool};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ActiveWorkerEntry {
    pub job_id: String,
    pub project_id: String,
    pub fencing_token: i64,
    pub pid: u32,
    pub pgid: i32,
    pub start_time: String,
    pub shadow_dir: PathBuf,
}

#[derive(Clone, Default)]
pub struct ActiveWorkerRegistry {
    inner: Arc<Mutex<HashMap<(String, i64), ActiveWorkerEntry>>>,
}

impl ActiveWorkerRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&self, entry: ActiveWorkerEntry) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.insert((entry.job_id.clone(), entry.fencing_token), entry);
        }
    }
    pub fn take_all_for_job(&self, job_id: &str) -> Vec<ActiveWorkerEntry> {
        let Ok(mut guard) = self.inner.lock() else { return Vec::new(); };
        let keys: Vec<_> = guard.keys().filter(|(jid, _)| jid == job_id).cloned().collect();
        keys.into_iter().filter_map(|k| guard.remove(&k)).collect()
    }
}

fn validate_safe_segment(raw: &str, label: &str) -> Result<String, HarnessError> {
    let s = raw.trim();
    if s.is_empty() || s.contains("..") || s.contains('/') || s.contains('\\') || s.contains('\0') {
        return Err(HarnessError::BoundaryViolation {
            reason: format!("SHADOW_JAIL_INVALID_SEGMENT: {label}"),
        });
    }
    Ok(s.to_string())
}

pub fn shadow_dir_for(workspace_root: &Path, project_id: &str, job_id: &str) -> Result<PathBuf, HarnessError> {
    let pid = validate_safe_segment(project_id, "project_id")?;
    let jid = validate_safe_segment(job_id, "job_id")?;
    Ok(workspace_root.join(".cowok-design").join("shadow").join(pid).join(jid))
}

pub fn discard_shadow_dir(dir: &Path) -> std::io::Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn read_process_start_time(pid: u32) -> Option<String> {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rparen = raw.rfind(')')?;
    let rest: Vec<&str> = raw[rparen + 1..].split_whitespace().collect();
    if *rest.first()? == "Z" { return None; }
    rest.get(19).map(|s| (*s).to_string())
}

#[cfg(not(target_os = "linux"))]
pub fn read_process_start_time(pid: u32) -> Option<String> {
    let out = Command::new("ps").args(["-p", &pid.to_string(), "-o", "state=,lstart="]).output().ok()?;
    if !out.status.success() { return None; }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() || text.starts_with('Z') { return None; }
    let mut parts = text.splitn(2, char::is_whitespace);
    let _ = parts.next()?;
    let lstart = parts.next()?.trim();
    (!lstart.is_empty()).then(|| lstart.to_string())
}

pub fn spawn_pgid_child(mut cmd: Command) -> std::io::Result<(Child, u32, i32, String)> {
    // SAFETY: pre_exec only invokes async-signal-safe libc::setpgid(0, 0) prior to exec.
    unsafe {
        cmd.pre_exec(|| if libc::setpgid(0, 0) != 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) });
    }
    let child = cmd.spawn()?;
    let pid = child.id();
    let start_time = read_process_start_time(pid).unwrap_or_default();
    Ok((child, pid, pid as i32, start_time))
}

fn pgid_or_child_alive(pid: u32, pgid: i32) -> bool {
    let mut status: libc::c_int = 0;
    // SAFETY: waitpid(WNOHANG) reaps zombie children; kill(-pgid, 0) probes process group liveness.
    unsafe {
        let _ = libc::waitpid(pid as libc::pid_t, &mut status, libc::WNOHANG);
        libc::kill(-pgid, 0) == 0
    }
}

pub fn terminate_pgid_checked(pid: u32, pgid: i32, expected_start_time: &str) -> Result<bool, String> {
    if pid == 0 || pgid <= 0 {
        return Err("INVALID_PID_OR_PGID".to_string());
    }
    let Some(live_start) = read_process_start_time(pid) else { return Ok(false); };
    let expected = expected_start_time.trim();
    if !expected.is_empty() && live_start != expected {
        return Ok(false);
    }
    // SAFETY: verified live start time matches expected_start_time; signal only target -PGID.
    if unsafe { libc::kill(-pgid, libc::SIGTERM) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let deadline = Instant::now() + Duration::from_millis(250);
    while Instant::now() < deadline {
        if !pgid_or_child_alive(pid, pgid) {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // SAFETY: grace period elapsed; force-kill remaining -PGID processes and reap.
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
        let mut status: libc::c_int = 0;
        libc::waitpid(pid as libc::pid_t, &mut status, 0);
    }
    Ok(true)
}

pub fn reap_job_worker_and_shadow(
    registry: &ActiveWorkerRegistry,
    workspace_root: &Path,
    project_id: &str,
    job_id: &str,
) -> Result<bool, HarnessError> {
    let mut reaped = false;
    for entry in registry.take_all_for_job(job_id) {
        let _ = discard_shadow_dir(&entry.shadow_dir);
        let _ = terminate_pgid_checked(entry.pid, entry.pgid, &entry.start_time);
        reaped = true;
    }
    let canonical_shadow = shadow_dir_for(workspace_root, project_id, job_id)?;
    let _ = discard_shadow_dir(&canonical_shadow);
    Ok(reaped)
}

fn mark_jobs_interrupted_tx(
    conn: &mut rusqlite::Connection,
    sql_select: &str,
    select_param: Value,
    reason_json: &str,
    now_ms: i64,
) -> Result<Vec<String>, rusqlite::Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut targets = Vec::new();
    {
        let mut stmt = tx.prepare(sql_select)?;
        let rows = stmt.query_map([select_param], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?))
        })?;
        for r in rows { targets.push(r?); }
    }
    let mut projects = Vec::new();
    for (jid, tid, pid, nid, scope) in targets {
        tx.execute("UPDATE node_execution_jobs SET status = 'INTERRUPTED', error_json = COALESCE(error_json, ?1), updated_at = ?2 WHERE id = ?3", params![reason_json, now_ms, jid])?;
        let next_seq: i64 = tx.query_row("SELECT COALESCE(MAX(seq_no), 0) + 1 FROM execution_events WHERE job_id = ?1", params![jid], |r| r.get(0))?;
        let next_pseq: i64 = tx.query_row("SELECT COALESCE(MAX(project_seq), 0) + 1 FROM execution_events WHERE project_id = ?1", params![pid], |r| r.get(0))?;
        tx.execute(
            "INSERT INTO execution_events (id, tenant_id, project_id, job_id, node_id, composite_scope_key, event_kind, payload_text, seq_no, project_seq, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'interrupted', ?7, ?8, ?9, ?10)",
            params![Uuid::new_v4().to_string(), tid, pid, jid, nid, scope, reason_json, next_seq, next_pseq, now_ms],
        )?;
        projects.push(pid);
    }
    tx.commit()?;
    Ok(projects)
}

async fn run_reap_query(pool: &SqlitePool, label: &'static str, sql: &'static str, param: Value, reason: &'static str) -> Result<Vec<String>, HarnessError> {
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool.connection()?;
        let now_ms = Utc::now().timestamp_millis();
        retry_on_busy(label, || mark_jobs_interrupted_tx(&mut conn, sql, param.clone(), reason, now_ms))
    })
    .await
    .map_err(|e| HarnessError::ValidationFailed { reason: format!("{label}_JOIN_FAILED: {e}") })?
}

pub async fn reap_orphan_jobs_on_startup(pool: &SqlitePool, live_nonce: &str) -> Result<usize, HarnessError> {
    let sql = "SELECT id, tenant_id, project_id, node_id, composite_scope_key FROM node_execution_jobs WHERE status IN ('RUNNING', 'PAUSED') AND daemon_nonce != ?1";
    let projects = run_reap_query(pool, "reap_orphan_jobs_on_startup", sql, Value::Text(live_nonce.to_string()), "{\"reason\":\"DAEMON_RESTART_ORPHAN\"}").await?;
    Ok(projects.len())
}

pub async fn sweep_expired_leases(pool: &SqlitePool, sse_tx: &broadcast::Sender<String>) -> Result<usize, HarnessError> {
    let sql = "SELECT id, tenant_id, project_id, node_id, composite_scope_key FROM node_execution_jobs WHERE status = 'RUNNING' AND stale_after_timestamp IS NOT NULL AND stale_after_timestamp < ?1";
    let projects = run_reap_query(pool, "sweep_expired_leases", sql, Value::Integer(Utc::now().timestamp_millis()), "{\"reason\":\"LEASE_EXPIRED\"}").await?;
    let count = projects.len();
    for pid in projects {
        let _ = sse_tx.send(pid);
    }
    Ok(count)
}
