// @story 08.4
// @implements AC-04 (tokio-cron-scheduler, LeaseReaper, SSRF Redirect Guard, Hardened Git Clone, ETag & Backoff)
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use chrono::{TimeZone, Utc};
use croner::parser::CronParser;
use plugin_host::{
    ensure_safe_dir_no_symlink, load_verified_staged_bundle_from_dir, scan_skill_markdown_safety,
    validate_safe_segment, validate_skill_metadata_text, ManagedPlugin, ManagedPluginSnapshot,
    ManifestUpdateOutcome, PluginState, SkillManifest, SkillRepackager, StagedPluginBundle,
};
use spec_registry::plugin_store::{
    claim_cron_lease, commit_cron_and_plugin_fenced, find_plugin_for_tenant, get_plugin_scoped,
    list_active_cron_schedules, list_all_cron_schedules, reap_stale_cron_leases,
    refresh_cron_lease, update_plugin_cas, upsert_cron_schedule, FencedCronCommit,
    PluginCasUpdateSpec, PluginCronScheduleRow, SkillPluginRow, DEFAULT_LEASE_TTL_MS,
    MAX_CONSECUTIVE_CRON_FAILURES,
};
use spec_registry::SqlitePool;
use tokio::sync::{watch, Semaphore};
use tokio::task::JoinSet;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

use crate::routes::legacy_workbench_v6::{check_ssrf_socket_addr, validate_ssrf_safe_url};
use crate::vault::VaultStore;

const MAX_HTTP_REDIRECTS: usize = 3;
const HTTP_TIMEOUT_SECS: u64 = 15;
const GIT_TIMEOUT_SECS: u64 = 45;
const BASE_BACKOFF_MS: u64 = 30_000;
const MAX_BACKOFF_MS: u64 = 3_600_000;
const REAPER_INTERVAL_SECS: u64 = 15;

static IN_FLIGHT_PLUGINS: OnceLock<Mutex<HashSet<(String, String, String)>>> = OnceLock::new();

tokio::task_local! {
    static IN_CRON_WORKER_TICK: bool;
}

fn in_flight_map() -> &'static Mutex<HashSet<(String, String, String)>> {
    IN_FLIGHT_PLUGINS.get_or_init(|| Mutex::new(HashSet::new()))
}

struct SingleFlightGuard {
    key: (String, String, String),
}

impl SingleFlightGuard {
    fn try_acquire(tenant_id: &str, project_id: &str, plugin_id: &str) -> Option<Self> {
        let key = (
            tenant_id.to_string(),
            project_id.to_string(),
            plugin_id.to_string(),
        );
        let mut guard = in_flight_map().lock().expect("in_flight_plugins");
        if guard.contains(&key) {
            return None;
        }
        guard.insert(key.clone());
        Some(Self { key })
    }
}

impl Drop for SingleFlightGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = in_flight_map().lock() {
            guard.remove(&self.key);
        }
    }
}

fn is_github_token_eligible_host(host: &str) -> bool {
    let lower = host.trim().to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "github.com" | "api.github.com" | "gist.github.com" | "raw.githubusercontent.com"
    ) {
        return true;
    }
    if let Ok(extra) = std::env::var("COWOK_GITHUB_ENTERPRISE_HOSTS") {
        return extra
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .any(|allowed| !allowed.is_empty() && allowed == lower);
    }
    false
}

/// Validates a 5/6/7-field or `@nickname` cron expression using the single `croner` parser while enforcing no-nested-cron.
pub fn validate_cron_expression(expr: &str) -> Result<String, String> {
    let trimmed = expr.trim();
    if trimmed.is_empty() || trimmed.len() > 120 {
        return Err("Cron expression must be 1..=120 characters".to_string());
    }
    if IN_CRON_WORKER_TICK.try_with(|v| *v).unwrap_or(false) {
        return Err(
            "NESTED_CRON_FORBIDDEN: cron worker cannot mutate or spawn nested cron schedules"
                .to_string(),
        );
    }
    CronParser::new()
        .parse(trimmed)
        .map_err(|e| format!("Invalid cron expression '{trimmed}': {e}"))?;
    Ok(trimmed.to_string())
}

/// Evaluates whether any persisted cron expression has a scheduled tick in `(last_updated_ms, now_ms]`.
pub fn cron_is_due(expr: &str, last_updated_ms: i64, now_ms: i64) -> bool {
    if now_ms <= last_updated_ms {
        return false;
    }
    let Ok(schedule) = CronParser::new().parse(expr.trim()) else {
        return false;
    };
    let Some(last_dt) = Utc.timestamp_millis_opt(last_updated_ms).single() else {
        return false;
    };
    let Some(now_dt) = Utc.timestamp_millis_opt(now_ms).single() else {
        return false;
    };
    match schedule.find_next_occurrence(&last_dt, false) {
        Ok(next_dt) => next_dt <= now_dt,
        Err(_) => false,
    }
}

/// Pure exponential backoff helper (`30_000ms * 2^(failures-1)` clamped `<= 3_600_000ms` + deterministic/time jitter).
pub fn compute_backoff_ms(
    consecutive_failures: i32,
    retry_after_secs: Option<u64>,
    jitter_ms: u64,
) -> u64 {
    let exp = u32::try_from(consecutive_failures.saturating_sub(1).clamp(0, 7)).unwrap_or(0);
    let computed = BASE_BACKOFF_MS
        .saturating_mul(1u64 << exp)
        .min(MAX_BACKOFF_MS);
    let server_hint = retry_after_secs
        .map(|s| s.saturating_mul(1_000).min(MAX_BACKOFF_MS))
        .unwrap_or(0);
    let bounded_jitter = jitter_ms.min(5_000);
    computed
        .max(server_hint)
        .saturating_add(bounded_jitter)
        .min(MAX_BACKOFF_MS)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpstreamCheckResult {
    NotModified {
        etag: Option<String>,
    },
    HeadCommit {
        commit_sha: Option<String>,
        etag: Option<String>,
    },
    AuthRequired {
        status: u16,
        detail: String,
    },
    RateLimited {
        status: u16,
        retry_after_secs: Option<u64>,
        detail: String,
    },
    SsrfBlocked(String),
}

fn resolve_redirect_url(base_url: &str, location: &str) -> Result<String, String> {
    if location.starts_with("http://") || location.starts_with("https://") {
        return Ok(location.to_string());
    }
    let base_uri: axum::http::Uri = base_url
        .parse()
        .map_err(|e| format!("Invalid base URI '{base_url}': {e}"))?;
    let scheme = base_uri.scheme_str().unwrap_or("https");
    let authority = base_uri
        .authority()
        .map(|a| a.as_str())
        .ok_or_else(|| "Base URI missing authority".to_string())?;
    if location.starts_with('/') {
        Ok(format!("{scheme}://{authority}{location}"))
    } else {
        Ok(format!("{scheme}://{authority}/{location}"))
    }
}

/// Resolves a `3xx Location` target and enforces `validate_ssrf_safe_url(target, false)` on every hop.
pub fn validate_redirect_target(base_url: &str, location: &str) -> Result<String, String> {
    let next_url = resolve_redirect_url(base_url, location)?;
    validate_ssrf_safe_url(&next_url, false)
}

async fn resolve_and_pin_public_addr(
    target_url: &str,
) -> Result<(String, std::net::SocketAddr), String> {
    let clean_url = validate_ssrf_safe_url(target_url, false)?;
    let uri: axum::http::Uri = clean_url
        .parse()
        .map_err(|e| format!("Invalid target URI '{clean_url}': {e}"))?;
    let host = uri
        .host()
        .ok_or_else(|| "Target URL missing host".to_string())?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    let default_port = if uri.scheme_str() == Some("https") {
        443
    } else {
        80
    };
    let port = uri.port_u16().unwrap_or(default_port);
    let addrs = tokio::time::timeout(
        Duration::from_millis(1200),
        tokio::net::lookup_host((host.as_str(), port)),
    )
    .await
    .map_err(|_| "DNS lookup timed out (<=1200ms)".to_string())?
    .map_err(|e| format!("DNS lookup failed: {e}"))?;

    let mut chosen: Option<std::net::SocketAddr> = None;
    for sa in addrs {
        check_ssrf_socket_addr(sa, false)?;
        if chosen.is_none() {
            chosen = Some(sa);
        }
    }
    let addr = chosen.ok_or_else(|| format!("DNS returned no public addresses for '{host}'"))?;
    Ok((host, addr))
}

/// Issues a redirect-validated HTTP GET/HEAD probe enforcing `validate_ssrf_safe_url(url, false)`
/// plus synchronous per-hop DNS pinning (`resolve(host, pinned_addr)`) on BOTH initial URL and redirects.
pub async fn probe_upstream_with_ssrf_redirects(
    initial_url: &str,
    last_etag: Option<&str>,
    github_token: Option<&str>,
) -> UpstreamCheckResult {
    let mut current_url = match validate_ssrf_safe_url(initial_url, false) {
        Ok(u) => u,
        Err(reason) => return UpstreamCheckResult::SsrfBlocked(reason),
    };
    let initial_host: Option<String> = current_url
        .parse::<axum::http::Uri>()
        .ok()
        .and_then(|u| u.host().map(|h| h.to_ascii_lowercase()));

    for _hop in 0..=MAX_HTTP_REDIRECTS {
        let (host, pinned_addr) = match resolve_and_pin_public_addr(&current_url).await {
            Ok(pair) => pair,
            Err(reason) => return UpstreamCheckResult::SsrfBlocked(reason),
        };
        let client = match reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .resolve(&host, pinned_addr)
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return UpstreamCheckResult::RateLimited {
                    status: 500,
                    retry_after_secs: None,
                    detail: format!("HTTP client init failed: {e}"),
                }
            }
        };

        let mut req = client
            .get(&current_url)
            .header("User-Agent", "cowok-design-plugin-sync/1.0");
        if let Some(etag) = last_etag {
            req = req.header("If-None-Match", etag);
        }
        let same_https_github_origin = current_url.starts_with("https://")
            && current_url
                .parse::<axum::http::Uri>()
                .ok()
                .and_then(|u| u.host().map(|h| h.to_ascii_lowercase()))
                .is_some_and(|h| {
                    initial_host.as_deref() == Some(h.as_str()) && is_github_token_eligible_host(&h)
                });
        if same_https_github_origin {
            if let Some(tok) = github_token {
                req = req.header("Authorization", format!("Bearer {tok}"));
            }
        }
        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                return UpstreamCheckResult::RateLimited {
                    status: 503,
                    retry_after_secs: None,
                    detail: format!("Upstream probe error: {e}"),
                }
            }
        };
        let status = resp.status().as_u16();
        if status == 304 {
            let etag = resp
                .headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            return UpstreamCheckResult::NotModified { etag };
        }
        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            let Some(loc) = resp.headers().get("location").and_then(|v| v.to_str().ok()) else {
                return UpstreamCheckResult::RateLimited {
                    status,
                    retry_after_secs: None,
                    detail: "Redirect response missing Location header".to_string(),
                };
            };
            current_url = match validate_redirect_target(&current_url, loc) {
                Ok(u) => u,
                Err(reason) => return UpstreamCheckResult::SsrfBlocked(reason),
            };
            continue;
        }
        if matches!(status, 401 | 403) {
            return UpstreamCheckResult::AuthRequired {
                status,
                detail: format!("GitHub/upstream rejected credentials with HTTP {status}"),
            };
        }
        if status == 404 {
            if same_https_github_origin && github_token.is_none() {
                return UpstreamCheckResult::AuthRequired {
                    status,
                    detail: "GitHub upstream returned HTTP 404 without scoped PAT; configure token or verify repository"
                        .to_string(),
                };
            }
            return UpstreamCheckResult::RateLimited {
                status,
                retry_after_secs: None,
                detail: "Upstream returned HTTP 404 Not Found".to_string(),
            };
        }
        if status == 429 || status >= 500 || !(200..=299).contains(&status) {
            let retry_after_secs = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<u64>().ok());
            return UpstreamCheckResult::RateLimited {
                status,
                retry_after_secs,
                detail: format!("Upstream probe rejected non-success HTTP {status}"),
            };
        }
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let commit_sha = resp
            .headers()
            .get("x-cowok-commit-sha")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        drop(resp);
        return UpstreamCheckResult::HeadCommit { commit_sha, etag };
    }

    UpstreamCheckResult::SsrfBlocked("Exceeded maximum 3 redirects".to_string())
}

fn build_git_askpass_helper(dir: &Path) -> Result<PathBuf, String> {
    let script_path = dir.join("askpass.sh");
    let script = "#!/bin/sh\ncase \"$1\" in\n  *Username*) printf '%s\\n' 'x-access-token' ;;\n  *) printf '%s\\n' \"$COWOK_GIT_PAT\" ;;\nesac\n";
    std::fs::write(&script_path, script).map_err(|e| format!("Failed writing git askpass: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("Failed setting 0o700 on git askpass: {e}"))?;
    }
    Ok(script_path)
}

fn configure_hardened_git_cmd(
    cmd: &mut tokio::process::Command,
    safe_url: &str,
    github_token: Option<&str>,
    askpass_dir: Option<&Path>,
) -> Result<(), String> {
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .arg("-c")
        .arg("protocol.file.allow=never")
        .arg("-c")
        .arg("protocol.ext.allow=never")
        .arg("-c")
        .arg("protocol.ssh.allow=never")
        .arg("-c")
        .arg("http.followRedirects=false")
        .arg("-c")
        .arg("core.symlinks=false")
        .arg("-c")
        .arg("submodule.recurse=false");

    let is_github_host = safe_url
        .parse::<axum::http::Uri>()
        .ok()
        .and_then(|u| u.host().map(|h| h.to_ascii_lowercase()))
        .is_some_and(|h| is_github_token_eligible_host(&h));
    if let Some(tok) = github_token
        .filter(|t| !t.trim().is_empty() && safe_url.starts_with("https://") && is_github_host)
    {
        let dir = askpass_dir.ok_or_else(|| "Missing askpass temp dir".to_string())?;
        let askpass = build_git_askpass_helper(dir)?;
        cmd.env("GIT_ASKPASS", &askpass)
            .env("COWOK_GIT_PAT", tok.trim())
            .arg("-c")
            .arg(format!("core.askPass={}", askpass.display()));
    } else {
        cmd.env("GIT_ASKPASS", "true");
    }
    Ok(())
}

struct PinnedConnectProxyGuard {
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    proxy_url: String,
}

impl Drop for PinnedConnectProxyGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Runs an ephemeral single-target `CONNECT` proxy bound to `127.0.0.1:0` so external `git`/libcurl
/// connects exclusively to the SSRF-verified `pinned_addr` without re-resolving DNS.
async fn spawn_pinned_connect_proxy(
    expected_host: &str,
    pinned_addr: std::net::SocketAddr,
) -> Result<PinnedConnectProxyGuard, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Failed binding pinned git proxy: {e}"))?;
    let local_port = listener
        .local_addr()
        .map_err(|e| format!("Failed reading pinned git proxy addr: {e}"))?
        .port();
    let clean_host = expected_host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    let expected_authority = if clean_host.contains(':') {
        format!("[{clean_host}]:{}", pinned_addr.port())
    } else {
        format!("{clean_host}:{}", pinned_addr.port())
    };
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accept_res = listener.accept() => {
                    let Ok((mut client_stream, _)) = accept_res else { break };
                    let expected_auth = expected_authority.clone();
                    tokio::spawn(async move {
                        let mut buf = vec![0u8; 4096];
                        let mut total = 0usize;
                        while total < buf.len() {
                            let Ok(n) = client_stream.read(&mut buf[total..]).await else { return };
                            if n == 0 { return; }
                            total += n;
                            if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                        let req_head = String::from_utf8_lossy(&buf[..total]);
                        let first_line = req_head.lines().next().unwrap_or("");
                        let mut parts = first_line.split_whitespace();
                        let method = parts.next().unwrap_or("");
                        let target = parts.next().unwrap_or("").to_ascii_lowercase();
                        if method != "CONNECT" || target != expected_auth {
                            let _ = client_stream.write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n").await;
                            return;
                        }
                        let Ok(mut upstream) = tokio::net::TcpStream::connect(pinned_addr).await else {
                            let _ = client_stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
                            return;
                        };
                        if client_stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await.is_err() {
                            return;
                        }
                        let _ = tokio::io::copy_bidirectional(&mut client_stream, &mut upstream).await;
                    });
                }
            }
        }
    });
    Ok(PinnedConnectProxyGuard {
        shutdown_tx: Some(shutdown_tx),
        proxy_url: format!("http://127.0.0.1:{local_port}"),
    })
}

/// Resolves the upstream remote `HEAD` commit OID via hardened `git ls-remote` pinned to validated `SocketAddr`.
pub async fn ls_remote_head_hardened(
    repo_url: &str,
    github_token: Option<&str>,
) -> Result<Option<String>, String> {
    let safe_url = validate_ssrf_safe_url(repo_url, false)?;
    if !safe_url.starts_with("https://") {
        return Err("SSRF_BLOCKED: upstream git repository URL must use https://".to_string());
    }
    let (host, pinned_addr) = resolve_and_pin_public_addr(&safe_url).await?;
    let proxy = spawn_pinned_connect_proxy(&host, pinned_addr).await?;
    let ask_tmp = ScopedTempDir::create("cowok-git-ask")?;
    let mut cmd = tokio::process::Command::new("git");
    configure_hardened_git_cmd(&mut cmd, &safe_url, github_token, Some(ask_tmp.path()))?;
    cmd.arg("-c").arg(format!("http.proxy={}", proxy.proxy_url));
    cmd.arg("ls-remote").arg(&safe_url).arg("HEAD");
    let output = tokio::time::timeout(Duration::from_secs(GIT_TIMEOUT_SECS), cmd.output())
        .await
        .map_err(|_| format!("git ls-remote timed out after {GIT_TIMEOUT_SECS}s"))?
        .map_err(|e| format!("Failed to spawn git ls-remote: {e}"))?;
    drop(proxy);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("401")
            || stderr.contains("403")
            || stderr.contains("Authentication failed")
            || stderr.contains("could not read Username")
        {
            return Err(format!("AUTH_REQUIRED: git ls-remote rejected ({stderr})"));
        }
        return Err(format!("git ls-remote failed: {}", stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let sha = stdout
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .map(str::trim)
        .filter(|s| s.len() >= 40 && s.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_string);
    Ok(sha)
}

/// Clones an upstream Git repository cleanly with hardened security flags (pinned CONNECT proxy,
/// `GIT_ASKPASS` file custody, `http.followRedirects=false`, `protocol.file/ext/ssh.allow=never`,
/// `core.symlinks=false`, shallow `--depth=1 --no-recurse-submodules`).
pub async fn clone_repo_hardened(
    repo_url: &str,
    dest_dir: &Path,
    github_token: Option<&str>,
) -> Result<String, String> {
    let safe_url = validate_ssrf_safe_url(repo_url, false)?;
    if !safe_url.starts_with("https://") {
        return Err("SSRF_BLOCKED: upstream git repository URL must use https://".to_string());
    }
    let (host, pinned_addr) = resolve_and_pin_public_addr(&safe_url).await?;
    let proxy = spawn_pinned_connect_proxy(&host, pinned_addr).await?;
    let ask_tmp = ScopedTempDir::create("cowok-git-clone-ask")?;
    let mut cmd = tokio::process::Command::new("git");
    configure_hardened_git_cmd(&mut cmd, &safe_url, github_token, Some(ask_tmp.path()))?;
    cmd.arg("-c").arg(format!("http.proxy={}", proxy.proxy_url));

    cmd.arg("clone")
        .arg("--depth=1")
        .arg("--no-recurse-submodules")
        .arg(&safe_url)
        .arg(dest_dir);

    let output = tokio::time::timeout(Duration::from_secs(GIT_TIMEOUT_SECS), cmd.output())
        .await
        .map_err(|_| format!("Git clone timed out after {GIT_TIMEOUT_SECS}s"))?
        .map_err(|e| format!("Failed to spawn git: {e}"))?;
    drop(proxy);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("401")
            || stderr.contains("403")
            || stderr.contains("Authentication failed")
            || stderr.contains("could not read Username")
        {
            return Err(format!("AUTH_REQUIRED: git clone rejected ({stderr})"));
        }
        return Err(format!("Git clone failed: {}", stderr.trim()));
    }

    let head_out = tokio::process::Command::new("git")
        .arg("-C")
        .arg(dest_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .await
        .map_err(|e| format!("Failed to resolve HEAD commit: {e}"))?;
    Ok(String::from_utf8_lossy(&head_out.stdout).trim().to_string())
}

pub fn snapshot_from_plugin_row(row: &SkillPluginRow) -> Result<ManagedPlugin, String> {
    let manifest: SkillManifest =
        serde_json::from_str(&row.manifest_json).map_err(|e| e.to_string())?;
    let staging_manifest: Option<SkillManifest> = row
        .staging_manifest_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| e.to_string())?;
    let staging_diff_reasons: Vec<String> = row
        .staging_diff_reasons_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let state = PluginState::parse_compat(&row.lifecycle_state).map_err(|e| e.to_string())?;
    ManagedPlugin::from_persisted(ManagedPluginSnapshot {
        id: row.id.clone(),
        tenant_id: row.tenant_id.clone(),
        project_id: row.project_id.clone(),
        plugin_slug: row.plugin_slug.clone(),
        name: manifest.name.clone(),
        version: row.version.clone(),
        state,
        revision: row.revision,
        breaking_change_isolated: row.breaking_change_isolated,
        staging_version: row.staging_version.clone(),
        staging_manifest,
        staging_diff_reasons,
        manifest,
        quarantined: row.quarantined,
        quarantine_reason: row.quarantine_reason.clone(),
        updated_at: row.updated_at,
    })
    .map_err(|e| e.to_string())
}

/// Applies a verified fetched `StagedPluginBundle` against a persisted `SkillPluginRow`
/// inside `.cowok-staging-candidates/` (never touching live `.cowok-skills/<run_id>/`).
pub(crate) struct ScopedTempDir(PathBuf);

impl ScopedTempDir {
    pub(crate) fn create(prefix: &str) -> Result<Self, String> {
        let base = std::env::temp_dir();
        Self::create_in(&base, prefix)
    }

    pub(crate) fn create_in(parent: &Path, prefix: &str) -> Result<Self, String> {
        let safe_parent = ensure_safe_dir_no_symlink(parent).map_err(|e| e.to_string())?;
        let dir = safe_parent.join(format!("{prefix}-{}", Uuid::new_v4()));
        std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
        Ok(Self(dir))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScopedTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_clean_git_tree_excluding_dotgit(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    for entry in std::fs::read_dir(src).map_err(|e| format!("Failed reading clone dir: {e}"))? {
        let entry = entry.map_err(|e| format!("Failed iterating clone dir: {e}"))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == ".git" {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        let meta = std::fs::symlink_metadata(&src_path)
            .map_err(|e| format!("Failed inspecting cloned entry '{name_str}': {e}"))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "Security scan failed: symlink rejected in cloned repo at '{name_str}'"
            ));
        }
        if meta.is_dir() {
            std::fs::create_dir_all(&dst_path).map_err(|e| e.to_string())?;
            copy_clean_git_tree_excluding_dotgit(&src_path, &dst_path)?;
        } else if meta.is_file() {
            #[cfg(unix)]
            if meta.nlink() != 1 {
                return Err(format!(
                    "Security scan failed: hardlink rejected in cloned repo at '{name_str}' (nlink={})",
                    meta.nlink()
                ));
            }
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Failed copying cloned file '{name_str}': {e}"))?;
        }
    }
    Ok(())
}

pub(crate) fn read_clean_regular_utf8_nofollow(path: &Path, label: &str) -> Result<String, String> {
    use std::io::Read;
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    let file = opts
        .open(path)
        .map_err(|e| format!("Failed opening {label} with O_NOFOLLOW: {e}"))?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err(format!("{label} is not a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() != 1 {
            return Err(format!("{label} hardlink rejected (nlink != 1)"));
        }
    }
    const MAX_META_READ_BYTES: u64 = 1_048_576;
    if meta.len() > MAX_META_READ_BYTES {
        return Err(format!("{label} exceeds max {MAX_META_READ_BYTES}B bound"));
    }
    let mut out = String::new();
    file.take(MAX_META_READ_BYTES + 1)
        .read_to_string(&mut out)
        .map_err(|e| format!("Failed reading {label}: {e}"))?;
    Ok(out)
}

pub(crate) fn write_staged_source_bytes(
    src_root: &Path,
    rel_path: &str,
    prefix: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let rel_trimmed = rel_path.strip_prefix(prefix).unwrap_or(rel_path);
    let mut rel_clean = PathBuf::new();
    for comp in Path::new(rel_trimmed).components() {
        if let std::path::Component::Normal(seg) = comp {
            let s = seg.to_string_lossy();
            validate_safe_segment("bundle_segment", &s)?;
            rel_clean.push(seg);
        } else {
            return Err(format!("Unsafe staged path '{rel_path}'"));
        }
    }
    let dst_path = src_root.join(rel_clean);
    if let Some(parent) = dst_path.parent() {
        ensure_safe_dir_no_symlink(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(dst_path, bytes).map_err(|e| e.to_string())
}

fn synthesize_synced_skill_md_preserving_body(
    manifest: &SkillManifest,
    raw_skill_md: &str,
) -> Result<String, String> {
    let normalized = raw_skill_md.replace("\r\n", "\n");
    let body = normalized
        .strip_prefix("---\n")
        .and_then(|rest| rest.find("\n---").map(|idx| &rest[idx + 4..]))
        .unwrap_or("")
        .trim_start_matches('\n');
    let caps_json =
        serde_json::to_string(&manifest.capabilities_required).map_err(|e| e.to_string())?;
    let tags_line = if manifest.tags.is_empty() {
        String::new()
    } else {
        format!(
            "tags: {}\n",
            serde_json::to_string(&manifest.tags).map_err(|e| e.to_string())?
        )
    };
    let ep_line = if manifest.entry_point == "SKILL.md" {
        String::new()
    } else {
        format!(
            "entry_point: {}\n",
            serde_json::to_string(&manifest.entry_point).map_err(|e| e.to_string())?
        )
    };
    let fm_block = format!(
        "---\nname: {}\nversion: {}\ndescription: {}\n{ep_line}capabilities_required: {caps_json}\nrisk_tier: {}\n{tags_line}---\n\n",
        serde_json::to_string(&manifest.name).map_err(|e| e.to_string())?,
        serde_json::to_string(&manifest.version).map_err(|e| e.to_string())?,
        serde_json::to_string(&manifest.description).map_err(|e| e.to_string())?,
        manifest.risk_tier,
    );
    Ok(if body.is_empty() {
        format!("{fm_block}# {}\n", manifest.name)
    } else {
        format!("{fm_block}{body}")
    })
}

/// Extracts a cloned git checkout excluding `.git/` (rejecting symlinks/hardlinks) and verifies or repackages into a `StagedPluginBundle`.
pub fn prepare_verified_bundle_from_clone(
    clone_dir: &Path,
    plugin_row: &SkillPluginRow,
) -> Result<StagedPluginBundle, String> {
    let clean_dir = ScopedTempDir::create("cowok-clone-clean")?;
    copy_clean_git_tree_excluding_dotgit(clone_dir, clean_dir.path())?;
    if clean_dir.path().join("skills-lock.json").exists() {
        return load_verified_staged_bundle_from_dir(clean_dir.path(), None);
    }
    let skill_md_path = clean_dir.path().join("SKILL.md");
    if std::fs::symlink_metadata(&skill_md_path).is_err() {
        return Err(
            "BundleIntegrityMismatch: upstream git repo contains neither skills-lock.json nor SKILL.md"
                .to_string(),
        );
    }
    let skill_md = read_clean_regular_utf8_nofollow(&skill_md_path, "SKILL.md")?;
    let fm = plugin_host::parse_and_validate_skill_frontmatter(&skill_md)
        .map_err(|e| format!("FRONTMATTER_INVALID: {e}"))?;
    scan_skill_markdown_safety("SKILL.md", &skill_md).map_err(|e| e.to_string())?;
    validate_skill_metadata_text("SKILL.md name", &fm.name).map_err(|e| e.to_string())?;
    validate_skill_metadata_text("SKILL.md description", &fm.description)
        .map_err(|e| e.to_string())?;

    let upstream_plugin_json_path = clean_dir.path().join("plugin.json");
    let upstream_manifest = if std::fs::symlink_metadata(&upstream_plugin_json_path).is_ok() {
        let raw_pj = read_clean_regular_utf8_nofollow(&upstream_plugin_json_path, "plugin.json")?;
        let parsed: SkillManifest = serde_json::from_str(&raw_pj)
            .map_err(|e| format!("Invalid upstream plugin.json: {e}"))?;
        if !(1..=3).contains(&parsed.risk_tier) {
            return Err(format!(
                "Upstream plugin.json risk_tier {} out of 1..=3",
                parsed.risk_tier
            ));
        }
        if let Some(ref params) = parsed.parameters {
            if !params.is_null() && !params.is_object() && !params.is_boolean() {
                return Err(
                    "Upstream plugin.json parameters must be object, boolean, or null".to_string(),
                );
            }
        }
        validate_skill_metadata_text("plugin.json name", &parsed.name)
            .map_err(|e| e.to_string())?;
        validate_skill_metadata_text("plugin.json description", &parsed.description)
            .map_err(|e| e.to_string())?;
        if let Some(ref pov) = parsed.prompt_overview {
            scan_skill_markdown_safety("plugin.json prompt_overview", pov)
                .map_err(|e| e.to_string())?;
        }
        Some(parsed)
    } else {
        None
    };

    let mut manifest = match upstream_manifest {
        Some(mut up) => {
            if up.name.trim() != fm.name {
                return Err(
                    "BundleIntegrityMismatch: upstream plugin.json name diverges from SKILL.md"
                        .to_string(),
                );
            }
            if fm
                .version
                .as_deref()
                .is_some_and(|v| v != up.version.trim())
            {
                return Err(
                    "BundleIntegrityMismatch: upstream plugin.json version diverges from SKILL.md"
                        .to_string(),
                );
            }
            if fm.risk_tier.is_some_and(|rt| rt != up.risk_tier) {
                return Err(
                    "BundleIntegrityMismatch: upstream plugin.json risk_tier diverges from SKILL.md"
                        .to_string(),
                );
            }
            if fm
                .entry_point
                .as_deref()
                .is_some_and(|ep| ep != up.entry_point.trim())
            {
                return Err(
                    "BundleIntegrityMismatch: upstream plugin.json entry_point diverges from SKILL.md"
                        .to_string(),
                );
            }
            up.description = fm.description.clone();
            if !fm.capabilities_required.is_empty() {
                up.capabilities_required = fm.capabilities_required.clone();
            }
            if !fm.tags.is_empty() {
                up.tags = fm.tags.clone();
            }
            if up.prompt_overview.is_none() {
                up.prompt_overview = Some(fm.description.clone());
            }
            up
        }
        None => SkillManifest {
            name: fm.name.clone(),
            version: fm
                .version
                .clone()
                .unwrap_or_else(|| plugin_row.version.clone()),
            description: fm.description.clone(),
            entry_point: fm
                .entry_point
                .clone()
                .unwrap_or_else(|| "SKILL.md".to_string()),
            capabilities_required: fm.capabilities_required.clone(),
            risk_tier: fm.risk_tier.unwrap_or(1),
            parameters: None,
            tags: fm.tags.clone(),
            prompt_overview: Some(fm.description.clone()),
        },
    };
    manifest.name = manifest.name.trim().to_string();
    manifest.version = manifest.version.trim().to_string();
    manifest.description = manifest.description.trim().to_string();
    manifest.entry_point = manifest.entry_point.trim().to_string();

    let effective_skill_md = if fm.version.as_deref() == Some(manifest.version.as_str())
        && fm.risk_tier == Some(manifest.risk_tier)
        && fm.entry_point.as_deref().unwrap_or("SKILL.md") == manifest.entry_point
        && fm.capabilities_required == manifest.capabilities_required
    {
        skill_md
    } else {
        synthesize_synced_skill_md_preserving_body(&manifest, &skill_md)?
    };

    let repack_root = ScopedTempDir::create("cowok-clone-repack")?;
    let target_folder = manifest.name.clone();
    validate_safe_segment("skill_name", &target_folder)?;
    // `repackage_directory_with_manifest` synthesizes root package metadata from parsed manifest/`skill_md`
    // and rejects pre-existing reserved root filenames in `source_dir`.
    for reserved in [
        "SKILL.md",
        "plugin.json",
        "discovery-stub.json",
        "skills-lock.json",
    ] {
        let p = clean_dir.path().join(reserved);
        if p.exists() {
            std::fs::remove_file(&p)
                .map_err(|e| format!("Failed clearing root '{reserved}' before repackage: {e}"))?;
        }
    }
    let receipt = SkillRepackager::repackage_directory_with_manifest(
        clean_dir.path(),
        repack_root.path(),
        &target_folder,
        manifest,
        Some(&effective_skill_md),
        plugin_row.upstream_repo_url.as_deref(),
    )?;
    load_verified_staged_bundle_from_dir(Path::new(&receipt.target_dir), None)
}

fn prepare_synced_plugin_row(
    workspace_root: &Path,
    plugin_row: &SkillPluginRow,
    bundle: StagedPluginBundle,
    upstream_commit_sha: Option<&str>,
    fencing_token: Option<i64>,
) -> Result<(u64, SkillPluginRow, Option<PathBuf>), String> {
    let mut managed = snapshot_from_plugin_row(plugin_row)?;
    let expected_rev = managed.revision();
    let outcome = managed
        .apply_manifest_update(bundle.manifest.clone(), expected_rev, false)
        .map_err(|e| e.to_string())?;

    let mut next_row = plugin_row.clone();
    next_row.revision = managed.revision();
    next_row.lifecycle_state = managed.state().as_wire_str().to_string();
    next_row.version = managed.version().to_string();
    next_row.manifest_json =
        serde_json::to_string(managed.manifest()).map_err(|e| e.to_string())?;
    next_row.upstream_commit_sha = upstream_commit_sha
        .map(str::to_string)
        .or_else(|| plugin_row.upstream_commit_sha.clone());
    next_row.last_synced_at = Some(Utc::now().timestamp_millis());
    next_row.updated_at = Utc::now().timestamp_millis();
    let mut newly_created_stage_dir: Option<PathBuf> = None;

    match outcome {
        ManifestUpdateOutcome::Applied { .. } => {
            next_row.breaking_change_isolated = false;
            next_row.staging_version = None;
            next_row.staging_manifest_json = None;
            next_row.staging_diff_reasons_json = None;
            next_row.staging_bundle_dir = None;
            next_row.staging_package_sha256 = None;
        }
        ManifestUpdateOutcome::StagedForReview {
            candidate_version,
            reasons,
            ..
        } => {
            // Stage candidate repackaged bundle strictly OUTSIDE live `.cowok-skills/<run_id>/`
            validate_safe_segment("tenant_id", &plugin_row.tenant_id)?;
            validate_safe_segment("project_id", &plugin_row.project_id)?;
            validate_safe_segment("plugin_id", &plugin_row.id)?;
            validate_safe_segment("candidate_version", &candidate_version)?;
            let src_dir = ScopedTempDir::create("cowok-stage")?;
            for sc in &bundle.scripts {
                write_staged_source_bytes(
                    src_dir.path(),
                    &sc.rel_path,
                    "scripts/",
                    sc.content.as_bytes(),
                )?;
            }
            for rf in &bundle.references {
                write_staged_source_bytes(
                    src_dir.path(),
                    &rf.rel_path,
                    "references/",
                    rf.content.as_bytes(),
                )?;
            }
            for asset in &bundle.assets {
                write_staged_source_bytes(
                    src_dir.path(),
                    &asset.rel_path,
                    "assets/",
                    &asset.bytes,
                )?;
            }
            let version_segment = match fencing_token {
                Some(tok) => format!("{candidate_version}-f{tok}"),
                None => candidate_version.clone(),
            };
            let version_dir = workspace_root
                .join(".cowok-design")
                .join("plugin-staging")
                .join(&plugin_row.tenant_id)
                .join(&plugin_row.project_id)
                .join(&plugin_row.id)
                .join(&version_segment);
            if fencing_token.is_none() && version_dir.exists() {
                let _ = std::fs::remove_dir_all(&version_dir);
            }
            ensure_safe_dir_no_symlink(&version_dir).map_err(|e| e.to_string())?;
            let target_folder = bundle.manifest.name.clone();
            let receipt = SkillRepackager::repackage_directory_with_manifest(
                src_dir.path(),
                &version_dir,
                &target_folder,
                bundle.manifest.clone(),
                Some(&bundle.skill_md),
                plugin_row.upstream_repo_url.as_deref(),
            )?;

            newly_created_stage_dir = Some(version_dir);
            next_row.breaking_change_isolated = true;
            next_row.staging_version = Some(candidate_version);
            next_row.staging_manifest_json =
                Some(serde_json::to_string(&bundle.manifest).map_err(|e| e.to_string())?);
            next_row.staging_diff_reasons_json =
                Some(serde_json::to_string(&reasons).map_err(|e| e.to_string())?);
            next_row.staging_bundle_dir = Some(receipt.target_dir);
            next_row.staging_package_sha256 = Some(receipt.discovery_stub.package_sha256.clone());
        }
        ManifestUpdateOutcome::IdempotentStaged { .. } => {
            return Ok((expected_rev, plugin_row.clone(), None));
        }
    }

    Ok((expected_rev, next_row, newly_created_stage_dir))
}

pub fn apply_synced_bundle_to_plugin(
    pool: &SqlitePool,
    workspace_root: &Path,
    plugin_row: &SkillPluginRow,
    bundle: StagedPluginBundle,
    upstream_commit_sha: Option<&str>,
) -> Result<SkillPluginRow, String> {
    let (expected_rev, next_row, _) = prepare_synced_plugin_row(
        workspace_root,
        plugin_row,
        bundle,
        upstream_commit_sha,
        None,
    )?;
    if next_row.revision == plugin_row.revision
        && next_row.breaking_change_isolated == plugin_row.breaking_change_isolated
    {
        return Ok(plugin_row.clone());
    }
    let updated = update_plugin_cas(pool, expected_rev, &next_row, None)
        .map_err(|e| e.to_string())?
        .map_err(|_| "CAS revision conflict while persisting synced plugin bundle".to_string())?;
    Ok(updated)
}

/// Executes one complete claim-release-work-commit cycle for a plugin cron schedule.
pub async fn run_plugin_sync_cycle(
    pool: &SqlitePool,
    workspace_root: &Path,
    tenant_id: &str,
    project_id: &str,
    plugin_id: &str,
    force_manual: bool,
) -> Result<PluginCronScheduleRow, String> {
    let Some(_flight_guard) = SingleFlightGuard::try_acquire(tenant_id, project_id, plugin_id)
    else {
        return Err("SYNC_ALREADY_IN_FLIGHT".to_string());
    };

    let lease_owner = format!("cron-{}", Uuid::new_v4());
    let now_ms = Utc::now().timestamp_millis();
    let Some(claimed) = claim_cron_lease(
        pool,
        tenant_id,
        project_id,
        plugin_id,
        &lease_owner,
        force_manual,
        now_ms,
        DEFAULT_LEASE_TTL_MS,
    )
    .map_err(|e| e.to_string())?
    else {
        return Err("CRON_LEASE_BUSY_OR_BLOCKED".to_string());
    };

    // Spawn heartbeat task keeping lease alive every 10s while external I/O executes
    let (hb_cancel_tx, mut hb_cancel_rx) = watch::channel(false);
    let hb_pool = pool.clone();
    let hb_tenant = tenant_id.to_string();
    let hb_project = project_id.to_string();
    let hb_plugin = plugin_id.to_string();
    let hb_fence = claimed.fencing_token;
    let hb_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = hb_cancel_rx.changed() => {
                    if *hb_cancel_rx.borrow() { break; }
                }
                _ = interval.tick() => {
                    if !refresh_cron_lease(
                        &hb_pool,
                        &hb_tenant,
                        &hb_project,
                        &hb_plugin,
                        hb_fence,
                        Utc::now().timestamp_millis(),
                        DEFAULT_LEASE_TTL_MS,
                    )
                    .unwrap_or(false)
                    {
                        break;
                    }
                }
            }
        }
    });

    let preflight_row = plugin_exists_for_scope(pool, tenant_id, Some(project_id), plugin_id);
    match preflight_row {
        Ok(ref row) if row.quarantined || row.lifecycle_state == "ARCHIVED" => {
            let _ = hb_cancel_tx.send(true);
            let _ = hb_handle.await;
            let done_ms = Utc::now().timestamp_millis();
            let _ = commit_cron_and_plugin_fenced(
                pool,
                &FencedCronCommit {
                    tenant_id,
                    project_id,
                    plugin_id,
                    fencing_token: claimed.fencing_token,
                    sync_status: "FAILED",
                    last_etag: claimed.last_etag.as_deref(),
                    last_commit_sha: claimed.last_commit_sha.as_deref(),
                    consecutive_failures: claimed.consecutive_failures,
                    next_retry_at: None,
                    last_error_code: Some("PluginQuarantinedOrArchived"),
                    last_error_message: Some("Sync blocked for quarantined or archived plugin"),
                    updated_at: done_ms,
                },
                None,
            );
            return Err("PLUGIN_QUARANTINED_OR_ARCHIVED".to_string());
        }
        Ok(ref row) if row.breaking_change_isolated => {
            let _ = hb_cancel_tx.send(true);
            let _ = hb_handle.await;
            let done_ms = Utc::now().timestamp_millis();
            let _ = commit_cron_and_plugin_fenced(
                pool,
                &FencedCronCommit {
                    tenant_id,
                    project_id,
                    plugin_id,
                    fencing_token: claimed.fencing_token,
                    sync_status: "FAILED",
                    last_etag: claimed.last_etag.as_deref(),
                    last_commit_sha: claimed.last_commit_sha.as_deref(),
                    consecutive_failures: claimed.consecutive_failures,
                    next_retry_at: None,
                    last_error_code: Some("PendingStagingMustBeResolved"),
                    last_error_message: Some(
                        "Existing breaking-change staging candidate must be promoted or discarded before syncing further commits",
                    ),
                    updated_at: done_ms,
                },
                None,
            );
            return Err("PendingStagingMustBeResolved".to_string());
        }
        Err(e) => {
            let _ = hb_cancel_tx.send(true);
            let _ = hb_handle.await;
            return Err(e);
        }
        Ok(_) => {}
    }

    // Connection dropped before external network/git I/O.
    let vault = VaultStore::from_workspace_root(workspace_root);
    let github_tok = vault.get_github_sync_token_scoped(tenant_id).ok().flatten();
    let mut probe = probe_upstream_with_ssrf_redirects(
        &claimed.upstream_repo_url,
        claimed.last_etag.as_deref(),
        github_tok.as_deref().map(|z| z.as_str()),
    )
    .await;

    // Fallback to `git ls-remote` when standard Git hosts omit custom `x-cowok-commit-sha` headers
    if let UpstreamCheckResult::HeadCommit {
        commit_sha: None,
        etag,
    } = probe.clone()
    {
        match ls_remote_head_hardened(
            &claimed.upstream_repo_url,
            github_tok.as_deref().map(|z| z.as_str()),
        )
        .await
        {
            Ok(Some(remote_sha)) => {
                probe = UpstreamCheckResult::HeadCommit {
                    commit_sha: Some(remote_sha),
                    etag,
                };
            }
            Ok(None) => {}
            Err(e) if e.starts_with("AUTH_REQUIRED:") => {
                probe = UpstreamCheckResult::AuthRequired {
                    status: 401,
                    detail: e,
                };
            }
            Err(e) => {
                probe = UpstreamCheckResult::RateLimited {
                    status: 502,
                    retry_after_secs: None,
                    detail: e,
                };
            }
        }
    }

    let mut prepared_plugin_update: Option<(u64, SkillPluginRow, Option<PathBuf>)> = None;
    let mut prior_stage_dir_to_prune: Option<PathBuf> = None;

    if let UpstreamCheckResult::HeadCommit {
        commit_sha: Some(ref new_sha),
        ..
    } = probe.clone()
    {
        if claimed.last_commit_sha.as_deref() != Some(new_sha.as_str()) {
            match ScopedTempDir::create("cowok-cron-clone") {
                Ok(clone_tmp) => {
                    match clone_repo_hardened(
                        &claimed.upstream_repo_url,
                        clone_tmp.path(),
                        github_tok.as_deref().map(|z| z.as_str()),
                    )
                    .await
                    {
                        Ok(cloned_sha) => {
                            match plugin_exists_for_scope(
                                pool,
                                tenant_id,
                                Some(project_id),
                                plugin_id,
                            )
                            .and_then(|plugin_row| {
                                let bundle = prepare_verified_bundle_from_clone(
                                    clone_tmp.path(),
                                    &plugin_row,
                                )?;
                                prior_stage_dir_to_prune =
                                    plugin_row.staging_bundle_dir.as_deref().map(PathBuf::from);
                                prepare_synced_plugin_row(
                                    workspace_root,
                                    &plugin_row,
                                    bundle,
                                    Some(&cloned_sha),
                                    Some(claimed.fencing_token),
                                )
                            }) {
                                Ok(prep) => {
                                    prepared_plugin_update = Some(prep);
                                }
                                Err(e) => {
                                    probe = UpstreamCheckResult::RateLimited {
                                        status: 422,
                                        retry_after_secs: None,
                                        detail: format!("BUNDLE_VERIFICATION_FAILED: {e}"),
                                    };
                                }
                            }
                        }
                        Err(e) if e.starts_with("AUTH_REQUIRED:") => {
                            probe = UpstreamCheckResult::AuthRequired {
                                status: 401,
                                detail: e,
                            };
                        }
                        Err(e) => {
                            probe = UpstreamCheckResult::RateLimited {
                                status: 502,
                                retry_after_secs: None,
                                detail: e,
                            };
                        }
                    }
                }
                Err(e) => {
                    probe = UpstreamCheckResult::RateLimited {
                        status: 500,
                        retry_after_secs: None,
                        detail: e,
                    };
                }
            }
        }
    }

    let done_ms = Utc::now().timestamp_millis();
    let _ = hb_cancel_tx.send(true);
    let _ = hb_handle.await;

    let (status, next_etag, next_sha, failures, retry_at, err_code, err_msg) = match probe {
        UpstreamCheckResult::NotModified { etag } => (
            "SUCCEEDED",
            etag.or(claimed.last_etag.clone()),
            claimed.last_commit_sha.clone(),
            0,
            None,
            None,
            None,
        ),
        UpstreamCheckResult::HeadCommit { commit_sha, etag } => (
            "SUCCEEDED",
            etag.or(claimed.last_etag.clone()),
            commit_sha.or(claimed.last_commit_sha.clone()),
            0,
            None,
            None,
            None,
        ),
        UpstreamCheckResult::AuthRequired { status, detail } => (
            "AUTH_REQUIRED",
            claimed.last_etag.clone(),
            claimed.last_commit_sha.clone(),
            claimed.consecutive_failures.saturating_add(1),
            None,
            Some(format!("HTTP_{status}")),
            Some(detail),
        ),
        UpstreamCheckResult::SsrfBlocked(reason) => (
            "FAILED",
            claimed.last_etag.clone(),
            claimed.last_commit_sha.clone(),
            claimed.consecutive_failures.saturating_add(1),
            None,
            Some("SSRF_BLOCKED".to_string()),
            Some(reason),
        ),
        UpstreamCheckResult::RateLimited {
            status,
            retry_after_secs,
            detail,
        } => {
            let next_failures = claimed.consecutive_failures.saturating_add(1);
            if next_failures >= MAX_CONSECUTIVE_CRON_FAILURES || status == 422 {
                (
                    "FAILED",
                    claimed.last_etag.clone(),
                    claimed.last_commit_sha.clone(),
                    next_failures,
                    None,
                    Some(if status == 422 {
                        "BUNDLE_VERIFICATION_FAILED".to_string()
                    } else {
                        format!("HTTP_{status}_MAX_RETRIES")
                    }),
                    Some(detail),
                )
            } else {
                let jitter = u64::try_from((done_ms % 5000).abs()).unwrap_or(0);
                let delay = compute_backoff_ms(next_failures, retry_after_secs, jitter);
                (
                    "BACKOFF",
                    claimed.last_etag.clone(),
                    claimed.last_commit_sha.clone(),
                    next_failures,
                    Some(done_ms.saturating_add(i64::try_from(delay).unwrap_or(i64::MAX))),
                    Some(format!("HTTP_{status}")),
                    Some(detail),
                )
            }
        }
    };

    let fence_stage_dir = prepared_plugin_update
        .as_ref()
        .and_then(|(_, _, dir)| dir.clone());
    let plugin_update_spec = if status == "SUCCEEDED" {
        prepared_plugin_update
            .as_ref()
            .map(|(exp_rev, next_row, _)| PluginCasUpdateSpec {
                expected_revision: *exp_rev,
                next: next_row,
                audit: None,
            })
    } else {
        None
    };

    let committed = commit_cron_and_plugin_fenced(
        pool,
        &FencedCronCommit {
            tenant_id,
            project_id,
            plugin_id,
            fencing_token: claimed.fencing_token,
            sync_status: status,
            last_etag: next_etag.as_deref(),
            last_commit_sha: next_sha.as_deref(),
            consecutive_failures: failures,
            next_retry_at: retry_at,
            last_error_code: err_code.as_deref(),
            last_error_message: err_msg.as_deref(),
            updated_at: done_ms,
        },
        plugin_update_spec,
    )
    .inspect_err(|_| {
        if let Some(ref stage_dir) = fence_stage_dir {
            let _ = std::fs::remove_dir_all(stage_dir);
        }
    })
    .map_err(|e| e.to_string())?;

    if !committed {
        if let Some(ref stage_dir) = fence_stage_dir {
            let _ = std::fs::remove_dir_all(stage_dir);
        }
        return Err("CRON_ZOMBIE_FENCE_REJECTED".to_string());
    }

    if status == "SUCCEEDED" {
        if let Some(ref old_dir) = prior_stage_dir_to_prune {
            let new_target = prepared_plugin_update
                .as_ref()
                .and_then(|(_, r, _)| r.staging_bundle_dir.as_deref());
            if new_target != Some(old_dir.to_string_lossy().as_ref()) {
                let staging_root = workspace_root.join(".cowok-design").join("plugin-staging");
                if let (Ok(canon_root), Ok(canon_old)) = (
                    std::fs::canonicalize(&staging_root),
                    std::fs::canonicalize(old_dir),
                ) {
                    if canon_old.starts_with(&canon_root) && canon_old != canon_root {
                        let _ = std::fs::remove_dir_all(&canon_old);
                    }
                }
            }
        }
    }

    let conn = pool.connection().map_err(|e| e.to_string())?;
    spec_registry::plugin_store::get_cron_schedule_scoped(&conn, tenant_id, project_id, plugin_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Schedule missing after fenced commit".to_string())
}

fn broadcast_all_cron_projects(
    pool: &SqlitePool,
    sse_tx: Option<&tokio::sync::broadcast::Sender<String>>,
) {
    let Some(tx) = sse_tx else {
        return;
    };
    let Ok(conn) = pool.connection() else {
        return;
    };
    if let Ok(rows) = list_all_cron_schedules(&conn) {
        let mut seen = HashSet::new();
        for row in rows {
            if seen.insert(row.project_id.clone()) {
                let _ = tx.send(row.project_id);
            }
        }
    }
}

/// Spawns daemon startup `LeaseReaper`, `tokio-cron-scheduler`, and periodic lease cleanup loop.
pub async fn spawn_plugin_cron_runtime(
    pool: SqlitePool,
    workspace_root: PathBuf,
    sse_tx: Option<tokio::sync::broadcast::Sender<String>>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    // Run startup orphan reaper immediately before scheduling ticks
    if reap_stale_cron_leases(&pool, Utc::now().timestamp_millis()).unwrap_or(0) > 0 {
        broadcast_all_cron_projects(&pool, sse_tx.as_ref());
    }

    let scheduler_res = JobScheduler::new().await;
    if let Ok(sched) = scheduler_res {
        let p_clone = pool.clone();
        let ws_clone = workspace_root.clone();
        let sse_clone = sse_tx.clone();
        if let Ok(job) = Job::new_async("*/15 * * * * *", move |_uuid, _lock| {
            let p = p_clone.clone();
            let ws = ws_clone.clone();
            let sse = sse_clone.clone();
            Box::pin(IN_CRON_WORKER_TICK.scope(true, async move {
                let now_ms = Utc::now().timestamp_millis();
                if reap_stale_cron_leases(&p, now_ms).unwrap_or(0) > 0 {
                    broadcast_all_cron_projects(&p, sse.as_ref());
                }
                let schedules = {
                    let Ok(conn) = p.connection() else { return };
                    list_active_cron_schedules(&conn).unwrap_or_default()
                };
                let sem = Arc::new(Semaphore::new(4));
                let mut tasks = JoinSet::new();
                for sched_row in schedules {
                    if sched_row.sync_status == "AUTH_REQUIRED"
                        || sched_row.sync_status == "RUNNING"
                        || sched_row.sync_status == "FAILED"
                    {
                        continue;
                    }
                    if sched_row
                        .next_retry_at
                        .is_some_and(|next_at| next_at > now_ms)
                    {
                        continue;
                    }
                    if sched_row.next_retry_at.is_none()
                        && !cron_is_due(&sched_row.cron_expression, sched_row.updated_at, now_ms)
                    {
                        continue;
                    }
                    let Ok(permit) = sem.clone().acquire_owned().await else {
                        break;
                    };
                    let pool_task = p.clone();
                    let ws_task = ws.clone();
                    tasks.spawn(async move {
                        let _permit = permit;
                        let ok = run_plugin_sync_cycle(
                            &pool_task,
                            &ws_task,
                            &sched_row.tenant_id,
                            &sched_row.project_id,
                            &sched_row.plugin_id,
                            false,
                        )
                        .await
                        .is_ok();
                        (ok, sched_row.project_id)
                    });
                }
                while let Some(res) = tasks.join_next().await {
                    if let Ok((true, project_id)) = res {
                        if let Some(ref tx) = sse {
                            let _ = tx.send(project_id);
                        }
                    }
                }
            }))
        }) {
            if let Err(err) = sched.add(job).await {
                eprintln!("plugin cron scheduler failed adding job: {err}");
            } else if let Err(err) = sched.start().await {
                eprintln!("plugin cron scheduler failed starting: {err}");
            } else {
                while !*shutdown_rx.borrow() {
                    if shutdown_rx.changed().await.is_err() {
                        break;
                    }
                }
                return;
            }
        }
    }

    // Fallback interval loop if JobScheduler fails in restricted test sandboxes
    let mut ticker = tokio::time::interval(Duration::from_secs(REAPER_INTERVAL_SECS));
    loop {
        tokio::select! {
            res = shutdown_rx.changed() => {
                if res.is_err() || *shutdown_rx.borrow() { break; }
            }
            _ = ticker.tick() => {
                if reap_stale_cron_leases(&pool, Utc::now().timestamp_millis()).unwrap_or(0) > 0 {
                    broadcast_all_cron_projects(&pool, sse_tx.as_ref());
                }
            }
        }
    }
}

pub fn load_bundle_from_dir_verified(
    dir: &Path,
    expected_sha256: Option<&str>,
) -> Result<StagedPluginBundle, String> {
    load_verified_staged_bundle_from_dir(dir, expected_sha256).map_err(|e| e.to_string())
}

pub fn schedule_to_map_helper(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    plugin_id: &str,
    cron_expression: &str,
    upstream_repo_url: &str,
) -> Result<PluginCronScheduleRow, String> {
    let valid_expr = validate_cron_expression(cron_expression)?;
    let safe_url = validate_ssrf_safe_url(upstream_repo_url, false)?;
    if !safe_url.starts_with("https://") {
        return Err("SSRF_BLOCKED: upstreamRepoUrl must use https://".to_string());
    }
    upsert_cron_schedule(
        pool,
        tenant_id,
        project_id,
        plugin_id,
        &valid_expr,
        &safe_url,
        None,
    )
    .map_err(|e| e.to_string())
}

pub fn plugin_exists_for_scope(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: Option<&str>,
    plugin_id: &str,
) -> Result<SkillPluginRow, String> {
    let conn = pool.connection().map_err(|e| e.to_string())?;
    let opt = match project_id {
        Some(pid) => {
            get_plugin_scoped(&conn, tenant_id, pid, plugin_id).map_err(|e| e.to_string())?
        }
        None => find_plugin_for_tenant(&conn, tenant_id, plugin_id).map_err(|e| e.to_string())?,
    };
    opt.ok_or_else(|| format!("Plugin '{plugin_id}' not found"))
}

pub fn parse_manifest_json(raw: &str) -> HashMap<String, serde_json::Value> {
    serde_json::from_str(raw).unwrap_or_default()
}
