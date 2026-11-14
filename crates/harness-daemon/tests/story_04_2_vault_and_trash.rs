// @story 04.2
// @cover AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::routes::{DeleteProjectResponse, FsBrowseResponse, RestoreProjectResponse};
use harness_daemon::{build_router, DaemonAppState, PathTokenResponse, ProjectDto};
use http_body_util::BodyExt;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let tok_a = format!("sess-a-{}", uuid::Uuid::new_v4());
    let tok_b = format!("sess-b-{}", uuid::Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        tok_a.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".into(),
            user_id: "u1".into(),
            role: "editor".into(),
        },
        exp,
    );
    state.auth.insert_session(
        tok_b.clone(),
        FlowPrincipal {
            tenant_id: "tenant-b".into(),
            user_id: "u2".into(),
            role: "editor".into(),
        },
        exp,
    );
    let router = build_router((*state).clone());
    (state, router, tok_a, tok_b)
}

async fn send_req(
    router: &axum::Router,
    method: &str,
    uri: &str,
    hdrs: &[(&str, &str)],
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let raw = body
        .map(|v| serde_json::to_vec(&v).unwrap())
        .unwrap_or_default();
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4170");
    if !raw.is_empty() {
        b = b.header("content-type", "application/json");
    }
    for (k, v) in hdrs {
        b = b.header(*k, *v);
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

#[tokio::test]
async fn test_fs_browse_auth_jail_and_symlink_rejection() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let managed = tmp.path().join("CowokProjectsRoot");
    std::env::set_var("COWOK_PROJECTS_ROOT", &managed);
    let (_s, router, tok_a, _) = setup_daemon(&tmp);

    let (st, val) = send_req(
        &router,
        "GET",
        "/api/v1/fs/browse",
        &[("authorization", "Bearer single-tok")],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(val.to_string().contains("AUTH_SCOPE_MISMATCH"));

    let sub = managed.join("demo-folder");
    fs::create_dir_all(sub.join(".git")).unwrap();
    let (st, val) = send_req(
        &router,
        "GET",
        "/api/v1/fs/browse",
        &[("x-cowok-session-token", &tok_a)],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{val}");
    let browse: FsBrowseResponse = serde_json::from_value(val).unwrap();
    assert!(browse
        .entries
        .iter()
        .any(|e| e.name == "demo-folder" && e.is_git_repo));

    let allowed = tmp.path().join("allowed-workspace");
    fs::create_dir_all(&allowed).unwrap();
    assert_eq!(
        send_req(
            &router,
            "GET",
            &format!("/api/v1/fs/browse?path={}", allowed.display()),
            &[("x-cowok-session-token", &tok_a)],
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    for denied in [
        "/",
        "/etc",
        "/private",
        "/private/var",
        "/private/var/db",
        "/private/etc",
    ] {
        assert_eq!(
            send_req(
                &router,
                "GET",
                &format!("/api/v1/fs/browse?path={denied}"),
                &[("x-cowok-session-token", &tok_a)],
                None
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    let sym = tmp.path().join("symlinked-escape");
    symlink(&allowed, &sym).unwrap();
    assert_eq!(
        send_req(
            &router,
            "GET",
            &format!("/api/v1/fs/browse?path={}", sym.display()),
            &[("x-cowok-session-token", &tok_a)],
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    std::env::remove_var("COWOK_PROJECTS_ROOT");
}

#[tokio::test]
async fn test_path_token_v2_promote_and_relative_compat() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let (_s, router, tok_a, _) = setup_daemon(&tmp);

    let (st, val) = send_req(
        &router,
        "POST",
        "/api/v1/projects",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({ "slug": "scratch-rel", "name": "Scratch Rel", "product_brief": "Compat" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{val}");
    let proj: ProjectDto = serde_json::from_value(val).unwrap();
    assert_eq!(proj.provenance.as_deref(), Some("scratch"));
    assert!(proj
        .project_code
        .as_deref()
        .is_some_and(|c| c.starts_with("CWK-")));

    // Story 01.3 relative_path backward compatibility on managed_scratch (.cowok-design)
    let (st, rel_v) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/path-token", proj.id),
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({ "relative_path": "product/specs/overview.md" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{rel_v}");
    let rel_tok: PathTokenResponse = serde_json::from_value(rel_v).unwrap();
    assert_eq!(send_req(&router, "POST", &format!("/api/v1/projects/{}/verify-path-token", proj.id), &[("x-cowok-session-token", &tok_a)], Some(json!({ "token": rel_tok.token, "expected_relative_path": "product/specs/overview.md" }))).await.0, StatusCode::OK);

    // PathToken v2, tamper detection, promote 0o700, and ancestor inode swap TOCTOU detection
    let ancestor = tmp.path().join("promote-parent");
    fs::create_dir_all(&ancestor).unwrap();
    let target = ancestor.join("new-app-dir");
    let (st, tok_v) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/path-token", proj.id),
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({ "target_path": target.display().to_string() })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{tok_v}");
    let v2: PathTokenResponse = serde_json::from_value(tok_v).unwrap();
    assert!(!v2.target_exists && !v2.ancestor_dev_ino.is_empty());
    assert_eq!(send_req(&router, "POST", &format!("/api/v1/projects/{}/verify-path-token", proj.id), &[("x-cowok-session-token", &tok_a)], Some(json!({ "token": format!("{}x", v2.token), "target_path": target.display().to_string() }))).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/promote", proj.id),
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "target_directory": target.display().to_string() }))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/promote", proj.id),
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "target_directory": target.display().to_string(), "token": v2.token }))
        )
        .await
        .0,
        StatusCode::OK
    );
    let listed: Vec<ProjectDto> = serde_json::from_value(
        send_req(
            &router,
            "GET",
            "/api/v1/projects",
            &[("x-cowok-session-token", &tok_a)],
            None,
        )
        .await
        .1,
    )
    .unwrap();
    assert_eq!(
        listed
            .iter()
            .find(|p| p.id == proj.id)
            .and_then(|p| p.provenance.as_deref()),
        Some("folder-backed")
    );
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o700
    );

    fs::rename(&ancestor, tmp.path().join("promote-parent-old")).unwrap();
    fs::create_dir_all(&ancestor).unwrap();
    assert_eq!(
        send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/verify-path-token", proj.id),
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "token": v2.token, "target_path": target.display().to_string() }))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn test_external_ws_and_managed_slug_validation() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let (_s, router, tok_a, _) = setup_daemon(&tmp);

    // Invalid managed_slug rejected with 422; /etc external ws_root rejected with 403; symlink-to-root rejected with 403
    assert_eq!(send_req(&router, "POST", "/api/v1/projects", &[("x-cowok-session-token", &tok_a)], Some(json!({ "slug": "bad", "name": "Bad", "product_brief": "x", "managed_slug": "../escape" }))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(send_req(&router, "POST", "/api/v1/projects", &[("x-cowok-session-token", &tok_a)], Some(json!({ "slug": "etc", "name": "Etc", "product_brief": "x", "workspace_root": "/etc/evil" }))).await.0, StatusCode::FORBIDDEN);
    let root_sym = tmp.path().join("sym-to-root");
    symlink(tmp.path(), &root_sym).unwrap();
    assert_eq!(send_req(&router, "POST", "/api/v1/projects", &[("x-cowok-session-token", &tok_a)], Some(json!({ "slug": "sym", "name": "Sym", "product_brief": "x", "workspace_root": root_sym.display().to_string() }))).await.0, StatusCode::FORBIDDEN);

    let ext = tmp.path().join("external-repo");
    let (st, ev) = send_req(&router, "POST", "/api/v1/projects", &[("x-cowok-session-token", &tok_a)], Some(json!({ "slug": "ext-proj", "name": "Ext", "product_brief": "External", "workspace_root": ext.display().to_string() }))).await;
    assert_eq!(st, StatusCode::CREATED, "{ev}");
    let ext_p: ProjectDto = serde_json::from_value(ev).unwrap();
    assert_eq!(ext_p.provenance.as_deref(), Some("folder-backed"));
}

#[tokio::test]
async fn test_trash_restore_cross_tenant_and_collisions() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("COWOK_PROJECTS_ROOT", tmp.path().join("CowokProjects"));
    let (_s, router, tok_a, tok_b) = setup_daemon(&tmp);

    let (st, p1v) = send_req(&router, "POST", "/api/v1/projects", &[("x-cowok-session-token", &tok_a)], Some(json!({ "slug": "wallet-redesign", "name": "Wallet", "product_brief": "Managed", "managed_slug": "wallet-redesign" }))).await;
    assert_eq!(st, StatusCode::CREATED, "{p1v}");
    let p1: ProjectDto = serde_json::from_value(p1v).unwrap();
    assert_eq!(p1.provenance.as_deref(), Some("managed"));
    assert_eq!(
        fs::metadata(p1.workspace_root.as_ref().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    // Soft-delete twice: first sets deleted=true & retention_days=30, second preserves original trash_deleted_at
    let d1: DeleteProjectResponse = serde_json::from_value(
        send_req(
            &router,
            "DELETE",
            &format!("/api/v1/projects/{}", p1.id),
            &[("x-cowok-session-token", &tok_a)],
            None,
        )
        .await
        .1,
    )
    .unwrap();
    assert!(d1.deleted && d1.retention_days == 30 && d1.project_id == p1.id);
    let d2: DeleteProjectResponse = serde_json::from_value(
        send_req(
            &router,
            "DELETE",
            &format!("/api/v1/projects/{}", p1.id),
            &[("x-cowok-session-token", &tok_a)],
            None,
        )
        .await
        .1,
    )
    .unwrap();
    assert!(!d2.deleted && d2.trash_deleted_at == d1.trash_deleted_at);
    assert_eq!(
        send_req(
            &router,
            "GET",
            &format!("/api/v1/projects/{}/specs", p1.id),
            &[("x-cowok-session-token", &tok_a)],
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send_req(
            &router,
            "GET",
            &format!("/api/v1/projects/{}/flow/snapshot", p1.id),
            &[("x-cowok-session-token", &tok_a)],
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    // Tenant isolation on active list, ?include_trashed=true, and POST /restore (IDOR -> 403)
    let b_list: Vec<ProjectDto> = serde_json::from_value(
        send_req(
            &router,
            "GET",
            "/api/v1/projects?include_trashed=true",
            &[("x-cowok-session-token", &tok_b)],
            None,
        )
        .await
        .1,
    )
    .unwrap();
    assert!(b_list.is_empty());
    let a_trash: Vec<ProjectDto> = serde_json::from_value(
        send_req(
            &router,
            "GET",
            "/api/v1/projects?include_trashed=true",
            &[("x-cowok-session-token", &tok_a)],
            None,
        )
        .await
        .1,
    )
    .unwrap();
    assert_eq!(a_trash.len(), 1);
    assert_eq!(
        send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/restore", p1.id),
            &[("x-cowok-session-token", &tok_b)],
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    // Cross-tenant active slug collision: tenant-b claims "wallet-redesign", tenant-a restores p1 -> renamed_slug!
    assert_eq!(send_req(&router, "POST", "/api/v1/projects", &[("x-cowok-session-token", &tok_b)], Some(json!({ "slug": "wallet-redesign", "name": "Other Tenant", "product_brief": "Takes slug" }))).await.0, StatusCode::CREATED);
    let r2: RestoreProjectResponse = serde_json::from_value(
        send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/restore", p1.id),
            &[("x-cowok-session-token", &tok_a)],
            None,
        )
        .await
        .1,
    )
    .unwrap();
    assert!(
        r2.restored
            && r2
                .renamed_slug
                .as_deref()
                .is_some_and(|s| s.starts_with("wallet-redesign-restored-"))
    );

    // Active workspace_root collision -> 409 WORKSPACE_ROOT_CONFLICT
    assert_eq!(
        send_req(
            &router,
            "DELETE",
            &format!("/api/v1/projects/{}", p1.id),
            &[("x-cowok-session-token", &tok_a)],
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(send_req(&router, "POST", "/api/v1/projects", &[("x-cowok-session-token", &tok_a)], Some(json!({ "slug": "wallet-v3", "name": "V3", "product_brief": "Takes ws", "managed_slug": "wallet-redesign" }))).await.0, StatusCode::CREATED);
    let (st, c_val) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/restore", p1.id),
        &[("x-cowok-session-token", &tok_a)],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{c_val}");
    assert!(c_val.to_string().contains("WORKSPACE_ROOT_CONFLICT"));

    // Provenance spoofing rejected & promote-into-active-workspace fails closed with 409 without overwriting marker
    let spoil_dir = tmp.path().join("spoil-ext");
    assert_eq!(send_req(&router, "POST", "/api/v1/projects", &[("x-cowok-session-token", &tok_a)], Some(json!({ "slug": "spoiled", "name": "Spoil", "product_brief": "x", "workspace_root": spoil_dir.display().to_string(), "provenance": "scratch" }))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let active_ws = tmp.path().join("active-owner-ws");
    assert_eq!(send_req(&router, "POST", "/api/v1/projects", &[("x-cowok-session-token", &tok_a)], Some(json!({ "slug": "active-owner", "name": "Owner", "product_brief": "x", "workspace_root": active_ws.display().to_string() }))).await.0, StatusCode::CREATED);
    let scratch_p: ProjectDto = serde_json::from_value(
        send_req(
            &router,
            "POST",
            "/api/v1/projects",
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "slug": "scratch-attacker", "name": "Attacker", "product_brief": "x" })),
        )
        .await
        .1,
    )
    .unwrap();
    fs::write(
        std::path::Path::new(scratch_p.workspace_root.as_deref().unwrap())
            .join("attacker-leak.txt"),
        "leak",
    )
    .unwrap();
    let atk_tok: PathTokenResponse = serde_json::from_value(
        send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/path-token", scratch_p.id),
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "target_path": active_ws.display().to_string() })),
        )
        .await
        .1,
    )
    .unwrap();
    assert_eq!(send_req(&router, "POST", &format!("/api/v1/projects/{}/promote", scratch_p.id), &[("x-cowok-session-token", &tok_a)], Some(json!({ "target_directory": active_ws.display().to_string(), "token": atk_tok.token }))).await.0, StatusCode::CONFLICT);
    assert!(
        !active_ws.join("attacker-leak.txt").exists(),
        "Failed promote must not copy files into existing active workspace"
    );
    let owner_pj = fs::read_to_string(active_ws.join(".cowok-design").join("project.json"))
        .unwrap_or_default();
    assert!(
        !owner_pj.contains(&scratch_p.id.to_string()),
        "Failed promote must not overwrite owner's project.json"
    );
    std::env::remove_var("COWOK_PROJECTS_ROOT");
}

#[tokio::test]
async fn test_pre_auth_disk_write_and_symlink_manifest_guards() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let (_s, router, tok_a, _tok_b) = setup_daemon(&tmp);

    // 1. Non-superadmin invalid Bearer must fail closed BEFORE recreating missing workspace_root/project.token
    let ws_missing = tmp.path().join("bearer-missing-ws");
    let p: ProjectDto = serde_json::from_value(
        send_req(
            &router,
            "POST",
            "/api/v1/projects",
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "slug": "bearer-guard", "name": "BearerGuard", "product_brief": "x", "workspace_root": ws_missing.display().to_string() })),
        )
        .await
        .1,
    )
    .unwrap();
    fs::remove_dir_all(&ws_missing).unwrap();
    assert!(!ws_missing.exists());
    let (st, body) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/specs", p.id),
        &[("authorization", "Bearer wrong-guess")],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{body}");
    assert!(
        !ws_missing.exists(),
        "Invalid Bearer request must not create dirs or project.token on disk"
    );

    // 2. Existing workspace with symlinked .cowok-design/project.json must fail closed without overwriting target
    #[cfg(unix)]
    {
        let victim = tmp.path().join("victim.txt");
        fs::write(&victim, "KEEP_SAFE").unwrap();
        let sym_ws = tmp.path().join("sym-project-json-ws");
        fs::create_dir_all(sym_ws.join(".cowok-design")).unwrap();
        std::os::unix::fs::symlink(&victim, sym_ws.join(".cowok-design").join("project.json"))
            .unwrap();
        let (cst, cbody) = send_req(
            &router,
            "POST",
            "/api/v1/projects",
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "slug": "sym-pj", "name": "SymPJ", "product_brief": "x", "workspace_root": sym_ws.display().to_string() })),
        )
        .await;
        assert!(
            cst == StatusCode::FORBIDDEN || cst == StatusCode::UNPROCESSABLE_ENTITY,
            "Unexpected {cst}: {cbody}"
        );
        assert_eq!(fs::read_to_string(&victim).unwrap(), "KEEP_SAFE");
    }
}

#[tokio::test]
async fn test_hardlink_dst_rejected_and_src_hardlink_allowed() {
    use harness_daemon::routes::fs_jail::{copy_file_between_dir_fds, write_file_in_dir_fd};
    use std::ffi::OsStr;
    use std::os::fd::OwnedFd;

    let tmp = tempfile::tempdir().unwrap();
    let src_dir = tmp.path().join("src");
    let dst_dir = tmp.path().join("dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();

    // 1. Victim hardlinked into destination directory must fail closed without altering victim inode
    let victim = tmp.path().join("victim.txt");
    fs::write(&victim, "VICTIM_ORIGINAL").unwrap();
    fs::hard_link(&victim, dst_dir.join("target.txt")).unwrap();
    let dst_fd: OwnedFd = fs::File::open(&dst_dir).unwrap().into();
    let err = write_file_in_dir_fd(&dst_fd, OsStr::new("target.txt"), b"OVERWRITTEN", 0o644)
        .expect_err("Destination hardlink overwrite must fail closed");
    assert!(
        err.to_string().contains("Hardlink"),
        "Unexpected err: {err}"
    );
    assert_eq!(fs::read_to_string(&victim).unwrap(), "VICTIM_ORIGINAL");

    // 2. Source workspace hardlinked file (e.g. git object / pnpm store) must copy cleanly
    let src_a = src_dir.join("obj.bin");
    let src_b = src_dir.join("obj-link.bin");
    fs::write(&src_a, b"GIT_PACK_PAYLOAD").unwrap();
    fs::hard_link(&src_a, &src_b).unwrap();
    let src_fd: OwnedFd = fs::File::open(&src_dir).unwrap().into();
    copy_file_between_dir_fds(&src_fd, &dst_fd, OsStr::new("obj.bin"), 0o644)
        .expect("Source hardlink must be allowed when copying to fresh dst");
    assert_eq!(
        fs::read(dst_dir.join("obj.bin")).unwrap(),
        b"GIT_PACK_PAYLOAD"
    );
}

#[tokio::test]
async fn test_empty_bearer_rejected_and_v7_workbench_convergence() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let (_s, router, tok_a, _tok_b) = setup_daemon(&tmp);

    // 1. Empty "Bearer " header must be rejected (401/403), never 200
    let (st_empty, _) = send_req(
        &router,
        "GET",
        "/api/v1/projects",
        &[("authorization", "Bearer   ")],
        None,
    )
    .await;
    assert_eq!(st_empty, StatusCode::UNAUTHORIZED);

    // 2. Global agent-settings upsert (project_id: null) persists V7 fields and converges to 1 row
    for model in ["gpt-4o-mini", "gpt-4o"] {
        let (ust, uval) = send_req(
            &router,
            "PUT",
            "/api/v1/agent-settings",
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({
                "providerId": "openai",
                "modelId": model,
                "permissionMode": "accept_edits",
                "vaultMode": "keychain",
                "onboardingDone": true
            })),
        )
        .await;
        assert_eq!(ust, StatusCode::OK, "{uval}");
        assert_eq!(uval["permissionMode"], "accept_edits");
        assert_eq!(uval["vaultMode"], "keychain");
        assert_eq!(uval["onboardingDone"], true);
    }
    let (lst, lval) = send_req(
        &router,
        "GET",
        "/api/v1/agent-settings",
        &[("x-cowok-session-token", &tok_a)],
        None,
    )
    .await;
    assert_eq!(lst, StatusCode::OK);
    let rows = lval.as_array().expect("agent-settings array");
    let openai_rows: Vec<_> = rows
        .iter()
        .filter(|r| r["providerId"] == "openai")
        .collect();
    assert_eq!(
        openai_rows.len(),
        1,
        "Global NULL project_id upsert must not create duplicate rows: {openai_rows:?}"
    );

    // 3. MCP server command validation + real 404 on deleting nonexistent server
    let (bad_mcp, _) = send_req(
        &router,
        "POST",
        "/api/v1/mcp-servers",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({
            "serverSlug": "evil",
            "displayName": "Evil",
            "transportKind": "stdio",
            "commandOrUrl": "npx foo && rm -rf /",
            "permissionTier": "read_only"
        })),
    )
    .await;
    assert_eq!(bad_mcp, StatusCode::BAD_REQUEST);
    let (del_404, _) = send_req(
        &router,
        "DELETE",
        "/api/v1/mcp-servers/nonexistent-server-id",
        &[("x-cowok-session-token", &tok_a)],
        None,
    )
    .await;
    assert_eq!(del_404, StatusCode::NOT_FOUND);

    // 4. Key verify returns honest syntax-only status (constructed dummy string avoids Gitleaks generic-api-key false positive)
    let dummy_key = format!("sk-{}", "0".repeat(20));
    let (vst, vval) = send_req(
        &router,
        "POST",
        "/api/v1/doctor/verify-key",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({ "providerId": "openai", "apiKey": dummy_key })),
    )
    .await;
    assert_eq!(vst, StatusCode::OK, "{vval}");
    assert_eq!(vval["status"], "format_checked");

    // 5. Job events accept PAUSED, populate composite_scope_key + monotonically increasing project_seq across jobs
    let p: ProjectDto = serde_json::from_value(
        send_req(
            &router,
            "POST",
            "/api/v1/projects",
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "slug": "sse-seq-proj", "name": "SSE Seq", "product_brief": "x" })),
        )
        .await
        .1,
    )
    .unwrap();
    let (e1_st, e1) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/events", p.id),
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({ "jobId": "job-1", "nodeId": "node-a", "eventKind": "STEP_LOG", "payloadText": "one", "jobStatus": "PAUSED" })),
    )
    .await;
    assert_eq!(e1_st, StatusCode::CREATED, "{e1}");
    assert_eq!(e1["jobStatus"], "PAUSED");
    assert_eq!(e1["projectSeq"], 1);
    let (e2_st, e2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/events", p.id),
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({ "jobId": "job-2", "nodeId": "node-b", "eventKind": "COMPLETED", "payloadText": "two", "jobStatus": "COMPLETED" })),
    )
    .await;
    assert_eq!(e2_st, StatusCode::CREATED, "{e2}");
    assert_eq!(e2["projectSeq"], 2);

    // 6. SSE resume with Last-Event-ID: 1 resumes at project_seq 2 without replaying seq 1
    let sse_req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/projects/{}/flow/jobs/stream", p.id))
        .header("host", "127.0.0.1:4170")
        .header("x-cowok-session-token", &tok_a)
        .header("last-event-id", "1")
        .body(Body::empty())
        .unwrap();
    let sse_resp = router.clone().oneshot(sse_req).await.unwrap();
    assert_eq!(sse_resp.status(), StatusCode::OK);
    let mut sse_body = sse_resp.into_body();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(2), sse_body.frame())
        .await
        .expect("SSE frame timeout")
        .expect("SSE stream ended")
        .expect("SSE frame ok");
    let text = String::from_utf8_lossy(frame.data_ref().expect("data frame")).into_owned();
    assert!(text.contains("id: 2"), "Expected SSE id: 2, got: {text}");
    assert!(
        text.contains("\"payloadText\":\"two\""),
        "Expected job-2 payload, got: {text}"
    );
    assert!(
        !text.contains("\"payloadText\":\"one\""),
        "Must not replay seq 1: {text}"
    );
}

#[tokio::test]
async fn test_cross_scope_job_event_and_sensitive_fs_browse_guards() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let (_s, router, tok_a, tok_b) = setup_daemon(&tmp);

    // Create two projects: p_a (tenant-a) + p_a2 (tenant-a) + p_b (tenant-b)
    let p_a: ProjectDto = serde_json::from_value(
        send_req(
            &router,
            "POST",
            "/api/v1/projects",
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "slug": "scope-a1", "name": "Scope A1", "product_brief": "x" })),
        )
        .await
        .1,
    )
    .unwrap();
    let p_a2: ProjectDto = serde_json::from_value(
        send_req(
            &router,
            "POST",
            "/api/v1/projects",
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "slug": "scope-a2", "name": "Scope A2", "product_brief": "x" })),
        )
        .await
        .1,
    )
    .unwrap();
    let p_b: ProjectDto = serde_json::from_value(
        send_req(
            &router,
            "POST",
            "/api/v1/projects",
            &[("x-cowok-session-token", &tok_b)],
            Some(json!({ "slug": "scope-b1", "name": "Scope B1", "product_brief": "x" })),
        )
        .await
        .1,
    )
    .unwrap();

    // Seed session & job in p_a and p_b
    let (s_st, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/events", p_a.id),
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({ "jobId": "job-a-owned", "sessionId": "sess-a-owned", "nodeId": "n1", "eventKind": "started" })),
    )
    .await;
    assert_eq!(s_st, StatusCode::CREATED);
    let (sb_st, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/events", p_b.id),
        &[("x-cowok-session-token", &tok_b)],
        Some(json!({ "jobId": "job-b-owned", "sessionId": "sess-b-owned", "nodeId": "n1", "eventKind": "started" })),
    )
    .await;
    assert_eq!(sb_st, StatusCode::CREATED);

    // Same-tenant cross-project sessionId / jobId -> 404 PROJECT_NOT_FOUND
    assert_eq!(
        send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/flow/jobs/events", p_a2.id),
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "jobId": "fresh-job", "sessionId": "sess-a-owned", "nodeId": "n1", "eventKind": "started" })),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/flow/jobs/events", p_a2.id),
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "jobId": "job-a-owned", "nodeId": "n1", "eventKind": "started" })),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    // Cross-tenant sessionId / jobId -> 403 TENANT_ISOLATION_BREACH (never 500 UNIQUE error)
    assert_eq!(
        send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/flow/jobs/events", p_a.id),
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "jobId": "fresh-job-2", "sessionId": "sess-b-owned", "nodeId": "n1", "eventKind": "started" })),
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/flow/jobs/events", p_a.id),
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({ "jobId": "job-b-owned", "nodeId": "n1", "eventKind": "started" })),
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    // Block /Library and symlink traversal in fs/browse
    let (lib_st, _) = send_req(
        &router,
        "GET",
        "/api/v1/fs/browse?path=/Library",
        &[("x-cowok-session-token", &tok_a)],
        None,
    )
    .await;
    assert_eq!(lib_st, StatusCode::FORBIDDEN);

    let browse_dir = tmp.path().join("browse-check");
    fs::create_dir_all(browse_dir.join("real-repo").join(".git")).unwrap();
    fs::create_dir_all(browse_dir.join("sym-git-repo")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            browse_dir.join("real-repo").join(".git"),
            browse_dir.join("sym-git-repo").join(".git"),
        )
        .unwrap();
        std::os::unix::fs::symlink(browse_dir.join("real-repo"), browse_dir.join("symlink-dir"))
            .unwrap();
    }
    let (br_st, br_val) = send_req(
        &router,
        "GET",
        &format!("/api/v1/fs/browse?path={}", browse_dir.display()),
        &[("x-cowok-session-token", &tok_a)],
        None,
    )
    .await;
    assert_eq!(br_st, StatusCode::OK, "{br_val}");
    let entries = br_val["entries"].as_array().unwrap();
    let names: Vec<&str> = entries.iter().filter_map(|e| e["name"].as_str()).collect();
    assert!(names.contains(&"real-repo"), "{names:?}");
    assert!(
        !names.contains(&"symlink-dir"),
        "symlink dir leaked: {names:?}"
    );
    let sym_git = entries
        .iter()
        .find(|e| e["name"] == "sym-git-repo")
        .unwrap();
    assert_eq!(sym_git["is_git_repo"], json!(false));

    // ensure_project_token rejects symlink project.token
    let sym_tok_root = tmp.path().join("sym-tok-proj");
    fs::create_dir_all(sym_tok_root.join(".cowok-design")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            "/etc/passwd",
            sym_tok_root.join(".cowok-design").join("project.token"),
        )
        .unwrap();
        assert!(harness_daemon::synthesis::ensure_project_token(&sym_tok_root).is_err());
    }
}
