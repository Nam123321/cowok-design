// @story 12.3
// @implements AC-01, AC-02, AC-03 (Contract Tests ut_12_3_01..ut_12_3_08 + Stage 3 Remediation Regressions)
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use harness_daemon::ci_gate::{
    detect_libreoffice_in_dirs, exec_ci_gate, exec_ci_gate_with_overrides,
    execute_ci_gate_cli_with_overrides, inspect_uid_gid_diagnostics, probe_shm_diagnostics,
    probe_shm_diagnostics_with_container, resolve_memory_cap, resolve_requested_max_memory_mb,
    run_ci_gate_with_overrides, CiGateRequest, CiGateVerdict, CiLiveCaptureOutput,
    CiRuntimeProbeOverrides, EXPECTED_CONTAINER_UID, SHM_SAFE_THRESHOLD_BYTES,
};
use harness_daemon::lockfile::{generate_csprng_hex_32, DaemonLockInfo, CURRENT_SCHEMA_VERSION};
use harness_daemon::port_binder::ChildProcessReaper;
use harness_daemon::visual_diff::FOCUS_FREE_HEADLESS_FLAGS;
use html_forge::visual_qc::{DomGeometryNode, QcBoundingBox};
use spec_registry::{SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use uuid::Uuid;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace repo root")
        .to_path_buf()
}

fn clean_isolated_overrides(tmp: &TempDir) -> CiRuntimeProbeOverrides {
    let clean_bin_dir = tmp.path().join("clean-bin");
    fs::create_dir_all(&clean_bin_dir).unwrap();
    CiRuntimeProbeOverrides {
        libreoffice_search_dirs: Some(vec![clean_bin_dir]),
        env_max_memory_mb: Some(None),
        env_chromium_no_sandbox: Some(false),
        ..Default::default()
    }
}

// -----------------------------------------------------------------------------
// ut_12_3_01: Multi-Arch OCI Contract, Non-Root UID 10001 & Zero LibreOffice
// -----------------------------------------------------------------------------
#[test]
fn ut_12_3_01_oci_contract_multiarch_nonroot_zero_libreoffice() {
    let root = repo_root();
    let contract_path = root.join("packaging/docker/oci-image-contract.json");
    let dockerfile_path = root.join("packaging/docker/Dockerfile");
    let verifier_path = root.join("packaging/docker/verify-oci-contract.mjs");

    assert!(
        contract_path.is_file(),
        "missing {}",
        contract_path.display()
    );
    assert!(
        dockerfile_path.is_file(),
        "missing {}",
        dockerfile_path.display()
    );
    assert!(
        verifier_path.is_file(),
        "missing {}",
        verifier_path.display()
    );

    let contract: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&contract_path).unwrap()).unwrap();
    assert_eq!(contract["image"], "ghcr.io/cowok-ai/uiux-os");
    assert_eq!(contract["user"]["uid"], EXPECTED_CONTAINER_UID);
    assert_eq!(contract["user"]["gid"], EXPECTED_CONTAINER_UID);
    assert_eq!(contract["user"]["nonRootRequired"], true);
    assert_eq!(
        contract["pooledChromiumCdp"]["mode"],
        "pooled-1-chromium-cdp"
    );
    assert_eq!(contract["pooledChromiumCdp"]["maxConcurrency"], 1);
    assert_eq!(
        contract["pooledChromiumCdp"]["containerNoSandboxFlag"],
        "--no-sandbox"
    );
    assert_eq!(contract["invariants"]["zeroLibreoffice"], true);
    assert_eq!(contract["invariants"]["zeroCircularDepOnStory13_1"], true);
    assert_eq!(contract["invariants"]["tiniSubreaperFlag"], "-s");
    assert_eq!(contract["endpoints"]["health"], "/api/v1/health");
    assert_eq!(contract["endpoints"]["mcpRpc"], "/api/v1/mcp/rpc");
    assert_eq!(contract["endpoints"]["qcRun"], "/api/v1/qc/run");

    let platforms = contract["platforms"].as_array().expect("platforms array");
    assert!(platforms.iter().any(|p| p == "linux/amd64"));
    assert!(platforms.iter().any(|p| p == "linux/arm64"));

    let dockerfile = fs::read_to_string(&dockerfile_path).unwrap();
    assert!(dockerfile.contains("FROM --platform=$TARGETPLATFORM rust:1-slim-bookworm"));
    assert!(dockerfile.contains("COPY packages/builtin-skills ./packages/builtin-skills"));
    assert!(dockerfile.contains("COPY packaging/plugins ./packaging/plugins"));
    assert!(dockerfile.contains("--locked"));
    assert!(dockerfile.contains("COWOK_CONTAINER_MODE=1"));
    assert!(dockerfile.contains("COWOK_CHROMIUM_NO_SANDBOX=1"));
    assert!(dockerfile.contains("USER 10001:10001"));
    assert!(dockerfile.contains("HEALTHCHECK"));
    assert!(dockerfile.contains("/api/v1/health"));
    assert!(dockerfile.contains("chromium"));
    assert!(dockerfile.contains(
        "ENTRYPOINT [\"/usr/bin/tini\", \"-s\", \"--\", \"/usr/local/bin/cowok-entrypoint.sh\"]"
    ));
    assert!(!dockerfile.to_ascii_lowercase().contains("libreoffice"));
    assert!(!dockerfile.to_ascii_lowercase().contains("soffice"));
    assert!(!dockerfile
        .to_ascii_lowercase()
        .contains("eval_benchmark_runs"));

    let verify_out = Command::new("node")
        .arg(&verifier_path)
        .output()
        .expect("run node verify-oci-contract.mjs");
    assert!(
        verify_out.status.success(),
        "verify-oci-contract.mjs failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&verify_out.stdout),
        String::from_utf8_lossy(&verify_out.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&verify_out.stdout).expect("valid verifier JSON");
    assert_eq!(report["ok"], true);
    assert_eq!(report["violations"].as_array().map(Vec::len), Some(0));
}

// -----------------------------------------------------------------------------
// ut_12_3_02: `ci gate --json` Story 07.1 Visual QC & Story 12.1 7 DE Tools Parity
// -----------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ut_12_3_02_ci_gate_json_qc_de_parity() {
    let tmp = TempDir::new().unwrap();
    let html_path = tmp.path().join("clean-screen.html");
    fs::write(
        &html_path,
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"><title>Clean CI Screen</title></head><body><main><h1>Dashboard Overview</h1><p>Deterministic semantic layout with zero placeholders.</p></main></body></html>",
    )
    .unwrap();

    let project_id = Uuid::new_v4().to_string();
    let req = CiGateRequest {
        project_id: project_id.clone(),
        target_id: "dashboard-screen".to_string(),
        html_path: Some(html_path),
        max_memory_mb: Some(512),
        force_no_browser: true,
        fail_on_unverified: false,
        workspace: Some(tmp.path().to_path_buf()),
    };

    let overrides = clean_isolated_overrides(&tmp);

    let out = exec_ci_gate_with_overrides(req, true, &overrides).await;
    assert_eq!(
        out.exit_code, 0,
        "stderr={}, stdout={}",
        out.stderr, out.stdout
    );
    let parsed: serde_json::Value = serde_json::from_str(&out.stdout).expect("valid JSON envelope");
    assert_eq!(parsed["ok"], true);
    assert!(parsed["requestId"].as_str().is_some());

    let data = &parsed["data"];
    assert_eq!(data["projectId"], project_id);
    assert_eq!(data["verdict"], "UNVERIFIED_NO_BROWSER");
    assert_eq!(data["passed"], true);
    assert_eq!(data["exitCode"], 0);
    assert_eq!(data["visualQc"]["verdict"], "UNVERIFIED_NO_BROWSER");
    assert_eq!(data["visualQc"]["p0Count"], 0);
    assert_eq!(data["deSuite"]["passed"], true);
    assert_eq!(data["deSuite"]["results"].as_array().map(Vec::len), Some(7));
    assert_eq!(data["diagnostics"]["zeroLibreofficeInvariant"], true);
    assert_eq!(data["diagnostics"]["pooledCdpMaxConcurrency"], 1);
    assert_eq!(data["diagnostics"]["liveCaptureExecuted"], false);

    // Confirm SQLite WAL actually persisted the `qc_gate_runs` row in workspace storage
    let db_path = PathBuf::from(
        data["diagnostics"]["walStaging"]["dbPath"]
            .as_str()
            .unwrap(),
    );
    let state_root = PathBuf::from(
        data["diagnostics"]["walStaging"]["stateRoot"]
            .as_str()
            .unwrap(),
    );
    assert!(
        db_path.is_file(),
        "SQLite WAL DB must exist at {}",
        db_path.display()
    );
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, &state_root)).unwrap();
    let conn = pool.connection().unwrap();
    let persisted_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM qc_gate_runs WHERE project_id = ?1",
            rusqlite::params![project_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(persisted_rows, 1);

    // Verify sync CLI wrapper deterministically without host `/usr/bin/libreoffice` flakiness (P2-5)
    let sync_out = execute_ci_gate_cli_with_overrides(
        CiGateRequest {
            project_id: "sync-cli-smoke".to_string(),
            force_no_browser: true,
            workspace: Some(tmp.path().to_path_buf()),
            ..Default::default()
        },
        true,
        &overrides,
    );
    assert_eq!(sync_out.exit_code, 0, "sync stdout={}", sync_out.stdout);
}

// -----------------------------------------------------------------------------
// ut_12_3_03: Deterministic Exit Codes (3 for FAIL, 2 for Invalid/Symlink/FIFO)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn ut_12_3_03_ci_gate_deterministic_exit_codes_and_fail_on_unverified() {
    let tmp = TempDir::new().unwrap();
    let overrides = clean_isolated_overrides(&tmp);

    // (a) `--force-no-browser --fail-on-unverified` -> FAIL (exit code 3)
    let strict_out = exec_ci_gate_with_overrides(
        CiGateRequest {
            project_id: "strict-ci".to_string(),
            force_no_browser: true,
            fail_on_unverified: true,
            workspace: Some(tmp.path().to_path_buf()),
            ..Default::default()
        },
        true,
        &overrides,
    )
    .await;
    assert_eq!(strict_out.exit_code, 3);
    let strict_json: serde_json::Value = serde_json::from_str(&strict_out.stdout).unwrap();
    assert_eq!(strict_json["data"]["verdict"], "FAIL");
    assert_eq!(strict_json["data"]["passed"], false);

    // (b) HTML triggering DE accessibility & slop errors -> FAIL (exit code 3)
    let bad_html = tmp.path().join("bad.html");
    fs::write(
        &bad_html,
        "<!DOCTYPE html><html><body><main><img src=\"hero.png\"><button></button><p>Delve into lorem ipsum placeholder.</p></main></body></html>",
    )
    .unwrap();
    let bad_out = exec_ci_gate_with_overrides(
        CiGateRequest {
            project_id: "bad-html-ci".to_string(),
            html_path: Some(bad_html.clone()),
            force_no_browser: true,
            fail_on_unverified: false,
            workspace: Some(tmp.path().to_path_buf()),
            ..Default::default()
        },
        true,
        &overrides,
    )
    .await;
    assert_eq!(bad_out.exit_code, 3);
    let bad_json: serde_json::Value = serde_json::from_str(&bad_out.stdout).unwrap();
    assert_eq!(bad_json["data"]["verdict"], "FAIL");
    assert_eq!(bad_json["data"]["deSuite"]["passed"], false);

    // (c) Invalid `--max-memory-mb 32` -> validation error (exit code 2)
    let bad_mem = exec_ci_gate(
        CiGateRequest {
            project_id: "bad-mem".to_string(),
            max_memory_mb: Some(32),
            force_no_browser: true,
            workspace: Some(tmp.path().to_path_buf()),
            ..Default::default()
        },
        true,
    )
    .await;
    assert_eq!(bad_mem.exit_code, 2);
    let bad_mem_json: serde_json::Value = serde_json::from_str(&bad_mem.stdout).unwrap();
    assert_eq!(bad_mem_json["error"]["code"], "INVALID_MAX_MEMORY_MB");

    // (d) Symlink `--html` -> SYMLINK_REJECTED (exit code 2)
    #[cfg(unix)]
    {
        let sym_html = tmp.path().join("symlink.html");
        std::os::unix::fs::symlink(&bad_html, &sym_html).unwrap();
        let sym_out = exec_ci_gate(
            CiGateRequest {
                project_id: "symlink-ci".to_string(),
                html_path: Some(sym_html),
                force_no_browser: true,
                workspace: Some(tmp.path().to_path_buf()),
                ..Default::default()
            },
            true,
        )
        .await;
        assert_eq!(sym_out.exit_code, 2);
        let sym_json: serde_json::Value = serde_json::from_str(&sym_out.stdout).unwrap();
        assert_eq!(sym_json["error"]["code"], "SYMLINK_REJECTED");
    }

    // (e) P1-3 Regression: Named FIFO `--html` MUST NOT block (`O_NONBLOCK`) -> HTML_NOT_REGULAR_FILE (exit code 2)
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let fifo_path = tmp.path().join("blocking.fifo.html");
        let c_fifo = CString::new(fifo_path.as_os_str().as_bytes()).unwrap();
        let rc = unsafe { libc::mkfifo(c_fifo.as_ptr(), 0o600) };
        assert_eq!(rc, 0, "mkfifo must succeed");

        let fifo_out = exec_ci_gate_with_overrides(
            CiGateRequest {
                project_id: "fifo-ci".to_string(),
                html_path: Some(fifo_path),
                force_no_browser: true,
                workspace: Some(tmp.path().to_path_buf()),
                ..Default::default()
            },
            true,
            &overrides,
        )
        .await;
        assert_eq!(fifo_out.exit_code, 2);
        let fifo_json: serde_json::Value = serde_json::from_str(&fifo_out.stdout).unwrap();
        assert_eq!(fifo_json["error"]["code"], "HTML_NOT_REGULAR_FILE");
    }
}

// -----------------------------------------------------------------------------
// ut_12_3_04: `/dev/shm` Low-Capacity Automatic `--disable-dev-shm-usage` & Container `--no-sandbox`
// -----------------------------------------------------------------------------
#[test]
fn ut_12_3_04_dev_shm_low_capacity_auto_disable_flag() {
    let shm_path = PathBuf::from("/dev/shm");
    let low_64_mib = 64 * 1024 * 1024;
    let high_512_mib = 512 * 1024 * 1024;

    // 1. Default Docker 64 MiB `/dev/shm` (< 256 MiB) -> auto-inject `--disable-dev-shm-usage`
    let low = probe_shm_diagnostics(&shm_path, Some(low_64_mib), Some(low_64_mib), false);
    assert_eq!(low.threshold_bytes, SHM_SAFE_THRESHOLD_BYTES);
    assert!(low.auto_disable_dev_shm_usage);
    assert!(!low.container_no_sandbox);
    assert!(low
        .resolved_chromium_flags
        .iter()
        .any(|f| f == "--disable-dev-shm-usage"));
    assert!(!low
        .resolved_chromium_flags
        .iter()
        .any(|f| f == "--no-sandbox"));
    for base_flag in FOCUS_FREE_HEADLESS_FLAGS {
        assert!(low.resolved_chromium_flags.iter().any(|f| f == base_flag));
    }

    // 2. Unreadable / absent `/dev/shm` -> fails safe by enabling `--disable-dev-shm-usage`
    let missing = probe_shm_diagnostics(
        &PathBuf::from("/nonexistent/cowok-shm-probe"),
        None,
        None,
        false,
    );
    assert!(missing.auto_disable_dev_shm_usage);
    assert!(missing
        .resolved_chromium_flags
        .iter()
        .any(|f| f == "--disable-dev-shm-usage"));

    // 3. Healthy 512 MiB `/dev/shm` (>= 256 MiB) -> omits `--disable-dev-shm-usage` unless forced
    let high = probe_shm_diagnostics(&shm_path, Some(high_512_mib), Some(high_512_mib), false);
    assert!(!high.auto_disable_dev_shm_usage);
    assert!(!high
        .resolved_chromium_flags
        .iter()
        .any(|f| f == "--disable-dev-shm-usage"));

    let high_forced =
        probe_shm_diagnostics(&shm_path, Some(high_512_mib), Some(high_512_mib), true);
    assert!(high_forced.auto_disable_dev_shm_usage);
    assert!(high_forced
        .resolved_chromium_flags
        .iter()
        .any(|f| f == "--disable-dev-shm-usage"));

    // 4. P1-1 Regression: Hardened container mode appends `--no-sandbox`
    let container_flags = probe_shm_diagnostics_with_container(
        &shm_path,
        Some(low_64_mib),
        Some(low_64_mib),
        false,
        true,
    );
    assert!(container_flags.container_no_sandbox);
    assert!(container_flags
        .resolved_chromium_flags
        .iter()
        .any(|f| f == "--no-sandbox"));
    assert!(container_flags
        .resolved_chromium_flags
        .iter()
        .any(|f| f == "--disable-dev-shm-usage"));
}

// -----------------------------------------------------------------------------
// ut_12_3_05: Cgroup v2/v1 `--max-memory-mb`, Hybrid v1+v2 Fallback & `COWOK_MAX_MEMORY_MB` Env
// -----------------------------------------------------------------------------
#[tokio::test]
async fn ut_12_3_05_cgroup_max_memory_mb_cap_enforcement() {
    let tmp = TempDir::new().unwrap();
    let missing = tmp.path().join("missing");

    // 1. Cgroup v2 `memory.max` = 512 MiB -> 75% ceiling = 384 MiB -> clamps requested 512 MiB
    let cg_v2 = tmp.path().join("memory.max");
    fs::write(&cg_v2, format!("{}\n", 512u64 * 1024 * 1024)).unwrap();
    let v2_cap = resolve_memory_cap(Some(512), &cg_v2, &missing).unwrap();
    assert_eq!(v2_cap.cgroup_source, "cgroup_v2");
    assert_eq!(v2_cap.cgroup_limit_mb, Some(512));
    assert_eq!(v2_cap.effective_max_memory_mb, 384);
    assert!(v2_cap.cgroup_clamped);
    assert_eq!(v2_cap.js_heap_flag, "--js-flags=--max-old-space-size=384");

    // 2. Cgroup v1 `memory.limit_in_bytes` = 1024 MiB -> 75% ceiling = 768 MiB
    let cg_v1 = tmp.path().join("memory.limit_in_bytes");
    fs::write(&cg_v1, format!("{}\n", 1024u64 * 1024 * 1024)).unwrap();
    let v1_unclamped = resolve_memory_cap(Some(512), &missing, &cg_v1).unwrap();
    assert_eq!(v1_unclamped.cgroup_source, "cgroup_v1");
    assert_eq!(v1_unclamped.cgroup_limit_mb, Some(1024));
    assert_eq!(v1_unclamped.effective_max_memory_mb, 512);
    assert!(!v1_unclamped.cgroup_clamped);

    let v1_clamped = resolve_memory_cap(Some(1024), &missing, &cg_v1).unwrap();
    assert_eq!(v1_clamped.effective_max_memory_mb, 768);
    assert!(v1_clamped.cgroup_clamped);
    assert_eq!(
        v1_clamped.js_heap_flag,
        "--js-flags=--max-old-space-size=768"
    );

    // 3. Cgroup v2 `"max"` with missing v1 -> unbounded host memory
    let cg_v2_max = tmp.path().join("memory.max.unbounded");
    fs::write(&cg_v2_max, "max\n").unwrap();
    let unbounded = resolve_memory_cap(Some(512), &cg_v2_max, &missing).unwrap();
    assert_eq!(unbounded.cgroup_source, "host_unbounded");
    assert_eq!(unbounded.cgroup_limit_mb, None);
    assert_eq!(unbounded.effective_max_memory_mb, 512);
    assert!(!unbounded.cgroup_clamped);

    // 4. P2-2 Regression: Hybrid cgroup v1+v2 where v2 == `"max"` falls through to v1 limit!
    let cg_v1_512 = tmp.path().join("memory.limit_in_bytes.512");
    fs::write(&cg_v1_512, format!("{}\n", 512u64 * 1024 * 1024)).unwrap();
    let hybrid_cap = resolve_memory_cap(Some(512), &cg_v2_max, &cg_v1_512).unwrap();
    assert_eq!(hybrid_cap.cgroup_source, "cgroup_v1");
    assert_eq!(hybrid_cap.cgroup_limit_mb, Some(512));
    assert_eq!(hybrid_cap.effective_max_memory_mb, 384);
    assert!(hybrid_cap.cgroup_clamped);

    // 5. Out-of-range memory caps rejected
    assert!(resolve_memory_cap(Some(64), &missing, &missing).is_err());
    assert!(resolve_memory_cap(Some(32_768), &missing, &missing).is_err());

    // 6. P2-1 Regression: `COWOK_MAX_MEMORY_MB` env var honored when CLI arg omitted
    assert_eq!(
        resolve_requested_max_memory_mb(None, Some("384")).unwrap(),
        Some(384)
    );
    assert_eq!(
        resolve_requested_max_memory_mb(Some(768), Some("384")).unwrap(),
        Some(768)
    );
    assert!(resolve_requested_max_memory_mb(None, Some("not-a-num")).is_err());

    let mut env_overrides = clean_isolated_overrides(&tmp);
    env_overrides.cgroup_v2_path = Some(missing.clone());
    env_overrides.cgroup_v1_path = Some(missing.clone());
    env_overrides.env_max_memory_mb = Some(Some("256".to_string()));
    let resp = run_ci_gate_with_overrides(
        &CiGateRequest {
            project_id: "env-mem-cap".to_string(),
            max_memory_mb: None,
            force_no_browser: true,
            workspace: Some(tmp.path().to_path_buf()),
            ..Default::default()
        },
        &env_overrides,
    )
    .await
    .expect("valid COWOK_MAX_MEMORY_MB must succeed");
    assert_eq!(resp.diagnostics.memory_cap.requested_max_memory_mb, 256);
    assert_eq!(resp.diagnostics.memory_cap.effective_max_memory_mb, 256);
    assert_eq!(
        resp.diagnostics.memory_cap.js_heap_flag,
        "--js-flags=--max-old-space-size=256"
    );

    let mut bad_env_overrides = clean_isolated_overrides(&tmp);
    bad_env_overrides.env_max_memory_mb = Some(Some("32".to_string()));
    let err = run_ci_gate_with_overrides(
        &CiGateRequest {
            project_id: "env-mem-bad".to_string(),
            max_memory_mb: None,
            force_no_browser: true,
            workspace: Some(tmp.path().to_path_buf()),
            ..Default::default()
        },
        &bad_env_overrides,
    )
    .await
    .expect_err("out-of-range COWOK_MAX_MEMORY_MB must fail validation");
    assert_eq!(err.exit_code, 2);
    assert_eq!(err.code, "INVALID_MAX_MEMORY_MB");
}

// -----------------------------------------------------------------------------
// ut_12_3_06: Read-Only (`0555`) Bind-Mount Workspace SQLite WAL Staging (Host-XDG Isolated)
// -----------------------------------------------------------------------------
#[cfg(unix)]
#[tokio::test]
async fn ut_12_3_06_readonly_bind_mount_sqlite_wal_staging() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    let ro_ws = tmp.path().join("readonly-workspace");
    let staging_home = tmp.path().join("writable-staging-home");
    fs::create_dir_all(&ro_ws).unwrap();
    fs::create_dir_all(&staging_home).unwrap();

    let html_fixture = ro_ws.join("index.html");
    fs::write(
        &html_fixture,
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"><title>Read-Only Workspace Fixture</title></head><body><main><h1>Read-Only Workspace Fixture</h1><p>Clean markup mounted read-only.</p></main></body></html>",
    )
    .unwrap();

    // Lock workspace strictly read-only (`0555`)
    fs::set_permissions(&ro_ws, fs::Permissions::from_mode(0o555)).unwrap();

    let project_id = Uuid::new_v4().to_string();
    let mut overrides = clean_isolated_overrides(&tmp);
    overrides.fallback_home = Some(staging_home.clone());

    let gate_res = run_ci_gate_with_overrides(
        &CiGateRequest {
            project_id: project_id.clone(),
            html_path: Some(html_fixture),
            force_no_browser: true,
            fail_on_unverified: false,
            workspace: Some(ro_ws.clone()),
            ..Default::default()
        },
        &overrides,
    )
    .await;

    // Always restore permissions before assertions so TempDir cleans up deterministically
    let _ = fs::set_permissions(&ro_ws, fs::Permissions::from_mode(0o755));

    let resp = gate_res.expect("ci gate must succeed on read-only 0555 workspace");
    assert_eq!(resp.exit_code, 0);
    assert!(resp.diagnostics.wal_staging.read_only_fallback);
    assert!(
        !ro_ws.join(".cowok-design").exists(),
        "zero files may be created inside read-only bind mount"
    );

    let staged_db = PathBuf::from(&resp.diagnostics.wal_staging.db_path);
    let staged_root = PathBuf::from(&resp.diagnostics.wal_staging.state_root);
    assert!(staged_db.is_file());
    assert!(!staged_db.starts_with(&ro_ws));
    assert!(
        staged_db.starts_with(&staging_home),
        "staged DB must stay isolated inside fallback_home even when host XDG_DATA_HOME is set: {}",
        staged_db.display()
    );

    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&staged_db, &staged_root)).unwrap();
    let conn = pool.connection().unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM qc_gate_runs WHERE project_id = ?1",
            rusqlite::params![project_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

// -----------------------------------------------------------------------------
// ut_12_3_07: Zero LibreOffice Invariant & Non-Root UID Diagnostics
// -----------------------------------------------------------------------------
#[tokio::test]
async fn ut_12_3_07_zero_libreoffice_and_nonroot_uid_diagnostics() {
    let tmp = TempDir::new().unwrap();
    let clean_bin = tmp.path().join("clean-bin");
    let tainted_bin = tmp.path().join("tainted-bin");
    fs::create_dir_all(&clean_bin).unwrap();
    fs::create_dir_all(&tainted_bin).unwrap();

    assert!(!detect_libreoffice_in_dirs(std::slice::from_ref(
        &clean_bin
    )));

    let fake_soffice = tainted_bin.join("soffice");
    fs::write(&fake_soffice, "#!/bin/sh\necho fake\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fake_soffice, fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert!(detect_libreoffice_in_dirs(std::slice::from_ref(
        &tainted_bin
    )));

    // Running `ci gate` when `soffice` is detected MUST fail with exit code 3
    let resp = run_ci_gate_with_overrides(
        &CiGateRequest {
            project_id: "libreoffice-violation".to_string(),
            force_no_browser: true,
            workspace: Some(tmp.path().to_path_buf()),
            ..Default::default()
        },
        &CiRuntimeProbeOverrides {
            libreoffice_search_dirs: Some(vec![tainted_bin]),
            uid_gid_override: Some((EXPECTED_CONTAINER_UID, EXPECTED_CONTAINER_UID)),
            env_max_memory_mb: Some(None),
            ..Default::default()
        },
    )
    .await
    .expect("ci gate returns structured FAIL verdict");
    assert_eq!(resp.verdict, CiGateVerdict::Fail);
    assert_eq!(resp.exit_code, 3);
    assert!(resp.diagnostics.libreoffice_binary_detected);
    assert!(!resp.diagnostics.zero_libreoffice_invariant);
    assert!(resp
        .failure_reasons
        .iter()
        .any(|r| r.contains("ZERO_LIBREOFFICE_VIOLATION")));

    // Non-root UID diagnostics
    let (root_uid, _, root_nonroot) = inspect_uid_gid_diagnostics(Some((0, 0)));
    assert_eq!(root_uid, 0);
    assert!(!root_nonroot);

    let (cowok_uid, cowok_gid, cowok_nonroot) =
        inspect_uid_gid_diagnostics(Some((EXPECTED_CONTAINER_UID, EXPECTED_CONTAINER_UID)));
    assert_eq!(cowok_uid, EXPECTED_CONTAINER_UID);
    assert_eq!(cowok_gid, EXPECTED_CONTAINER_UID);
    assert!(cowok_nonroot);
}

// -----------------------------------------------------------------------------
// ut_12_3_08: SIGTERM Graceful Child PGID Reaper, Entrypoint Syntax & Compose Hardening
// -----------------------------------------------------------------------------
#[test]
fn ut_12_3_08_sigterm_graceful_reaper_and_compose_hardening() {
    // 1. Spawn a real child PGID process (`sleep 30`), register in ChildProcessReaper + owned lockfile,
    //    trigger `reap_all()` simulating SIGTERM teardown, assert zero zombie PGIDs & lockfile removal.
    let tmp = TempDir::new().unwrap();
    let lock_path = tmp.path().join("daemon.lock");
    let nonce = generate_csprng_hex_32();
    let lock_info = DaemonLockInfo {
        pid: std::process::id(),
        host: "127.0.0.1".to_string(),
        port: 4170,
        started_at: domain::SafeJsInteger::new(chrono::Utc::now().timestamp_millis()).unwrap(),
        daemon_nonce: nonce.clone(),
        hmac_key_hex: generate_csprng_hex_32(),
        schema_version: CURRENT_SCHEMA_VERSION,
    };
    fs::write(&lock_path, serde_json::to_vec_pretty(&lock_info).unwrap()).unwrap();

    let reaper = ChildProcessReaper::new();
    reaper.set_owned_lockfile(lock_path.clone(), std::process::id(), nonce);

    let mut sleep_cmd = Command::new("sleep");
    sleep_cmd.arg("30");
    let (mut child, handle) = reaper
        .spawn_managed_child(sleep_cmd, "pooled-chromium-ci-worker")
        .expect("spawn child pgid");
    assert_eq!(reaper.tracked_count(), 1);

    let summary = reaper.reap_all();
    let _ = child.wait();
    assert_eq!(summary.children_reaped, 1);
    assert!(summary.lockfile_removed);
    assert!(!lock_path.exists());

    #[cfg(unix)]
    {
        // Confirm target PGID no longer exists (ESRCH)
        let probe = unsafe { libc::kill(-handle.pgid, 0) };
        assert_ne!(probe, 0);
    }

    // 2. Verify `packaging/docker/entrypoint.sh` syntax (`bash -n`) and `docker-compose.ci.yml` hardening
    let root = repo_root();
    let entrypoint = root.join("packaging/docker/entrypoint.sh");
    let compose_path = root.join("packaging/docker/docker-compose.ci.yml");

    let bash_check = Command::new("bash")
        .arg("-n")
        .arg(&entrypoint)
        .output()
        .expect("run bash -n on entrypoint.sh");
    assert!(
        bash_check.status.success(),
        "bash -n failed: {}",
        String::from_utf8_lossy(&bash_check.stderr)
    );

    let entrypoint_src = fs::read_to_string(&entrypoint).unwrap();
    assert!(entrypoint_src.contains("trap - TERM INT"));
    assert!(entrypoint_src.contains("is_pid_running_non_zombie"));
    assert!(entrypoint_src.contains("SELF_PGID"));
    assert!(entrypoint_src.contains("mktemp"));
    assert!(!entrypoint_src.contains(".cowok-uid-probe.$$"));
    assert!(entrypoint_src.contains("${RUNTIME_ROOT}/data"));

    let compose = fs::read_to_string(&compose_path).unwrap();
    assert!(compose.contains("read_only: true"));
    assert!(compose.contains("user: \"10001:10001\""));
    assert!(compose.contains("shm_size:"));
    assert!(compose.contains("mem_limit:"));
    assert!(compose.contains("no-new-privileges:true"));
    assert!(compose.contains("ALL"));
    assert!(compose.contains("127.0.0.1:4170:4170"));
    assert!(compose.contains("COWOK_CONTAINER_MODE: \"1\""));
    assert!(compose.contains("COWOK_CHROMIUM_NO_SANDBOX: \"1\""));
    assert!(compose.contains("COWOK_CONTAINER_BIND_ALL: \"1\""));
    assert!(!compose.lines().any(|line| line.trim() == "init: true"));
    for forbidden_port in [
        "3689", "5000", "6333", "6334", "6379", "6380", "6381", "6382", "6383", "6390", "7000",
        "11434", "54321", "54322", "54323", "54324",
    ] {
        assert!(
            !compose.contains(&format!(":{forbidden_port}")),
            "prohibited port {forbidden_port} found in docker-compose.ci.yml"
        );
    }
}

// -----------------------------------------------------------------------------
// ut_12_3_09 (P0-3 + P1-1): Live Pooled 1-Chromium Capture Pass & Wired Flags -> PASS (exit 0)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn ut_12_3_09_live_pooled_chromium_capture_wires_flags_and_passes_fail_on_unverified() {
    let tmp = TempDir::new().unwrap();
    let html_path = tmp.path().join("live-clean.html");
    fs::write(
        &html_path,
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"><title>Live Chromium Gate</title></head><body><main><h1>Live Verified Screen</h1><p>Clean semantic HTML verified via Pooled 1-Chromium CDP.</p></main></body></html>",
    )
    .unwrap();

    type CapturedInvocation = (String, Vec<String>);
    let captured_invocation: Arc<Mutex<Option<CapturedInvocation>>> = Arc::new(Mutex::new(None));
    let capture_sink = Arc::clone(&captured_invocation);

    let cg_v2 = tmp.path().join("memory.max");
    fs::write(&cg_v2, format!("{}\n", 512u64 * 1024 * 1024)).unwrap();

    let mut overrides = clean_isolated_overrides(&tmp);
    overrides.shm_available_override = Some(64 * 1024 * 1024); // triggers --disable-dev-shm-usage
    overrides.shm_total_override = Some(64 * 1024 * 1024);
    overrides.env_chromium_no_sandbox = Some(true); // triggers --no-sandbox inside hardened container
    overrides.cgroup_v2_path = Some(cg_v2); // clamps 512 MiB -> 384 MiB
    overrides.chromium_binary_override = Some(Some("/usr/bin/chromium".to_string()));
    overrides.live_capture_override = Some(Arc::new(move |bin, flags, html| {
        *capture_sink.lock().unwrap() = Some((bin.to_string(), flags.to_vec()));
        Ok(CiLiveCaptureOutput {
            chromium_binary: bin.to_string(),
            flags_used: flags.to_vec(),
            geometry_nodes: vec![DomGeometryNode {
                ff_id: "live-main".to_string(),
                tag: "main".to_string(),
                role: Some("main".to_string()),
                bbox: QcBoundingBox {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 120.0,
                },
                z_index: 0,
                scroll_width: 320.0,
                client_width: 320.0,
                scroll_height: 120.0,
                client_height: 120.0,
                font_size_px: 16.0,
                line_height_ratio: 1.5,
                fg_hex: None,
                bg_hex: None,
                border_radius_px: None,
                parent_radius_px: None,
                parent_padding_px: None,
                is_interactive: false,
                is_visible: true,
                pointer_events_none: false,
                text_len: html.len().min(200),
                has_aria_label: true,
                has_focus_ring: true,
            }],
            captured_dom_bytes: html.len(),
        })
    }));

    // (a) `force_no_browser: false` + `fail_on_unverified: true` MUST return PASS (exit 0)!
    let live_out = exec_ci_gate_with_overrides(
        CiGateRequest {
            project_id: "live-cdp-pass".to_string(),
            target_id: "live-screen".to_string(),
            html_path: Some(html_path.clone()),
            max_memory_mb: Some(512),
            force_no_browser: false,
            fail_on_unverified: true,
            workspace: Some(tmp.path().to_path_buf()),
        },
        true,
        &overrides,
    )
    .await;
    assert_eq!(
        live_out.exit_code, 0,
        "live Chromium capture with clean HTML must PASS even with --fail-on-unverified; stdout={}",
        live_out.stdout
    );
    let live_json: serde_json::Value = serde_json::from_str(&live_out.stdout).unwrap();
    assert_eq!(live_json["data"]["verdict"], "PASS");
    assert_eq!(live_json["data"]["passed"], true);
    assert_eq!(live_json["data"]["visualQc"]["verdict"], "PASS");
    assert_eq!(
        live_json["data"]["diagnostics"]["liveCaptureExecuted"],
        true
    );
    assert_eq!(
        live_json["data"]["diagnostics"]["shm"]["containerNoSandbox"],
        true
    );

    let (invoked_bin, invoked_flags) = captured_invocation
        .lock()
        .unwrap()
        .clone()
        .expect("live capture hook must have been invoked");
    assert_eq!(invoked_bin, "/usr/bin/chromium");
    assert!(invoked_flags.iter().any(|f| f == "--disable-dev-shm-usage"));
    assert!(invoked_flags.iter().any(|f| f == "--no-sandbox"));
    assert!(invoked_flags
        .iter()
        .any(|f| f == "--js-flags=--max-old-space-size=384"));

    // (b) Contrast: `force_no_browser: true` + `fail_on_unverified: false` -> UNVERIFIED_NO_BROWSER (exit 0)
    let no_browser_out = exec_ci_gate_with_overrides(
        CiGateRequest {
            project_id: "no-browser-contrast".to_string(),
            target_id: "live-screen".to_string(),
            html_path: Some(html_path.clone()),
            max_memory_mb: Some(512),
            force_no_browser: true,
            fail_on_unverified: false,
            workspace: Some(tmp.path().to_path_buf()),
        },
        true,
        &overrides,
    )
    .await;
    assert_eq!(no_browser_out.exit_code, 0);
    let nb_json: serde_json::Value = serde_json::from_str(&no_browser_out.stdout).unwrap();
    assert_eq!(nb_json["data"]["verdict"], "UNVERIFIED_NO_BROWSER");
    assert_eq!(nb_json["data"]["diagnostics"]["liveCaptureExecuted"], false);

    // (c) NEW-P1-1: Simulated Chromium capture crash MUST fail closed (never false-positive PASS)
    let mut crash_overrides = clean_isolated_overrides(&tmp);
    crash_overrides.chromium_binary_override = Some(Some("/usr/bin/chromium".to_string()));
    crash_overrides.live_capture_override = Some(Arc::new(|_bin, _flags, _html| {
        Err("simulated chromium crash".to_string())
    }));

    let crash_strict = exec_ci_gate_with_overrides(
        CiGateRequest {
            project_id: "live-crash-strict".to_string(),
            target_id: "live-screen".to_string(),
            html_path: Some(html_path.clone()),
            max_memory_mb: Some(512),
            force_no_browser: false,
            fail_on_unverified: true,
            workspace: Some(tmp.path().to_path_buf()),
        },
        true,
        &crash_overrides,
    )
    .await;
    assert_eq!(
        crash_strict.exit_code, 3,
        "crashed Chromium capture under --fail-on-unverified must exit 3: {}",
        crash_strict.stdout
    );
    let cs_json: serde_json::Value = serde_json::from_str(&crash_strict.stdout).unwrap();
    assert_eq!(cs_json["data"]["verdict"], "FAIL");
    assert_eq!(cs_json["data"]["passed"], false);
    assert_eq!(cs_json["data"]["diagnostics"]["liveCaptureExecuted"], false);
    assert!(
        cs_json["data"]["failureReasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r
                .as_str()
                .is_some_and(|s| s.contains("simulated chromium crash"))),
        "failureReasons must surface capture error"
    );

    let crash_non_strict = exec_ci_gate_with_overrides(
        CiGateRequest {
            project_id: "live-crash-nonstrict".to_string(),
            target_id: "live-screen".to_string(),
            html_path: Some(html_path),
            max_memory_mb: Some(512),
            force_no_browser: false,
            fail_on_unverified: false,
            workspace: Some(tmp.path().to_path_buf()),
        },
        true,
        &crash_overrides,
    )
    .await;
    assert_eq!(crash_non_strict.exit_code, 0);
    let cns_json: serde_json::Value = serde_json::from_str(&crash_non_strict.stdout).unwrap();
    assert_eq!(
        cns_json["data"]["verdict"], "UNVERIFIED_NO_BROWSER",
        "crashed capture must downgrade to UNVERIFIED_NO_BROWSER, never PASS"
    );
    assert_eq!(
        cns_json["data"]["diagnostics"]["liveCaptureExecuted"],
        false
    );
}

// -----------------------------------------------------------------------------
// ut_12_3_10 (P1-4): UNIQUE(slug) Collision Resistance (`"My App"` vs `"My/App"`)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn ut_12_3_10_project_slug_collision_resistance_in_same_workspace_db() {
    let tmp = TempDir::new().unwrap();
    let overrides = clean_isolated_overrides(&tmp);

    let first = run_ci_gate_with_overrides(
        &CiGateRequest {
            project_id: "My App".to_string(),
            force_no_browser: true,
            workspace: Some(tmp.path().to_path_buf()),
            ..Default::default()
        },
        &overrides,
    )
    .await
    .expect("first project 'My App' must succeed");
    assert_eq!(first.exit_code, 0);

    let second = run_ci_gate_with_overrides(
        &CiGateRequest {
            project_id: "My/App".to_string(),
            force_no_browser: true,
            workspace: Some(tmp.path().to_path_buf()),
            ..Default::default()
        },
        &overrides,
    )
    .await
    .expect("second project 'My/App' must not collide on UNIQUE(slug)");
    assert_eq!(second.exit_code, 0);

    let db_path = PathBuf::from(&first.diagnostics.wal_staging.db_path);
    let state_root = PathBuf::from(&first.diagnostics.wal_staging.state_root);
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, &state_root)).unwrap();
    let conn = pool.connection().unwrap();

    let project_count: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT slug) FROM projects WHERE id IN ('My App', 'My/App')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        project_count, 2,
        "both projects must have distinct collision-proof slugs"
    );

    let qc_run_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM qc_gate_runs WHERE project_id IN ('My App', 'My/App')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(qc_run_count, 2);
}
