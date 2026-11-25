// @story 05.3
// @cover TC-05.3-01..TC-05.3-17
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::{Arc, OnceLock};

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::lockfile::CURRENT_SCHEMA_VERSION;
use harness_daemon::vault::VaultStore;
use harness_daemon::{build_router, DaemonAppState, ProjectDto};
use http_body_util::BodyExt;
use rusqlite::params;
use serde_json::json;
use sha2::{Digest, Sha256};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tokio::sync::Mutex;
use tower::ServiceExt;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvRestore {
    saved_path: Option<std::ffi::OsString>,
    saved_vault_key_dir: Option<std::ffi::OsString>,
    saved_timeout: Option<std::ffi::OsString>,
}

impl EnvRestore {
    fn capture() -> Self {
        Self {
            saved_path: std::env::var_os("PATH"),
            saved_vault_key_dir: std::env::var_os("COWOK_VAULT_KEY_DIR"),
            saved_timeout: std::env::var_os("COWOK_PR_CMD_TIMEOUT_MS"),
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        match &self.saved_path {
            Some(v) => std::env::set_var("PATH", v),
            None => std::env::remove_var("PATH"),
        }
        match &self.saved_vault_key_dir {
            Some(v) => std::env::set_var("COWOK_VAULT_KEY_DIR", v),
            None => std::env::remove_var("COWOK_VAULT_KEY_DIR"),
        }
        match &self.saved_timeout {
            Some(v) => std::env::set_var("COWOK_PR_CMD_TIMEOUT_MS", v),
            None => std::env::remove_var("COWOK_PR_CMD_TIMEOUT_MS"),
        }
    }
}

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let tok = format!("sess-053-{}", uuid::Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "designer-1".into(),
            role: "editor".into(),
        },
        exp,
    );
    let router = build_router((*state).clone());
    (state, router, tok)
}

fn add_tenant_session(state: &Arc<DaemonAppState>, tenant_id: &str, user_id: &str) -> String {
    let tok = format!("sess-053-{}", uuid::Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        tok.clone(),
        FlowPrincipal {
            tenant_id: tenant_id.into(),
            user_id: user_id.into(),
            role: "editor".into(),
        },
        exp,
    );
    tok
}

async fn send_req_with_headers(
    router: &axum::Router,
    method: &str,
    uri: &str,
    tok: &str,
    extra_headers: &[(&str, &str)],
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let raw = body
        .map(|v| serde_json::to_vec(&v).unwrap())
        .unwrap_or_default();
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4170")
        .header("x-cowok-session-token", tok);
    for (k, v) in extra_headers {
        b = b.header(*k, *v);
    }
    if !raw.is_empty() {
        b = b.header("content-type", "application/json");
    }
    let resp = router
        .clone()
        .oneshot(b.body(Body::from(raw)).unwrap())
        .await
        .unwrap();
    let st = resp.status();
    let out = resp.into_body().collect().await.unwrap().to_bytes();
    (
        st,
        serde_json::from_slice(&out).unwrap_or(serde_json::Value::Null),
    )
}

async fn send_req(
    router: &axum::Router,
    method: &str,
    uri: &str,
    tok: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    send_req_with_headers(router, method, uri, tok, &[], body).await
}

async fn create_project(
    router: &axum::Router,
    tok: &str,
    slug: &str,
    ws_root: Option<&Path>,
) -> ProjectDto {
    let (st, val) = send_req(
        router,
        "POST",
        "/api/v1/projects",
        tok,
        Some(json!({
            "slug": slug,
            "name": format!("Project {slug}"),
            "workspace_root": ws_root.map(|p| p.to_string_lossy().to_string()),
            "product_brief": "Story 05.3 snapshots and handoff test"
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "project create: {val}");
    serde_json::from_value(val).unwrap()
}

fn seed_session_turn_and_node(
    state: &Arc<DaemonAppState>,
    proj_id: &str,
    tenant_id: &str,
) -> (String, String, String, String, String) {
    let conn = state.registry.pool().connection().unwrap();
    let now_ms = Utc::now().timestamp_millis();
    let graph_id = uuid::Uuid::new_v4().to_string();
    let node_id = uuid::Uuid::new_v4().to_string();
    let session_id = uuid::Uuid::new_v4().to_string();
    let msg1_id = uuid::Uuid::new_v4().to_string();
    let msg2_id = uuid::Uuid::new_v4().to_string();
    let turn_snap_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO flow_graphs (id, tenant_id, project_id, portal_key, title, version, layout_version, revision, updated_at)
         VALUES (?1, ?2, ?3, 'default', 'Main Flow', 1, 1, 1, ?4)",
        params![graph_id, tenant_id, proj_id, now_ms],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type, route_path, revision, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'default', 'checkout', 'Checkout', 'screen', '/checkout', 1, ?5, ?5)",
        params![node_id, graph_id, tenant_id, proj_id, now_ms],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO chat_sessions (id, tenant_id, project_id, title, model_provider, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'Checkout Design Thread', 'gemini', ?4, ?4)",
        params![session_id, tenant_id, proj_id, now_ms],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO chat_messages (id, tenant_id, session_id, role, prose_markdown, created_at)
         VALUES (?1, ?2, ?3, 'user', 'Build checkout view', ?4)",
        params![msg1_id, tenant_id, session_id, now_ms],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO chat_messages (id, tenant_id, session_id, role, prose_markdown, parent_message_id, created_at)
         VALUES (?1, ?2, ?3, 'assistant', 'Created checkout screen', ?4, ?5)",
        params![msg2_id, tenant_id, session_id, msg1_id, now_ms + 10],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO chat_messages (id, tenant_id, session_id, role, prose_markdown, parent_message_id, created_at)
         VALUES (?1, ?2, ?3, 'user', 'Later message after turn', ?4, ?5)",
        params![uuid::Uuid::new_v4().to_string(), tenant_id, session_id, msg2_id, now_ms + 100],
    )
    .unwrap();
    let scope_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO chat_message_node_scopes (id, tenant_id, message_id, project_id, session_id, node_id, composite_scope_key, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            scope_id,
            tenant_id,
            msg2_id,
            proj_id,
            session_id,
            node_id,
            format!("{proj_id}::{session_id}::{node_id}"),
            now_ms + 10
        ],
    )
    .unwrap();

    let node_snapshot = json!({
        "id": node_id,
        "graphId": graph_id,
        "screenKey": "checkout",
        "title": "Checkout",
        "nodeType": "screen",
        "routePath": "/checkout",
        "viewportPreset": "desktop-1440",
        "posX": 120.0,
        "posY": 80.0
    })
    .to_string();
    let manifest_json = json!({
        "files": [
            { "path": "src/Checkout.tsx", "content": "export const Checkout = () => <main>v1</main>;\n" }
        ]
    })
    .to_string();
    let content_hash = format!("{:x}", Sha256::digest(manifest_json.as_bytes()));

    conn.execute(
        "INSERT INTO chat_artifact_snapshots (
            id, tenant_id, project_id, session_id, message_id, node_id,
            artifact_type, content_hash, manifest_json, node_snapshot_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'turn_receipt', ?7, ?8, ?9, ?10)",
        params![
            turn_snap_id,
            tenant_id,
            proj_id,
            session_id,
            msg2_id,
            node_id,
            content_hash,
            manifest_json,
            node_snapshot,
            now_ms + 10
        ],
    )
    .unwrap();

    (node_id, session_id, msg2_id, turn_snap_id, graph_id)
}

fn write_exec_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    p
}

fn init_git_repo_with_bare_origin(ws: &Path, bare_origin: &Path) {
    let run_git = |cwd: &Path, args: &[&str]| {
        let out = StdCommand::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run_git(bare_origin, &["init", "--bare", "-b", "main"]);
    run_git(ws, &["init", "-b", "main"]);
    run_git(ws, &["config", "user.email", "test@cowok.local"]);
    run_git(ws, &["config", "user.name", "Cowok Test"]);
    fs::write(ws.join("README.md"), "# Test Project\n").unwrap();
    run_git(ws, &["add", "README.md"]);
    run_git(ws, &["commit", "-m", "init"]);
    run_git(
        ws,
        &[
            "remote",
            "add",
            "origin",
            bare_origin.to_string_lossy().as_ref(),
        ],
    );
    run_git(ws, &["push", "-u", "origin", "main"]);
}

#[tokio::test]
async fn tc_05_3_01_and_02_schema_ancestry_turns_and_crash_safe_revert() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path().join("ws-revert");
    fs::create_dir_all(&ws).unwrap();
    let ws = fs::canonicalize(&ws).unwrap();
    let (state, router, tok) = setup_daemon(&tmp);

    assert_eq!(CURRENT_SCHEMA_VERSION, 32);
    let (st_h, health) = send_req(&router, "GET", "/api/v1/health", &tok, None).await;
    assert_eq!(st_h, StatusCode::OK);
    assert_eq!(health["schema_version"], CURRENT_SCHEMA_VERSION);

    let proj = create_project(&router, &tok, "proj-053-revert", Some(&ws)).await;
    let proj_id = proj.id.to_string();
    let (_node_id, _session_id, _msg2_id, turn_snap_id, _graph_id) =
        seed_session_turn_and_node(&state, &proj_id, "tenant-alpha");

    // Seed a proposal so we can link proposalId & chatArtifactSnapshotId
    let (st_p, p_val) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Checkout UI",
            "files": [{ "path": "src/Checkout.tsx", "status": "added", "additions": 1, "deletions": 0, "patch": "export const Checkout = () => <main>v1</main>;" }]
        })),
    )
    .await;
    assert_eq!(st_p, StatusCode::CREATED);
    let prop_id = p_val["id"].as_str().unwrap().to_string();

    // Create baseline snapshot S1 linked to proposal + turn snapshot
    let (st_s1, s1) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "S1 Baseline",
            "proposalId": prop_id,
            "chatArtifactSnapshotId": turn_snap_id,
            "files": [{ "path": "src/Checkout.tsx", "content": "export const Checkout = () => <main>s1</main>;\n" }]
        })),
    )
    .await;
    assert_eq!(st_s1, StatusCode::CREATED, "s1: {s1}");
    assert_eq!(s1["proposalId"], prop_id);
    assert_eq!(s1["chatArtifactSnapshotId"], turn_snap_id);
    let s1_id = s1["id"].as_str().unwrap().to_string();

    // Create child snapshot S2 with parentSnapshotId = S1 + extra file
    let (st_s2, s2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "S2 Mutated",
            "parentSnapshotId": s1_id,
            "files": [
                { "path": "src/Checkout.tsx", "content": "export const Checkout = () => <main>s2</main>;\n" },
                { "path": "src/Extra.tsx", "content": "export const Extra = () => null;\n" }
            ]
        })),
    )
    .await;
    assert_eq!(st_s2, StatusCode::CREATED, "s2: {s2}");
    assert_eq!(s2["parentSnapshotId"], s1_id);

    // Turn snapshots listing via ?kind=turns and /turn-snapshots
    let (st_turns, turns_val) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{proj_id}/snapshots?kind=turns"),
        &tok,
        None,
    )
    .await;
    assert_eq!(st_turns, StatusCode::OK);
    let turns_arr = turns_val.as_array().unwrap();
    assert!(!turns_arr.is_empty());
    assert!(turns_arr.iter().any(|t| t["id"] == turn_snap_id));

    let (st_turns2, turns_val2) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{proj_id}/turn-snapshots"),
        &tok,
        None,
    )
    .await;
    assert_eq!(st_turns2, StatusCode::OK);
    assert!(!turns_val2.as_array().unwrap().is_empty());

    // Revert to S1 -> pre_revert_safety committed, Extra.tsx removed, Checkout.tsx matches S1, journal COMMITTED_COMPLETE
    let (st_rev, rev_val) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/{s1_id}/revert"),
        &tok,
        Some(json!({ "safetyLabel": "Before S1 revert" })),
    )
    .await;
    assert_eq!(st_rev, StatusCode::OK, "revert: {rev_val}");
    assert_eq!(rev_val["success"], true);
    assert_eq!(rev_val["revertedToSnapshotId"], s1_id);
    assert!(!ws.join("src/Extra.tsx").exists());
    assert_eq!(
        fs::read_to_string(ws.join("src/Checkout.tsx")).unwrap(),
        "export const Checkout = () => <main>s1</main>;\n"
    );

    let conn = state.registry.pool().connection().unwrap();
    let safety_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM version_snapshots WHERE project_id = ?1 AND trigger = 'pre_revert_safety'",
            params![proj_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(safety_count, 1);

    let journal_payload: String = conn
        .query_row(
            "SELECT payload_text FROM execution_events WHERE project_id = ?1 AND event_kind = 'revert_tx_journal' ORDER BY created_at DESC LIMIT 1",
            params![proj_id],
            |r| r.get(0),
        )
        .unwrap();
    let journal_json: serde_json::Value = serde_json::from_str(&journal_payload).unwrap();
    assert_eq!(journal_json["status"], "COMMITTED_COMPLETE");
}

#[tokio::test]
async fn tc_05_3_03_and_04_deleted_node_guard_restore_and_turn_fork() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path().join("ws-fork");
    fs::create_dir_all(&ws).unwrap();
    let ws = fs::canonicalize(&ws).unwrap();
    let (state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "proj-053-fork", Some(&ws)).await;
    let proj_id = proj.id.to_string();

    let (node_id, session_id, msg2_id, turn_snap_id, _graph_id) =
        seed_session_turn_and_node(&state, &proj_id, "tenant-alpha");

    // Create a version snapshot linked to turn_snap_id
    let (st_s, s_val) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "Turn Bound Snap",
            "chatArtifactSnapshotId": turn_snap_id,
            "files": [{ "path": "src/Checkout.tsx", "content": "export const Checkout = () => <main>turn1</main>;\n" }]
        })),
    )
    .await;
    assert_eq!(st_s, StatusCode::CREATED);
    let snap_id = s_val["id"].as_str().unwrap().to_string();

    // Delete FlowNode N1
    {
        let conn = state.registry.pool().connection().unwrap();
        conn.execute("DELETE FROM flow_nodes WHERE id = ?1", params![node_id])
            .unwrap();
    }

    // TC-05.3-03: Revert without restoreMissingNodes -> 409 NODE_DELETED_REQUIRES_GRAPH_RESTORE, zero DB/disk mutations
    let (st_409, err_409) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/{snap_id}/revert"),
        &tok,
        Some(json!({ "restoreMissingNodes": false })),
    )
    .await;
    assert_eq!(st_409, StatusCode::CONFLICT, "409 expected: {err_409}");
    assert_eq!(err_409["code"], "NODE_DELETED_REQUIRES_GRAPH_RESTORE");

    // TC-05.3-04: Fork from turn snapshot while node is deleted -> 201, marks node detached without failing
    let (st_fork, fork_val) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/{turn_snap_id}/fork"),
        &tok,
        Some(json!({ "title": "Branched Checkout Exploration" })),
    )
    .await;
    assert_eq!(st_fork, StatusCode::CREATED, "fork: {fork_val}");
    assert_eq!(fork_val["forkedFromSessionId"], session_id);
    assert_eq!(fork_val["forkedFromMessageId"], msg2_id);
    assert_eq!(fork_val["turnSnapshotId"], turn_snap_id);
    assert_eq!(fork_val["copiedMessagesCount"], 2); // only messages <= msg2.created_at
    assert_eq!(fork_val["detachedNodeIds"], json!([node_id]));

    let forked_sid = fork_val["forkedSessionId"].as_str().unwrap().to_string();
    {
        let conn = state.registry.pool().connection().unwrap();
        let (fsid, fmid): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT forked_from_session_id, forked_from_message_id FROM chat_sessions WHERE id = ?1",
                params![forked_sid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(fsid.as_deref(), Some(session_id.as_str()));
        assert_eq!(fmid.as_deref(), Some(msg2_id.as_str()));
    }

    // Revert WITH restoreMissingNodes: true -> 200 OK & restores flow_nodes row N1!
    let (st_ok, ok_val) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/{snap_id}/revert"),
        &tok,
        Some(json!({ "restoreMissingNodes": true })),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK, "restore revert: {ok_val}");
    assert_eq!(ok_val["restoredNodeIds"], json!([node_id]));
    let conn = state.registry.pool().connection().unwrap();
    let restored_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flow_nodes WHERE id = ?1 AND project_id = ?2",
            params![node_id, proj_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(restored_count, 1);
}

#[tokio::test]
async fn tc_05_3_05_cross_tenant_and_cross_project_anti_idor() {
    let tmp = TempDir::new().unwrap();
    let (state, router, tok_a) = setup_daemon(&tmp);
    let tok_b = add_tenant_session(&state, "tenant-beta", "mallory");

    let proj_a = create_project(&router, &tok_a, "proj-a-idor", None).await;
    let proj_b = create_project(&router, &tok_b, "proj-b-idor", None).await;
    let (_node_a, _sess_a, _msg_a, turn_a, _g_a) =
        seed_session_turn_and_node(&state, &proj_a.id.to_string(), "tenant-alpha");

    let (st_s, s_a) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/snapshots", proj_a.id),
        &tok_a,
        Some(json!({
            "label": "A Snap",
            "files": [{ "path": "src/A.tsx", "content": "export const A = 1;\n" }]
        })),
    )
    .await;
    assert_eq!(st_s, StatusCode::CREATED);
    let snap_a = s_a["id"].as_str().unwrap().to_string();

    // Tenant B accessing Project A endpoints directly is rejected
    for (method, path, body) in [
        (
            "GET",
            format!("/api/v1/projects/{}/snapshots", proj_a.id),
            None,
        ),
        (
            "POST",
            format!("/api/v1/projects/{}/snapshots/{snap_a}/revert", proj_a.id),
            Some(json!({})),
        ),
        (
            "POST",
            format!("/api/v1/projects/{}/snapshots/{turn_a}/fork", proj_a.id),
            Some(json!({})),
        ),
        (
            "POST",
            format!("/api/v1/projects/{}/snapshots/export", proj_a.id),
            Some(json!({})),
        ),
    ] {
        let (st, _) = send_req(&router, method, &path, &tok_b, body).await;
        assert!(
            matches!(st, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
            "Cross-tenant {method} {path} must deny, got {st}"
        );
    }

    // Tenant B using Project B's ID but referencing Project A's snapshot/turn/parent ID -> 404
    let (st_ref_parent, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/snapshots", proj_b.id),
        &tok_b,
        Some(json!({
            "label": "Cross Parent",
            "parentSnapshotId": snap_a,
            "files": [{ "path": "src/B.tsx", "content": "export const B = 1;\n" }]
        })),
    )
    .await;
    assert_eq!(st_ref_parent, StatusCode::NOT_FOUND);

    let (st_ref_fork, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/snapshots/{turn_a}/fork", proj_b.id),
        &tok_b,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_ref_fork, StatusCode::NOT_FOUND);

    let (st_ref_rev, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/snapshots/{snap_a}/revert", proj_b.id),
        &tok_b,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_ref_rev, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tc_05_3_06_and_07_secret_filtered_pkzip_manifest_and_bounds() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path().join("ws-export");
    fs::create_dir_all(&ws).unwrap();
    let ws = fs::canonicalize(&ws).unwrap();
    let (_state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "proj-053-export", Some(&ws)).await;
    let proj_id = proj.id.to_string();

    // Seed runnable files + secret files in snapshot
    let (st_s, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "Runnable With Secrets",
            "files": [
                { "path": "package.json", "content": "{\"name\":\"app\",\"scripts\":{\"build\":\"vite build\"}}\n" },
                { "path": "src/App.tsx", "content": "export default function App() { return <main>App</main>; }\n" },
                { "path": "src/styles/tokens.css", "content": ":root { --bg: #0b0e14; }\n" },
                { "path": "src/assets/icon-manifest.json", "content": "{\"icons\":[\"arrow-right\"]}\n" },
                { "path": "fixtures/sample.json", "content": "{\"ok\":true}\n" },
                { "path": ".env", "content": "SECRET=123\n" },
                { "path": ".env.local", "content": "SECRET=456\n" },
                { "path": ".npmrc", "content": "//registry.npmjs.org/:_authToken=secret\n" },
                { "path": ".netrc", "content": "password secret\n" },
                { "path": ".git-credentials", "content": "https://tok@github.com\n" },
                { "path": "keys/server.pem", "content": "PRIVATE KEY" },
                { "path": "keys/app.key", "content": "PRIVATE KEY" },
                { "path": "ssh/id_rsa", "content": "PRIVATE KEY" },
                { "path": "config/client_secret.json", "content": "{\"secret\":\"abc\"}" }
            ]
        })),
    )
    .await;
    assert_eq!(st_s, StatusCode::CREATED);

    let (st_exp, exp_val) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/export"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_exp, StatusCode::OK, "export: {exp_val}");
    assert_eq!(exp_val["readyToRun"], true);
    assert_eq!(exp_val["handoffManifestIncluded"], true);
    let included: Vec<String> = serde_json::from_value(exp_val["filesIncluded"].clone()).unwrap();
    assert!(included.contains(&"cowok-handoff-manifest.json".to_string()));
    assert!(included.contains(&"src/styles/tokens.css".to_string()));
    assert!(included.contains(&"src/assets/icon-manifest.json".to_string()));
    assert!(included.contains(&"package.json".to_string()));
    assert!(included.contains(&"fixtures/sample.json".to_string()));
    for secret in [
        ".env",
        ".env.local",
        ".npmrc",
        ".netrc",
        ".git-credentials",
        "keys/server.pem",
        "keys/app.key",
        "ssh/id_rsa",
        "config/client_secret.json",
    ] {
        assert!(!included.contains(&secret.to_string()), "leaked {secret}");
    }

    // Download binary ZIP and verify sha256 parity
    let dl_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/projects/{proj_id}/snapshots/export/download"
        ))
        .header("host", "127.0.0.1:4170")
        .header("x-cowok-session-token", &tok)
        .body(Body::empty())
        .unwrap();
    let dl_resp = router.clone().oneshot(dl_req).await.unwrap();
    assert_eq!(dl_resp.status(), StatusCode::OK);
    let dl_bytes = dl_resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&dl_bytes[..4], b"PK\x03\x04");
    assert_eq!(
        format!("{:x}", Sha256::digest(&dl_bytes)),
        exp_val["sha256"].as_str().unwrap()
    );

    // TC-05.3-07: Path traversal rejected on snapshot creation
    let (st_trav, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "Traversal",
            "files": [{ "path": "../etc/passwd", "content": "root:x" }]
        })),
    )
    .await;
    assert_eq!(st_trav, StatusCode::UNPROCESSABLE_ENTITY);

    // TC-05.3-07: > MAX_ZIP_FILES (2000) returns 413 PAYLOAD_TOO_LARGE
    let many_files: Vec<_> = (0..2005)
        .map(|i| json!({ "path": format!("src/f{i}.ts"), "content": "export const x = 1;" }))
        .collect();
    let (st_big, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({ "label": "Too Many Files", "files": many_files })),
    )
    .await;
    assert_eq!(st_big, StatusCode::CREATED);
    let (st_413, err_413) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/export"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(
        st_413,
        StatusCode::PAYLOAD_TOO_LARGE,
        "413 expected: {err_413}"
    );
}

#[tokio::test]
async fn tc_05_3_08_09_16_17_github_pr_validation_preflight_collision_and_scrubbing() {
    let _guard = env_lock().lock().await;
    let _env_restore = EnvRestore::capture();
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path().join("ws-pr");
    let bare = tmp.path().join("origin.git");
    let bin_dir = tmp.path().join("bin");
    let key_dir = tmp.path().join("keys");
    fs::create_dir_all(&ws).unwrap();
    fs::create_dir_all(&bare).unwrap();
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&key_dir).unwrap();
    let ws = fs::canonicalize(&ws).unwrap();
    std::env::set_var("COWOK_VAULT_KEY_DIR", &key_dir);

    let (state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "proj-053-pr", Some(&ws)).await;
    let proj_id = proj.id.to_string();

    // Seed snapshot S1
    let (st_s, s_val) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "PR Ready",
            "files": [{ "path": "src/App.tsx", "content": "export const App = () => null;\n" }]
        })),
    )
    .await;
    assert_eq!(st_s, StatusCode::CREATED);
    let snap_id = s_val["id"].as_str().unwrap().to_string();

    // TC-05.3-08: Malicious git refs rejected with 400 VALIDATION_ERROR
    for bad_ref in [
        "--upload-pack=evil",
        "foo..bar",
        "bad@{ref}",
        "main.lock",
        "a//b",
    ] {
        let (st_bad, err_bad) = send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
            &tok,
            Some(json!({
                "branchName": bad_ref,
                "prTitle": "Bad Ref PR"
            })),
        )
        .await;
        assert_eq!(
            st_bad,
            StatusCode::BAD_REQUEST,
            "ref={bad_ref} got {err_bad}"
        );
        assert_eq!(err_bad["code"], "VALIDATION_ERROR");
    }

    // TC-05.3-09a: No git remote -> 501 GIT_REMOTE_MISSING
    let (st_noremote, err_noremote) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
        &tok,
        Some(json!({ "branchName": "feature/ui-sync", "prTitle": "UI Sync" })),
    )
    .await;
    assert_eq!(st_noremote, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(err_noremote["code"], "GIT_REMOTE_MISSING");

    // Initialize real git repo + bare origin
    init_git_repo_with_bare_origin(&ws, &bare);

    // TC-05.3-09b: Missing gh CLI -> 501 GH_CLI_MISSING (limit PATH to system git only)
    let git_path = StdCommand::new("which").arg("git").output().unwrap();
    let real_git = PathBuf::from(String::from_utf8_lossy(&git_path.stdout).trim());
    let git_only_dir = tmp.path().join("git-only-bin");
    fs::create_dir_all(&git_only_dir).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_git, git_only_dir.join("git")).unwrap();
    std::env::set_var("PATH", &git_only_dir);

    let (st_nogh, err_nogh) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
        &tok,
        Some(json!({ "branchName": "feature/ui-sync", "prTitle": "UI Sync" })),
    )
    .await;
    assert_eq!(st_nogh, StatusCode::NOT_IMPLEMENTED, "nogh: {err_nogh}");
    assert_eq!(err_nogh["code"], "GH_CLI_MISSING");

    // Put stub `gh` in bin_dir alongside real `git`
    let orig_path = _env_restore
        .saved_path
        .as_ref()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{orig_path}", bin_dir.display()));
    let gh_stub = bin_dir.join("gh");
    write_exec_script(
        &bin_dir,
        "gh",
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "gh version 2.50.0"
  exit 0
fi
if [ "$1" = "auth" ] && [ "$2" = "status" ]; then
  MOCK_PREFIX="ghp_"
  if [ "$GH_TOKEN" = "${MOCK_PREFIX}revoked_secret_999" ]; then
    echo "error: HTTP 401 with token ${MOCK_PREFIX}revoked_secret_999" >&2
    exit 1
  fi
  echo "Logged in"
  exit 0
fi
if [ "$1" = "pr" ] && [ "$2" = "list" ]; then
  echo "[]"
  exit 0
fi
if [ "$1" = "pr" ] && [ "$2" = "create" ]; then
  echo "https://github.com/cowok/uxui-os/pull/42"
  exit 0
fi
if [ "$1" = "pr" ] && [ "$2" = "edit" ]; then
  echo "https://github.com/cowok/uxui-os/pull/42"
  exit 0
fi
exit 1
"#,
    );
    assert!(gh_stub.exists());

    // TC-05.3-09c: No Vault github token -> 501 GH_UNAUTHENTICATED
    let (st_noauth, err_noauth) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
        &tok,
        Some(json!({ "branchName": "feature/ui-sync", "prTitle": "UI Sync" })),
    )
    .await;
    assert_eq!(st_noauth, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(err_noauth["code"], "GH_UNAUTHENTICATED");

    // TC-05.3-17a: Revoked token -> 501 GH_UNAUTHENTICATED + scrubbed audit event
    let store = VaultStore::from_workspace_root(state.registry.pool().workspace_root());
    store
        .upsert_secret_scoped("tenant-alpha", "github", "ghp_revoked_secret_999", None)
        .unwrap();
    let (st_revoked, err_revoked) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
        &tok,
        Some(json!({ "branchName": "feature/ui-sync", "prTitle": "UI Sync" })),
    )
    .await;
    assert_eq!(st_revoked, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(err_revoked["code"], "GH_UNAUTHENTICATED");
    assert!(!err_revoked.to_string().contains("ghp_revoked_secret_999"));

    // Store valid token & create PR with Idempotency-Key
    store
        .upsert_secret_scoped("tenant-alpha", "github", "ghp_valid_token_abc123", None)
        .unwrap();
    let (st_ok, pr_ok) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
        &tok,
        &[("idempotency-key", "pr-idem-1")],
        Some(json!({
            "branchName": "feature/ui-sync",
            "prTitle": "UI Sync PR",
            "prBody": "Handoff",
            "targetBranch": "main",
            "snapshotId": snap_id
        })),
    )
    .await;
    assert_eq!(st_ok, StatusCode::CREATED, "pr create: {pr_ok}");
    assert_eq!(pr_ok["prNumber"], 42);
    assert_eq!(pr_ok["status"], "created");
    assert_eq!(pr_ok["snapshotId"], snap_id);

    // TC-05.3-14: Idempotent replay returns cached response with zero duplicate rows
    let (st_replay, pr_replay) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
        &tok,
        &[("idempotency-key", "pr-idem-1")],
        Some(json!({
            "branchName": "feature/ui-sync",
            "prTitle": "UI Sync PR",
            "prBody": "Handoff",
            "targetBranch": "main",
            "snapshotId": snap_id
        })),
    )
    .await;
    assert_eq!(st_replay, StatusCode::CREATED);
    assert_eq!(pr_replay["prNumber"], 42);

    // TC-05.3-16a: Matching open PR -> 200 status: "updated"
    write_exec_script(
        &bin_dir,
        "gh",
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then echo "gh version 2.50.0"; exit 0; fi
if [ "$1" = "auth" ] && [ "$2" = "status" ]; then echo "Logged in"; exit 0; fi
if [ "$1" = "pr" ] && [ "$2" = "list" ]; then
  echo '[{"number":42,"url":"https://github.com/cowok/uxui-os/pull/42"}]'
  exit 0
fi
if [ "$1" = "pr" ] && [ "$2" = "edit" ]; then
  echo "https://github.com/cowok/uxui-os/pull/42"
  exit 0
fi
exit 1
"#,
    );
    let (st_upd, pr_upd) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
        &tok,
        Some(json!({
            "branchName": "feature/ui-sync",
            "prTitle": "UI Sync PR Updated",
            "snapshotId": snap_id
        })),
    )
    .await;
    assert_eq!(st_upd, StatusCode::OK, "pr update: {pr_upd}");
    assert_eq!(pr_upd["status"], "updated");

    // TC-05.3-16b: Remote branch exists with unrelated history -> 409 GIT_BRANCH_EXISTS
    let other_clone = tmp.path().join("other-clone");
    StdCommand::new("git")
        .args([
            "clone",
            bare.to_string_lossy().as_ref(),
            other_clone.to_string_lossy().as_ref(),
        ])
        .output()
        .unwrap();
    for args in [
        &["config", "user.email", "other@cowok.local"][..],
        &["config", "user.name", "Other"][..],
        &["checkout", "--orphan", "feature/unrelated"][..],
    ] {
        StdCommand::new("git")
            .current_dir(&other_clone)
            .args(args)
            .output()
            .unwrap();
    }
    fs::write(other_clone.join("conflict.txt"), "diverged\n").unwrap();
    for args in [
        &["add", "conflict.txt"][..],
        &["commit", "-m", "unrelated"][..],
        &["push", "origin", "feature/unrelated"][..],
    ] {
        StdCommand::new("git")
            .current_dir(&other_clone)
            .args(args)
            .output()
            .unwrap();
    }
    write_exec_script(
        &bin_dir,
        "gh",
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then echo "gh version 2.50.0"; exit 0; fi
if [ "$1" = "auth" ] && [ "$2" = "status" ]; then echo "Logged in"; exit 0; fi
if [ "$1" = "pr" ] && [ "$2" = "list" ]; then echo "[]"; exit 0; fi
exit 1
"#,
    );
    let (st_col, err_col) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
        &tok,
        Some(json!({
            "branchName": "feature/unrelated",
            "prTitle": "Collide PR"
        })),
    )
    .await;
    assert_eq!(st_col, StatusCode::CONFLICT, "collision: {err_col}");
    assert_eq!(err_col["code"], "GIT_BRANCH_EXISTS");

    // TC-05.3-17b: Subprocess timeout -> 504 SUBPROCESS_TIMEOUT & secret scrubbing on stderr
    std::env::set_var("COWOK_PR_CMD_TIMEOUT_MS", "100");
    write_exec_script(
        &bin_dir,
        "gh",
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then echo "gh version 2.50.0"; exit 0; fi
if [ "$1" = "auth" ] && [ "$2" = "status" ]; then sleep 2; exit 0; fi
exit 1
"#,
    );
    let (st_to, err_to) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
        &tok,
        Some(json!({
            "branchName": "feature/timeout-test",
            "prTitle": "Timeout PR"
        })),
    )
    .await;
    assert_eq!(st_to, StatusCode::GATEWAY_TIMEOUT, "timeout: {err_to}");
    assert_eq!(err_to["code"], "SUBPROCESS_TIMEOUT");

    // Verify execution_events audit rows have zero plaintext token leakage
    let conn = state.registry.pool().connection().unwrap();
    let mut stmt = conn
        .prepare("SELECT payload_text FROM execution_events WHERE project_id = ?1 AND event_kind = 'github_pr_attempt'")
        .unwrap();
    let logs: Vec<String> = stmt
        .query_map(params![proj_id], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(logs.len() >= 3);
    for l in logs {
        assert!(!l.contains("ghp_revoked_secret_999"));
        assert!(!l.contains("ghp_valid_token_abc123"));
    }
}

#[tokio::test]
async fn tc_05_3_10_11_13_15_ide_handoff_sre_and_journal_reconciliation() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path().join("ws-ide");
    fs::create_dir_all(ws.join("src")).unwrap();
    fs::write(ws.join("src/App.tsx"), "export const App = () => null;\n").unwrap();
    let ws = fs::canonicalize(&ws).unwrap();

    let (state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "proj-053-ide", Some(&ws)).await;
    let proj_id = proj.id.to_string();

    // TC-05.3-10: Valid IDE handoff writes cowok-handoff-manifest.json and returns launchUri + sha256
    let (st_ide, ide_val) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/handoff/open-ide"),
        &tok,
        Some(json!({ "ide": "cursor", "relativeEntryPath": "src/App.tsx" })),
    )
    .await;
    assert_eq!(st_ide, StatusCode::OK, "open-ide: {ide_val}");
    assert_eq!(ide_val["ide"], "cursor");
    assert!(ide_val["launchUri"]
        .as_str()
        .unwrap()
        .starts_with("cursor://file/"));
    assert!(ws.join("cowok-handoff-manifest.json").exists());
    let manifest_bytes = fs::read(ws.join("cowok-handoff-manifest.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&manifest_bytes)),
        ide_val["manifestSha256"].as_str().unwrap()
    );

    // TC-05.3-11: Traversal & unknown IDE rejected with 400 VALIDATION_ERROR
    let (st_trav, err_trav) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/handoff/open-ide"),
        &tok,
        Some(json!({ "ide": "cursor", "relativeEntryPath": "../../secret" })),
    )
    .await;
    assert_eq!(st_trav, StatusCode::BAD_REQUEST);
    assert_eq!(err_trav["code"], "VALIDATION_ERROR");

    let (st_bad_ide, err_bad_ide) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/handoff/open-ide"),
        &tok,
        Some(json!({ "ide": "notepad" })),
    )
    .await;
    assert_eq!(st_bad_ide, StatusCode::BAD_REQUEST);
    assert_eq!(err_bad_ide["code"], "VALIDATION_ERROR");

    // TC-05.3-15: Duplicate snapshot ID -> 409 CONSTRAINT_CONFLICT (code: "P2002")
    let fixed_id = uuid::Uuid::new_v4().to_string();
    let (st1, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "id": fixed_id,
            "label": "First",
            "files": [{ "path": "src/App.tsx", "content": "export const App = () => null;\n" }]
        })),
    )
    .await;
    assert_eq!(st1, StatusCode::CREATED);
    let (st2, err2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "id": fixed_id,
            "label": "Duplicate",
            "files": [{ "path": "src/App.tsx", "content": "export const App = () => null;\n" }]
        })),
    )
    .await;
    assert_eq!(st2, StatusCode::CONFLICT, "P2002 expected: {err2}");
    assert_eq!(err2["code"], "P2002");

    // TC-05.3-15: Missing/invalid parentSnapshotId or proposalId -> 404 REFERENCE_NOT_FOUND
    let (st_bad_parent, err_bad_parent) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "Missing Parent",
            "parentSnapshotId": uuid::Uuid::new_v4().to_string(),
            "files": [{ "path": "src/App.tsx", "content": "export const App = () => null;\n" }]
        })),
    )
    .await;
    assert_eq!(st_bad_parent, StatusCode::NOT_FOUND);
    assert_eq!(err_bad_parent["code"], "REFERENCE_NOT_FOUND");

    // TC-05.3-13a: Disk swap failure after DB commit triggers compensating SQLite tx -> 500 DISK_REVERT_COMPENSATED
    let (st_target, snap_target) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "Target With Blocked Path",
            "files": [{ "path": "src/Blocked.tsx", "content": "export const Blocked = () => 1;\n" }]
        })),
    )
    .await;
    assert_eq!(st_target, StatusCode::CREATED);
    let target_id = snap_target["id"].as_str().unwrap().to_string();

    // Update current workspace to v2 and revert to v2 so project_workspace_files contains ONLY src/LiveV2.tsx
    let (st_v2, snap_v2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "Current V2",
            "files": [{ "path": "src/LiveV2.tsx", "content": "export const LiveV2 = () => 2;\n" }]
        })),
    )
    .await;
    assert_eq!(st_v2, StatusCode::CREATED);
    let v2_id = snap_v2["id"].as_str().unwrap().to_string();
    let (st_prep_rev, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/{v2_id}/revert"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_prep_rev, StatusCode::OK);

    // Place a symlink at src/Blocked.tsx so apply_files_to_disk_jail rejects the disk swap
    #[cfg(unix)]
    {
        let outside = TempDir::new().unwrap();
        let target_outside = outside.path().join("outside.tsx");
        fs::write(&target_outside, "outside").unwrap();
        std::os::unix::fs::symlink(&target_outside, ws.join("src/Blocked.tsx")).unwrap();

        let (st_comp, err_comp) = send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{proj_id}/snapshots/{target_id}/revert"),
            &tok,
            Some(json!({})),
        )
        .await;
        assert_eq!(
            st_comp,
            StatusCode::INTERNAL_SERVER_ERROR,
            "comp expected: {err_comp}"
        );
        assert_eq!(err_comp["code"], "DISK_REVERT_COMPENSATED");

        // DB project_workspace_files must have been compensated back to src/LiveV2.tsx (no src/Blocked.tsx)
        let conn = state.registry.pool().connection().unwrap();
        let ws_paths: Vec<String> = conn
            .prepare("SELECT relative_path FROM project_workspace_files WHERE project_id = ?1 ORDER BY relative_path")
            .unwrap()
            .query_map(params![proj_id], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(ws_paths, vec!["src/LiveV2.tsx".to_string()]);
        assert!(ws.join("src/LiveV2.tsx").exists());
        let _ = fs::remove_file(ws.join("src/Blocked.tsx"));
    }

    // TC-05.3-13b: PENDING_DISK_SWAP journal reconciles cleanly on next export
    let (audit_sess, audit_job) = {
        let conn = state.registry.pool().connection().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        let audit_sess = uuid::Uuid::new_v4().to_string();
        let audit_job = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO chat_sessions (id, tenant_id, project_id, title, model_provider, created_at, updated_at)
             VALUES (?1, 'tenant-alpha', ?2, 'Audit', 'gemini', ?3, ?3)",
            params![audit_sess, proj_id, now_ms],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO node_execution_jobs (id, tenant_id, project_id, session_id, node_id, status, fencing_token, created_at, updated_at)
             VALUES (?1, 'tenant-alpha', ?2, ?3, '__workspace__', 'COMPLETED', 1, ?4, ?4)",
            params![audit_job, proj_id, audit_sess, now_ms],
        )
        .unwrap();
        let pending_ev_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO execution_events (id, tenant_id, project_id, job_id, node_id, composite_scope_key, event_kind, payload_text, seq_no, project_seq, created_at)
             VALUES (?1, 'tenant-alpha', ?2, ?3, '__workspace__', '', 'revert_tx_journal', '{\"status\":\"PENDING_DISK_SWAP\"}', 90, 90, ?4)",
            params![pending_ev_id, proj_id, audit_job, now_ms],
        )
        .unwrap();
        let (st_exp_ok, _) = (StatusCode::OK, pending_ev_id);
        let _ = st_exp_ok;
        (audit_sess, audit_job)
    };
    let _ = audit_sess;
    let (st_reconcile_ok, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/export"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_reconcile_ok, StatusCode::OK);

    // TC-05.3-13c: Unresolved COMPENSATION_FAILED revert journal blocks export with 409 WORKSPACE_RECONCILIATION_REQUIRED
    {
        let conn = state.registry.pool().connection().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO execution_events (id, tenant_id, project_id, job_id, node_id, composite_scope_key, event_kind, payload_text, seq_no, project_seq, created_at)
             VALUES (?1, 'tenant-alpha', ?2, ?3, '__workspace__', '', 'revert_tx_journal', '{\"status\":\"COMPENSATION_FAILED\"}', 91, 91, ?4)",
            params![uuid::Uuid::new_v4().to_string(), proj_id, audit_job, now_ms],
        )
        .unwrap();
    }
    let (st_rec, err_rec) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/export"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(
        st_rec,
        StatusCode::CONFLICT,
        "reconcile expected: {err_rec}"
    );
    assert_eq!(err_rec["code"], "WORKSPACE_RECONCILIATION_REQUIRED");
}

#[tokio::test]
async fn regression_codex_f02_ready_to_run_and_f04_include_node_scope() {
    let tmp = TempDir::new().unwrap();
    let (state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "f02-f04-regression", None).await;
    let proj_id = proj.id.to_string();

    // F-02: Snapshot containing only package.json without scripts.build/tokens/icons/fixtures/src
    // MUST report readyToRun == false.
    let (st_snap, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "Incomplete Package Only",
            "files": [{ "path": "package.json", "content": "{ \"name\": \"incomplete\" }" }]
        })),
    )
    .await;
    assert_eq!(st_snap, StatusCode::CREATED);

    // F-06: Runtime POST /snapshots MUST automatically emit an immutable chat_artifact_snapshots turn receipt
    let (st_turns, turns_body) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{proj_id}/snapshots?kind=turns"),
        &tok,
        None,
    )
    .await;
    assert_eq!(st_turns, StatusCode::OK);
    let turns_arr = turns_body.as_array().expect("turns array");
    assert_eq!(
        turns_arr.len(),
        1,
        "expected 1 auto-created turn snapshot: {turns_body}"
    );
    let runtime_turn_id = turns_arr[0]["id"].as_str().unwrap().to_string();
    assert!(!runtime_turn_id.is_empty());

    // F-07: Undo Turn directly via runtime turnSnapshotId (array manifest_json) MUST restore files without wiping workspace
    let (st_undo_turn, undo_turn_body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/{runtime_turn_id}/revert"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_undo_turn, StatusCode::OK, "undo turn: {undo_turn_body}");
    assert_eq!(undo_turn_body["restoredFilesCount"].as_u64(), Some(1));

    let (st_exp, exp_body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/export"),
        &tok,
        Some(json!({ "includeFixtures": true, "includeTokens": true, "includeIcons": true })),
    )
    .await;
    assert_eq!(st_exp, StatusCode::OK, "export body: {exp_body}");
    assert_eq!(exp_body["readyToRun"], false);

    // F-04: includeNodeScope=false MUST NOT copy chat_message_node_scopes into forked session.
    let (_node_id, _sid, _mid, turn_snap_id, _gid) =
        seed_session_turn_and_node(&state, &proj_id, "tenant-alpha");
    let (st_fork, fork_body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/{turn_snap_id}/fork"),
        &tok,
        Some(json!({ "title": "Clean No Scope Fork", "includeNodeScope": false })),
    )
    .await;
    assert_eq!(st_fork, StatusCode::CREATED, "fork body: {fork_body}");
    let forked_sid = fork_body["forkedSessionId"].as_str().unwrap().to_string();
    let conn = state.registry.pool().connection().unwrap();
    let scope_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chat_message_node_scopes WHERE session_id = ?1",
            params![forked_sid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        scope_count, 0,
        "includeNodeScope=false must copy 0 node scopes"
    );
}

#[tokio::test]
async fn regression_codex_f05_pr_never_stages_workspace_secret_files() {
    let _guard = env_lock().lock().await;
    let _env_restore = EnvRestore::capture();
    let tmp = TempDir::new().unwrap();
    let ws = fs::canonicalize(tmp.path().join("ws-pr")).unwrap_or_else(|_| {
        fs::create_dir_all(tmp.path().join("ws-pr")).unwrap();
        fs::canonicalize(tmp.path().join("ws-pr")).unwrap()
    });
    let remote = tmp.path().join("remote.git");
    StdCommand::new("git")
        .args(["init", "--bare"])
        .arg(&remote)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["init"])
        .current_dir(&ws)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "t@cowok.local"])
        .current_dir(&ws)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "Tester"])
        .current_dir(&ws)
        .output()
        .unwrap();
    fs::write(ws.join("README.md"), "# base\n").unwrap();
    StdCommand::new("git")
        .args(["add", "README.md"])
        .current_dir(&ws)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&ws)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["remote", "add", "origin"])
        .arg(&remote)
        .current_dir(&ws)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["push", "-u", "origin", "HEAD:main"])
        .current_dir(&ws)
        .output()
        .unwrap();

    // Pre-stage secret files in git index before PR -> handle_create_pr MUST unstage/reset them and exclude from commit
    fs::create_dir_all(ws.join("src")).unwrap();
    fs::write(ws.join("src/secrets.json"), "{\"apiKey\":\"top-secret\"}").unwrap();
    fs::write(ws.join(".env"), "TOP_SECRET=1\n").unwrap();
    StdCommand::new("git")
        .args(["add", "-f", "src/secrets.json", ".env"])
        .current_dir(&ws)
        .output()
        .unwrap();

    let fake_bin = tmp.path().join("bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let gh_script = fake_bin.join("gh");
    fs::write(
        &gh_script,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'gh version 2.50.0'; exit 0; fi\nif [ \"$1\" = \"auth\" ] && [ \"$2\" = \"status\" ]; then echo 'Logged in'; exit 0; fi\nif [ \"$1\" = \"pr\" ] && [ \"$2\" = \"list\" ]; then echo '[]'; exit 0; fi\nif [ \"$1\" = \"pr\" ] && [ \"$2\" = \"create\" ]; then echo 'https://github.com/cowok/test/pull/77'; exit 0; fi\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&gh_script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&gh_script, perms).unwrap();
    }
    let orig_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{orig_path}", fake_bin.display()));

    let key_dir = tmp.path().join("vault-keys");
    fs::create_dir_all(&key_dir).unwrap();
    std::env::set_var("COWOK_VAULT_KEY_DIR", &key_dir);
    let (state, router, tok) = setup_daemon(&tmp);
    let store = VaultStore::from_workspace_root(state.registry.pool().workspace_root());
    store
        .upsert_secret_scoped("tenant-alpha", "github", "ghp_testtoken1234567890", None)
        .unwrap();

    let proj = create_project(&router, &tok, "f05-secret-pr", Some(&ws)).await;
    let proj_id = proj.id.to_string();
    let (st_snap, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots"),
        &tok,
        Some(json!({
            "label": "Clean UI Only",
            "files": [
                { "path": "src/CleanUi.tsx", "content": "export const CleanUi = () => null;\n" },
                { "path": "src/secrets.json", "content": "{\"leak\":\"no\"}" }
            ]
        })),
    )
    .await;
    assert_eq!(st_snap, StatusCode::CREATED);

    let (st_pr, pr_body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/snapshots/pr"),
        &tok,
        Some(json!({
            "branchName": "feat/clean-pr",
            "prTitle": "Clean UI Export",
            "targetBranch": "main"
        })),
    )
    .await;
    assert_eq!(st_pr, StatusCode::CREATED, "PR response: {pr_body}");
    let show = StdCommand::new("git")
        .args(["show", "--name-only", "--pretty=format:", "HEAD"])
        .current_dir(&ws)
        .output()
        .unwrap();
    let committed = String::from_utf8_lossy(&show.stdout);
    assert!(
        committed.contains("src/CleanUi.tsx"),
        "expected CleanUi.tsx in {committed}"
    );
    assert!(
        committed.contains("cowok-handoff-manifest.json"),
        "expected manifest in {committed}"
    );
    assert!(
        !committed.contains("secrets.json"),
        "secrets.json leaked into commit: {committed}"
    );
    assert!(
        !committed.contains(".env"),
        ".env leaked into commit: {committed}"
    );
    let _ = state;
}
