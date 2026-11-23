// @story 11.1
// @implements AC-01, AC-02, AC-03, AC-04 (Security, C-02/C-03/H-01..H-04/M-01..M-05 Regression Tests)
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use bot_fleet::{
    resolve_and_pin_provider_endpoint, validate_model_id, validate_session_id, BotFleetError,
    ByokCredential, ByokGatewayRouter, CliAdapterRegistry, CooldownTracker, IsolatedSessionDir,
    PgidProcessSupervisor, PostExecutionNormalizer, PromptTransport, SandboxExecutionSpec,
    StagedPromptFile,
};
use serde_json::json;
use tempfile::TempDir;
use zeroize::Zeroizing;

#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};

#[test]
fn test_11_1_01_0600_fd_staging_and_raii_cleanup_on_drop_and_panic() {
    let tmp = TempDir::new().expect("tempdir");
    let session = IsolatedSessionDir::create(tmp.path(), "sess-0600").expect("create session");

    #[cfg(unix)]
    {
        let dir_mode = std::fs::metadata(session.root())
            .expect("session root meta")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700, "$COWOK_SESSION_DIR must be strictly 0700");
    }

    let secret_prompt = "TOP_SECRET_PROMPT_PAYLOAD_11_1_ALPHA";
    let staged_path: PathBuf;
    {
        let staged =
            StagedPromptFile::stage_in_session(&session, secret_prompt).expect("stage prompt");
        staged_path = staged.path().to_path_buf();
        assert!(staged_path.exists());
        assert_eq!(staged.byte_len(), secret_prompt.len());

        #[cfg(unix)]
        {
            let file_mode = std::fs::metadata(&staged_path)
                .expect("staged file meta")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600, "Staged prompt file must be strictly 0600");
        }
    }
    assert!(
        !staged_path.exists(),
        "RAII Drop must unlink staged 0o600 prompt immediately"
    );

    let base = tmp.path().to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel::<(PathBuf, PathBuf)>();
    let unwind = std::panic::catch_unwind(move || {
        let s = IsolatedSessionDir::create(&base, "sess-panic").expect("session");
        let p = StagedPromptFile::stage_in_session(&s, "PANIC_SECRET_PROMPT_999").expect("stage");
        tx.send((s.root().to_path_buf(), p.path().to_path_buf()))
            .expect("send paths");
        panic!("simulated worker panic");
    });
    assert!(unwind.is_err());
    let (panic_session_root, panic_prompt_path) = rx.recv().expect("recv paths");
    assert!(
        !panic_prompt_path.exists(),
        "Staged prompt file must be cleaned up on panic unwind"
    );
    assert!(
        !panic_session_root.exists(),
        "Isolated session dir must be cleaned up on panic unwind"
    );
}

#[cfg(unix)]
#[test]
fn test_11_1_02_argv_zero_secret_and_env_clear_isolation_c02() {
    let tmp = TempDir::new().expect("tempdir");
    let session = IsolatedSessionDir::create(tmp.path(), "sess-argv").expect("session");
    let secret_prompt = "CONFIDENTIAL_PROMPT_NEVER_IN_ARGV_884422";
    let secret_key = "sk-live-secret-never-in-cmdline-991177";

    // Ambient daemon secret set in parent process MUST NOT leak into child after env_clear()
    std::env::set_var("COWOK_SUPERADMIN_SECRET", "superadmin-top-secret-c02");
    std::env::set_var("AMBIENT_OPENAI_LEAK_TEST", "sk-ambient-parent-leak-c02");

    // 1. Static fail-closed guard rejects prompt or secret in argv
    let bad_spec = SandboxExecutionSpec {
        binary: PathBuf::from("/bin/echo"),
        args: vec![secret_prompt.to_string()],
        prompt: Zeroizing::new(secret_prompt.to_string()),
        prompt_transport: PromptTransport::StdinFd,
        working_dir: None,
        extra_env: Vec::new(),
        timeout_ms: 3000,
        max_output_bytes: 64 * 1024,
        redact_secrets: vec![Zeroizing::new(secret_key.to_string())],
    };
    let err = PgidProcessSupervisor::execute_in_session(&session, &bad_spec)
        .expect_err("prompt in argv must be blocked");
    assert!(matches!(err, BotFleetError::ArgvSecretLeakBlocked(_)));

    // 2. Real subprocess verifies zero ambient secret inheritance + extra_env secret redaction
    let script = r#"
ARGS_LINE=$(ps -p $$ -o args= 2>/dev/null || true)
STDIN_DATA=$(cat)
echo "ARGS=${ARGS_LINE}"
echo "STDIN=${STDIN_DATA}"
echo "SUPERADMIN=${COWOK_SUPERADMIN_SECRET:-UNSET}"
echo "AMBIENT=${AMBIENT_OPENAI_LEAK_TEST:-UNSET}"
echo "EXTRA_ECHO=${CHILD_TEMP_TOKEN:-UNSET}"
echo "COWOK_SESSION_DIR=${COWOK_SESSION_DIR}"
echo "CLAUDE_CONFIG_DIR=${CLAUDE_CONFIG_DIR}"
echo "GEMINI_CONFIG_DIR=${GEMINI_CONFIG_DIR}"
echo "CODEX_HOME=${CODEX_HOME}"
"#;
    // Construct synthetic redaction probe at runtime so static secret scanners never flag it after squash-merge
    let extra_secret = ["tok", "child", "secret", "echo", "redacted", "999"].join("-");
    let good_spec = SandboxExecutionSpec {
        binary: PathBuf::from("/bin/sh"),
        args: vec!["-c".to_string(), script.to_string()],
        prompt: Zeroizing::new(secret_prompt.to_string()),
        prompt_transport: PromptTransport::StdinFd,
        working_dir: None,
        extra_env: vec![(
            "CHILD_TEMP_TOKEN".to_string(),
            Zeroizing::new(extra_secret.to_string()),
        )],
        timeout_ms: 5000,
        max_output_bytes: 64 * 1024,
        redact_secrets: vec![Zeroizing::new(secret_key.to_string())],
    };
    let out = PgidProcessSupervisor::execute_in_session(&session, &good_spec)
        .expect("sandboxed sh execution");
    assert_eq!(out.exit_code, Some(0));
    assert!(!out.timed_out);
    assert!(out.pgid_reaped);

    let args_line = out
        .stdout
        .lines()
        .find(|l| l.starts_with("ARGS="))
        .unwrap_or("");
    assert!(!args_line.contains(secret_prompt));
    assert!(!args_line.contains(secret_key));
    assert!(out.stdout.contains(&format!("STDIN={secret_prompt}")));
    assert!(
        out.stdout.contains("SUPERADMIN=UNSET"),
        "C-02: COWOK_SUPERADMIN_SECRET must NOT inherit into sandboxed child: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("AMBIENT=UNSET"),
        "C-02: Ambient host vars must NOT inherit into sandboxed child: {}",
        out.stdout
    );
    assert!(
        !out.stdout.contains(&extra_secret),
        "C-02: extra_env secret values must be redacted from stdout"
    );
    assert!(out.stdout.contains("EXTRA_ECHO=[REDACTED]"));

    let session_root_str = session.root().to_string_lossy().to_string();
    assert!(out
        .stdout
        .contains(&format!("COWOK_SESSION_DIR={session_root_str}")));
    assert!(out.stdout.contains(&format!(
        "CLAUDE_CONFIG_DIR={session_root_str}/xdg_config/claude"
    )));
    assert!(out.stdout.contains(&format!(
        "GEMINI_CONFIG_DIR={session_root_str}/xdg_config/gemini"
    )));
    assert!(out
        .stdout
        .contains(&format!("CODEX_HOME={session_root_str}/xdg_config/codex")));
}

#[cfg(unix)]
#[test]
fn test_11_1_03_pgid_child_tree_kill_and_zero_zombie_reaping() {
    let tmp = TempDir::new().expect("tempdir");
    let session = IsolatedSessionDir::create(tmp.path(), "sess-pgid").expect("session");
    let pid_file = tmp.path().join("grandchild.pid");

    let script = format!(
        "sleep 30 & GC_PID=$!; echo $GC_PID > '{}'; wait",
        pid_file.display()
    );
    let spec = SandboxExecutionSpec {
        binary: PathBuf::from("/bin/sh"),
        args: vec!["-c".to_string(), script],
        prompt: Zeroizing::new("pgid-timeout-test".to_string()),
        prompt_transport: PromptTransport::StdinFd,
        working_dir: None,
        extra_env: Vec::new(),
        timeout_ms: 300,
        max_output_bytes: 16 * 1024,
        redact_secrets: Vec::new(),
    };

    let out = PgidProcessSupervisor::execute_in_session(&session, &spec)
        .expect("supervisor executes and times out cleanly");
    assert!(out.timed_out);
    assert!(out.pgid_reaped);

    std::thread::sleep(std::time::Duration::from_millis(80));
    if let Ok(raw_pid) = std::fs::read_to_string(&pid_file) {
        if let Ok(gc_pid) = raw_pid.trim().parse::<libc::pid_t>() {
            let alive = unsafe { libc::kill(gc_pid, 0) } == 0;
            assert!(
                !alive,
                "Grandchild PID {gc_pid} must be dead after -PGID SIGKILL"
            );
        }
    }

    let ps_out = std::process::Command::new("ps")
        .args(["-p", &out.pid.to_string(), "-o", "state="])
        .output()
        .expect("ps check");
    let state = String::from_utf8_lossy(&ps_out.stdout).trim().to_string();
    assert!(state.is_empty() || !state.starts_with('Z'));
}

#[cfg(unix)]
#[test]
fn test_11_1_04_o_nofollow_symlink_trap_and_session_id_validation() {
    let tmp = TempDir::new().expect("tempdir");
    let outside_target = tmp.path().join("outside-host-file.txt");
    std::fs::write(&outside_target, "ORIGINAL_UNTOUCHED").expect("write outside");

    let session = IsolatedSessionDir::create(tmp.path(), "sess-symlink").expect("session");
    let trap_link = session.mcp_dir().join("evil-link.json");
    symlink(&outside_target, &trap_link).expect("create symlink trap");

    let write_err = session
        .write_isolated_file_0600("mcp", "evil-link.json", b"OVERWRITTEN")
        .expect_err("O_NOFOLLOW must reject symlink target");
    assert!(matches!(write_err, BotFleetError::SymlinkViolation(_)));

    let walk_err = session
        .validate_rel_path_nofollow(Path::new("mcp/evil-link.json"))
        .expect_err("validate_rel_path_nofollow must reject symlink component");
    assert!(matches!(walk_err, BotFleetError::SymlinkViolation(_)));

    assert!(validate_session_id("valid-sess_01.ok").is_ok());
    assert!(validate_session_id("../escape").is_err());
    assert!(validate_session_id("bad session!").is_err());

    let intact = std::fs::read_to_string(&outside_target).expect("read outside");
    assert_eq!(intact, "ORIGINAL_UNTOUCHED");
}

#[test]
fn test_11_1_05_adapters_catalog_and_strict_model_id_h03_m05() {
    let catalog = CliAdapterRegistry::catalog();
    assert!(catalog.len() >= 26);

    assert!(validate_model_id("claude-sonnet-4-5", true).is_ok());
    assert!(validate_model_id("anthropic/claude-sonnet-4", true).is_ok());
    assert!(validate_model_id("gemini-2.5-pro", false).is_ok());
    assert!(validate_model_id("models/../../admin", false).is_err());
    assert!(validate_model_id("-malicious-flag", true).is_err());
    assert!(validate_model_id("gpt-4?key=leak", true).is_err());
    assert!(validate_model_id("gpt-4#frag", true).is_err());

    // M-05: relative PATH dir & non-executable file must NOT resolve
    let tmp = TempDir::new().expect("tempdir");
    let non_exec = tmp.path().join("fake-non-exec-cli");
    std::fs::write(&non_exec, "#!/bin/sh\necho hi\n").expect("write");
    #[cfg(unix)]
    std::fs::set_permissions(&non_exec, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let old_path = std::env::var_os("PATH");
    std::env::set_var("PATH", format!(".:{}", tmp.path().display()));
    assert!(CliAdapterRegistry::resolve_binary("fake-non-exec-cli").is_none());
    if let Some(p) = old_path {
        std::env::set_var("PATH", p);
    }
}

#[tokio::test]
async fn test_11_1_06_byok_ssrf_c03_tenant_scoped_429_h04_and_failover() {
    // C-03: SSRF hardening checks
    assert!(resolve_and_pin_provider_endpoint(
        "http://[::ffff:169.254.169.254]/latest",
        "openai",
        false
    )
    .await
    .is_err());
    assert!(
        resolve_and_pin_provider_endpoint("https://[::ffff:10.0.0.1]/v1", "openai", false)
            .await
            .is_err()
    );
    assert!(
        resolve_and_pin_provider_endpoint("http://127.0.0.1:6379/", "byos", false)
            .await
            .is_err()
    );
    assert!(
        resolve_and_pin_provider_endpoint("http://127.0.0.1:6390/", "byos", false)
            .await
            .is_err()
    );
    assert!(
        resolve_and_pin_provider_endpoint("http://127.0.0.1:8080/v1", "openai", false)
            .await
            .is_err()
    );
    assert!(
        resolve_and_pin_provider_endpoint("http://127.0.0.1:8080/v1", "byos", false)
            .await
            .is_ok()
    );

    let primary_hits = Arc::new(AtomicUsize::new(0));
    let backup_hits = Arc::new(AtomicUsize::new(0));
    let p_hits = Arc::clone(&primary_hits);
    let b_hits = Arc::clone(&backup_hits);

    let app = Router::new()
        .route(
            "/gemini/models/:model",
            post(move || {
                let c = Arc::clone(&p_hits);
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    let mut headers = HeaderMap::new();
                    headers.insert("retry-after", "15".parse().unwrap());
                    (
                        StatusCode::TOO_MANY_REQUESTS,
                        headers,
                        Json(json!({ "error": "quota exceeded" })),
                    )
                        .into_response()
                }
            }),
        )
        .route(
            "/openai/chat/completions",
            post(move || {
                let c = Arc::clone(&b_hits);
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    (
                        StatusCode::OK,
                        Json(json!({
                            "choices": [{
                                "message": { "content": "```json\n{\"status\": \"ok\",}\n```" }
                            }]
                        })),
                    )
                }
            }),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let tracker = CooldownTracker::new();
    let router = ByokGatewayRouter::with_loopback_test_endpoints(tracker.clone());

    let creds = vec![
        ByokCredential {
            provider_id: "gemini".to_string(),
            api_key: Zeroizing::new("AIzaSySuperSecretGeminiKey1234567890".to_string()),
            endpoint_url: Some(format!("http://{addr}/gemini")),
            model_id: Some("gemini-2.5-pro".to_string()),
        },
        ByokCredential {
            provider_id: "openai".to_string(),
            api_key: Zeroizing::new("sk-SuperSecretOpenAiKey1234567890".to_string()),
            endpoint_url: Some(format!("http://{addr}/openai")),
            model_id: Some("gpt-4.1".to_string()),
        },
    ];

    let res1 = router
        .execute_with_failover(
            "tenant-a",
            "Generate layout",
            Some("gemini"),
            None,
            &creds,
            5000,
        )
        .await
        .expect("failover to openai should succeed");
    assert_eq!(res1.provider_id, "openai");
    assert!(res1.cooldown_applied);
    assert_eq!(primary_hits.load(Ordering::SeqCst), 1);

    // H-04: Tenant A is in cooldown, Tenant B MUST NOT observe Tenant A's cooldown!
    let snap_a = tracker.snapshot("tenant-a", &["gemini".to_string()]);
    let snap_b = tracker.snapshot("tenant-b", &["gemini".to_string()]);
    assert!(
        snap_a
            .iter()
            .find(|p| p.provider_id == "gemini")
            .unwrap()
            .in_cooldown
    );
    assert!(
        !snap_b
            .iter()
            .find(|p| p.provider_id == "gemini")
            .unwrap()
            .in_cooldown
    );

    // Tenant A skips Gemini while latched
    let res2 = router
        .execute_with_failover(
            "tenant-a",
            "Second prompt",
            Some("gemini"),
            None,
            &creds,
            5000,
        )
        .await
        .expect("tenant-a skips cooled-down gemini");
    assert_eq!(res2.provider_id, "openai");
    assert_eq!(primary_hits.load(Ordering::SeqCst), 1);
    assert_eq!(backup_hits.load(Ordering::SeqCst), 2);

    server.abort();
}

#[test]
fn test_11_1_07_normalizer_json_first_and_svg_xss_hardening_h02() {
    // H-02: JSON containing <svg> MUST parse cleanly before SVG rewriting
    let json_with_svg = r#"```json
{"icon": "<svg width=\"24\" height=\"24\"><path d=\"M0 0\"/></svg>", "ok": true,}
```"#;
    let r_json = PostExecutionNormalizer::normalize(json_with_svg);
    assert_eq!(r_json.json_blocks_extracted, 1);
    assert_eq!(r_json.extracted_json.as_ref().unwrap()["ok"], json!(true));

    // H-02: SVG XSS fixed-point sanitization strips onload, foreignObject, javascript: href, nested script
    let xss_svg = "\x1b[32mHi\x1b[0m\n<svg width=\"48\" height=\"32\" onload=\"alert(1)\"><sc<script>x</script>ript>evil()</script><foreignObject><div>x</div></foreignObject><a xlink:href=\"javascript:alert(2)\"><path d=\"M0 0\"/></a><animate attributeName=\"href\" /></svg>";
    let r_xss = PostExecutionNormalizer::normalize(xss_svg);
    assert_eq!(r_xss.ansi_sequences_stripped, 2);
    assert_eq!(r_xss.svg_viewboxes_injected, 1);
    assert!(r_xss.normalized_content.contains("viewBox=\"0 0 48 32\""));
    assert!(!r_xss
        .normalized_content
        .to_ascii_lowercase()
        .contains("onload"));
    assert!(!r_xss
        .normalized_content
        .to_ascii_lowercase()
        .contains("<script"));
    assert!(!r_xss
        .normalized_content
        .to_ascii_lowercase()
        .contains("foreignobject"));
    assert!(!r_xss
        .normalized_content
        .to_ascii_lowercase()
        .contains("javascript:"));
    assert!(!r_xss
        .normalized_content
        .to_ascii_lowercase()
        .contains("<animate"));
}
