// @story 04.4
// @implements AC-01, AC-04 (Hardened Persistent SSE Execution Event Stream)
use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::{Extension, Path as AxumPath, Query, State},
    http::HeaderMap,
    response::{
        sse::{Event, KeepAlive, Sse},
        Response,
    },
};
use domain::HarnessError;
use futures_util::stream::Stream;
use rusqlite::{params, params_from_iter, types::Value, OptionalExtension};
use spec_registry::{map_sqlite_error, SqlitePool};
use tokio::sync::broadcast::error::RecvError;

use crate::daemon_auth::FlowPrincipal;
use crate::routes::execution_events::{ExecutionEventDto, StreamJobEventsQuery};
use crate::routes::legacy_workbench_v6::{
    check_project_uuid, epoch_ms_to_iso, map_harness_resp, run_db,
};
use crate::routes::DaemonAppState;

struct SsePollState {
    pool: SqlitePool,
    tenant_id: String,
    project_id: String,
    job_id: Option<String>,
    node_id: Option<String>,
    last_seq: i64,
    started: bool,
    closed: bool,
    rx: tokio::sync::broadcast::Receiver<String>,
}

fn fetch_pending_events_params(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    job_id: Option<&str>,
    node_id: Option<&str>,
    last_seq: i64,
) -> Result<(Vec<ExecutionEventDto>, bool), HarnessError> {
    let conn = pool.connection()?;
    let mut sql = String::from(
        "SELECT e.id, e.job_id, e.project_id, e.node_id, e.event_kind, e.payload_text, \
         e.seq_no, COALESCE(e.project_seq, e.seq_no) AS eff_proj_seq, e.composite_scope_key, \
         COALESCE(j.status, 'RUNNING'), e.created_at \
         FROM execution_events e LEFT JOIN node_execution_jobs j ON j.id = e.job_id \
         WHERE e.tenant_id = ?1 AND e.project_id = ?2 AND COALESCE(e.project_seq, e.seq_no) > ?3",
    );
    let mut bind: Vec<Value> = vec![
        Value::Text(tenant_id.into()),
        Value::Text(project_id.into()),
        Value::Integer(last_seq),
    ];
    if let Some(jid) = job_id {
        sql.push_str(&format!(" AND e.job_id = ?{}", bind.len() + 1));
        bind.push(Value::Text(jid.into()));
    }
    if let Some(nid) = node_id {
        sql.push_str(&format!(" AND e.node_id = ?{}", bind.len() + 1));
        bind.push(Value::Text(nid.into()));
    }
    sql.push_str(" ORDER BY eff_proj_seq ASC, e.created_at ASC LIMIT 50");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| map_sqlite_error(e, "prep_sse_events"))?;
    let rows = stmt
        .query_map(params_from_iter(bind), |r| {
            Ok(ExecutionEventDto {
                id: r.get(0)?,
                job_id: r.get(1)?,
                project_id: r.get(2)?,
                node_id: r.get(3)?,
                event_kind: r.get(4)?,
                payload_text: r.get(5)?,
                seq_no: r.get(6)?,
                project_seq: r.get(7)?,
                composite_scope_key: r.get(8)?,
                job_status: r.get(9)?,
                created_at: epoch_ms_to_iso(r.get(10)?),
            })
        })
        .map_err(|e| map_sqlite_error(e, "query_sse_events"))?;
    let mut events = Vec::new();
    for r in rows {
        events.push(r.map_err(|e| map_sqlite_error(e, "row_sse_events"))?);
    }
    let is_terminal = match job_id {
        Some(jid) => conn
            .query_row(
                "SELECT status FROM node_execution_jobs WHERE id = ?1 AND project_id = ?2",
                params![jid, project_id],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "sse_job_status"))?
            .is_some_and(|st| matches!(st.as_str(), "COMPLETED" | "FAILED" | "CANCELLED")),
        None => false,
    };
    Ok((events, is_terminal))
}

pub async fn handle_stream_job_events(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Query(q): Query<StreamJobEventsQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Response> {
    let pid = check_project_uuid(&id)?;
    let (pool, tid, pid_c) = (
        state.registry.pool().clone(),
        principal.tenant_id.clone(),
        pid.clone(),
    );
    run_db(move || {
        let conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        super::legacy_workbench_v6::verify_project_ownership(&conn, &tid, &pid_c)
    })
    .await?;
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<i64>().ok());
    let init = SsePollState {
        pool: state.registry.pool().clone(),
        tenant_id: principal.tenant_id,
        project_id: pid,
        job_id: q.job_id.filter(|s| !s.trim().is_empty()),
        node_id: q.node_id.filter(|s| !s.trim().is_empty()),
        last_seq: last_event_id.or(q.after_seq).unwrap_or(0),
        started: false,
        closed: false,
        rx: state.sse_tx.subscribe(),
    };
    let stream = futures_util::stream::unfold(init, |mut st| async move {
        if st.closed {
            return None;
        }
        loop {
            if st.started {
                tokio::select! {
                    recv = st.rx.recv() => match recv {
                        Ok(msg_pid) if msg_pid != st.project_id => continue,
                        Err(RecvError::Closed) => {}
                        _ => {}
                    },
                    _ = tokio::time::sleep(Duration::from_millis(1500)) => {}
                }
            }
            st.started = true;
            let (pool, tid, pid, jid, nid, last_seq) = (
                st.pool.clone(),
                st.tenant_id.clone(),
                st.project_id.clone(),
                st.job_id.clone(),
                st.node_id.clone(),
                st.last_seq,
            );
            let Ok(Ok((events, is_terminal))) = tokio::task::spawn_blocking(move || {
                fetch_pending_events_params(
                    &pool,
                    &tid,
                    &pid,
                    jid.as_deref(),
                    nid.as_deref(),
                    last_seq,
                )
            })
            .await
            else {
                return None;
            };
            if let Some(last) = events.last() {
                st.last_seq = last.project_seq;
            }
            if !events.is_empty() {
                if events.len() == 50 {
                    st.started = false;
                } else if is_terminal {
                    st.closed = true;
                }
                let raw_payload = serde_json::to_string(&events).unwrap_or_else(|_| "[]".into());
                let payload = super::chat_cas_storage::redact_secrets(&raw_payload);
                let ev = Event::default()
                    .id(st.last_seq.to_string())
                    .event("execution_events")
                    .data(payload);
                return Some((Ok(ev), st));
            }
            if is_terminal {
                return None;
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
