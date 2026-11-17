// @story 08.4
// @implements AC-04 (tokio-cron-scheduler, LeaseReaper, SSRF Redirect Guard, Hardened Git Clone, ETag & Backoff)
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use chrono::Utc;
use plugin_host::{
    load_verified_staged_bundle_from_dir, validate_safe_segment, ManagedPlugin,
    ManagedPluginSnapshot, ManifestUpdateOutcome, PluginState, SkillManifest, SkillRepackager,
    StagedPluginBundle,
};
use spec_registry::plugin_store::{
    claim_cron_lease, commit_cron_fenced, find_plugin_for_tenant, get_plugin_scoped,
    list_all_cron_schedules, reap_stale_cron_leases, refresh_cron_lease, update_plugin_cas,
    upsert_cron_schedule, FencedCronCommit, PluginCronScheduleRow, SkillPluginRow,
    DEFAULT_LEASE_TTL_MS, MAX_CONSECUTIVE_CRON_FAILURES,
};
use spec_registry::SqlitePool;
use tokio::sync::watch;
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

/// Validates a 5-field standard cron expression or 6-field Tokio cron expression while enforcing no-nested-cron.
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
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let normalized = match parts.len() {
        5 => format!("0 {trimmed}"),
        6 | 7 => trimmed.to_string(),
        _ => {
            return Err(format!(
                "Invalid cron field count ({}): expected 5-field standard or 6-field cron expression",
                parts.len()
            ));
        }
    };
    // Verify via tokio-cron-scheduler Job parser without registering
    Job::new_async(normalized.as_str(), |_id, _l| Box::pin(async {}))
        .map_err(|e| format!("Invalid cron expression '{trimmed}': {e}"))?;
    Ok(trimmed.to_string())
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
        if let Some(tok) = github_token {
            req = req.header("Authorization", format!("Bearer {tok}"));
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
        if status == 429 || status >= 500 {
            let retry_after_secs = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<u64>().ok());
            return UpstreamCheckResult::RateLimited {
                status,
                retry_after_secs,
                detail: format!("Upstream returned transient HTTP {status}"),
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
        return UpstreamCheckResult::HeadCommit { commit_sha, etag };
    }

    UpstreamCheckResult::SsrfBlocked("Exceeded maximum 3 redirects".to_string())
}

/// Clones an upstream Git repository cleanly with hardened security flags (no interactive prompt,
/// `protocol.file.allow=never`, `protocol.ext.allow=never`, `core.symlinks=false`, shallow `--depth=1 --no-recurse-submodules`).
pub async fn clone_repo_hardened(
    repo_url: &str,
    dest_dir: &Path,
    github_token: Option<&str>,
) -> Result<String, String> {
    let safe_url = validate_ssrf_safe_url(repo_url, false)?;
    let mut cmd = tokio::process::Command::new("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "true")
        .arg("-c")
        .arg("protocol.file.allow=never")
        .arg("-c")
        .arg("protocol.ext.allow=never")
        .arg("-c")
        .arg("core.symlinks=false");

    if let Some(tok) = github_token {
        let auth_val = format!("http.extraheader=Authorization: Bearer {tok}");
        cmd.arg("-c").arg(auth_val);
    }

    cmd.arg("clone")
        .arg("--depth=1")
        .arg("--no-recurse-submodules")
        .arg(&safe_url)
        .arg(dest_dir);

    let output = tokio::time::timeout(Duration::from_secs(GIT_TIMEOUT_SECS), cmd.output())
        .await
        .map_err(|_| format!("Git clone timed out after {GIT_TIMEOUT_SECS}s"))?
        .map_err(|e| format!("Failed to spawn git: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("401")
            || stderr.contains("403")
            || stderr.contains("Authentication failed")
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
struct ScopedTempDir(PathBuf);

impl ScopedTempDir {
    fn create(prefix: &str) -> Result<Self, String> {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScopedTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn apply_synced_bundle_to_plugin(
    pool: &SqlitePool,
    workspace_root: &Path,
    plugin_row: &SkillPluginRow,
    bundle: StagedPluginBundle,
    upstream_commit_sha: Option<&str>,
) -> Result<SkillPluginRow, String> {
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
            validate_safe_segment("plugin_id", &plugin_row.id)?;
            validate_safe_segment("candidate_version", &candidate_version)?;
            let src_dir = ScopedTempDir::create("cowok-stage")?;
            for sc in &bundle.scripts {
                let rel_trimmed = sc.rel_path.strip_prefix("scripts/").unwrap_or(&sc.rel_path);
                let mut rel_clean = PathBuf::new();
                for comp in Path::new(rel_trimmed).components() {
                    if let std::path::Component::Normal(seg) = comp {
                        let s = seg.to_string_lossy();
                        validate_safe_segment("script_segment", &s)?;
                        rel_clean.push(seg);
                    } else {
                        return Err(format!("Unsafe staged script path '{}'", sc.rel_path));
                    }
                }
                let dst_script = src_dir.path().join(rel_clean);
                if let Some(p) = dst_script.parent() {
                    std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                std::fs::write(dst_script, &sc.content).map_err(|e| e.to_string())?;
            }
            let version_dir = workspace_root
                .join(".cowok-design")
                .join("plugin-staging")
                .join(&plugin_row.id)
                .join(&candidate_version);
            if version_dir.exists() {
                let _ = std::fs::remove_dir_all(&version_dir);
            }
            std::fs::create_dir_all(&version_dir).map_err(|e| e.to_string())?;
            let target_folder = bundle.manifest.name.clone();
            let receipt = SkillRepackager::repackage_directory_with_manifest(
                src_dir.path(),
                &version_dir,
                &target_folder,
                bundle.manifest.clone(),
                None,
                plugin_row.upstream_repo_url.as_deref(),
            )?;

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
            return Ok(plugin_row.clone());
        }
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
                    if !refresh_cron_lease(&hb_pool, &hb_plugin, hb_fence, Utc::now().timestamp_millis(), DEFAULT_LEASE_TTL_MS).unwrap_or(false) {
                        break;
                    }
                }
            }
        }
    });

    // Connection dropped before external network/git I/O.
    let vault = VaultStore::from_workspace_root(workspace_root);
    let github_tok = vault.get_github_sync_token().ok().flatten();
    let mut probe = probe_upstream_with_ssrf_redirects(
        &claimed.upstream_repo_url,
        claimed.last_etag.as_deref(),
        github_tok.as_deref().map(|z| z.as_str()),
    )
    .await;

    if let UpstreamCheckResult::HeadCommit {
        commit_sha: Some(ref new_sha),
        ..
    } = probe
    {
        if claimed.last_commit_sha.as_deref() != Some(new_sha.as_str()) {
            if let Ok(clone_tmp) = ScopedTempDir::create("cowok-cron-clone") {
                match clone_repo_hardened(
                    &claimed.upstream_repo_url,
                    clone_tmp.path(),
                    github_tok.as_deref().map(|z| z.as_str()),
                )
                .await
                {
                    Ok(cloned_sha) => {
                        if let Ok(bundle) = load_bundle_from_dir_verified(clone_tmp.path(), None) {
                            if let Ok(plugin_row) = plugin_exists_for_scope(
                                pool,
                                tenant_id,
                                Some(project_id),
                                plugin_id,
                            ) {
                                let _ = apply_synced_bundle_to_plugin(
                                    pool,
                                    workspace_root,
                                    &plugin_row,
                                    bundle,
                                    Some(&cloned_sha),
                                );
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
                        if claimed.upstream_repo_url.ends_with(".git") {
                            probe = UpstreamCheckResult::RateLimited {
                                status: 502,
                                retry_after_secs: None,
                                detail: e,
                            };
                        }
                    }
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
            if next_failures >= MAX_CONSECUTIVE_CRON_FAILURES {
                (
                    "FAILED",
                    claimed.last_etag.clone(),
                    claimed.last_commit_sha.clone(),
                    next_failures,
                    None,
                    Some(format!("HTTP_{status}_MAX_RETRIES")),
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

    let committed = commit_cron_fenced(
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
    )
    .map_err(|e| e.to_string())?;

    if !committed {
        return Err("CRON_ZOMBIE_FENCE_REJECTED".to_string());
    }

    let conn = pool.connection().map_err(|e| e.to_string())?;
    spec_registry::plugin_store::get_cron_schedule_scoped(&conn, tenant_id, project_id, plugin_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Schedule missing after fenced commit".to_string())
}

/// Spawns daemon startup `LeaseReaper`, `tokio-cron-scheduler`, and periodic lease cleanup loop.
pub async fn spawn_plugin_cron_runtime(
    pool: SqlitePool,
    workspace_root: PathBuf,
    sse_tx: Option<tokio::sync::broadcast::Sender<String>>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    // Run startup orphan reaper immediately before scheduling ticks
    let _ = reap_stale_cron_leases(&pool, Utc::now().timestamp_millis());

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
                let _ = reap_stale_cron_leases(&p, now_ms);
                let schedules = {
                    let Ok(conn) = p.connection() else { return };
                    list_all_cron_schedules(&conn).unwrap_or_default()
                };
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
                    if run_plugin_sync_cycle(
                        &p,
                        &ws,
                        &sched_row.tenant_id,
                        &sched_row.project_id,
                        &sched_row.plugin_id,
                        false,
                    )
                    .await
                    .is_ok()
                    {
                        if let Some(ref tx) = sse {
                            let _ = tx.send(sched_row.project_id.clone());
                        }
                    }
                }
            }))
        }) {
            let _ = sched.add(job).await;
            let _ = sched.start().await;
            while !*shutdown_rx.borrow() {
                if shutdown_rx.changed().await.is_err() {
                    break;
                }
            }
            return;
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
                let _ = reap_stale_cron_leases(&pool, Utc::now().timestamp_millis());
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
