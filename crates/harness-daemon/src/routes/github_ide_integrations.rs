// @story 05.3
// @implements GitHub Multi-Account Vault Custody, Git Remote Origin Binding & IDE Readiness Status
use std::path::Path;
use std::time::Duration;

use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon_auth::VerifiedLegacyIngress;
use crate::routes::doctor_vault::{resolve_and_pin_ssrf_addr, resolve_cli_binary_path_pub};
use crate::routes::legacy_proposals::{
    json_err_resp, resolve_tenant_id, run_bounded_cmd_with_pgid,
};
use crate::routes::legacy_sessions::map_error_to_response;
use crate::routes::legacy_specs::verify_anti_idor_with_caller;
use crate::routes::DaemonAppState;
use crate::vault::VaultStore;

const GITHUB_ACCOUNTS_VAULT_KEY: &str = "github_accounts_json";
const ACTIVE_GITHUB_VAULT_KEY: &str = "github";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubAccountEntry {
    pub login: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub source: String,
    pub masked_hint: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeReadinessItem {
    pub id: String,
    pub label: String,
    pub command: String,
    pub launch_scheme: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubIntegrationSummary {
    pub connected: bool,
    pub active_login: Option<String>,
    pub active_source: Option<String>,
    pub accounts: Vec<GitHubAccountEntry>,
    pub gh_cli_installed: bool,
    pub gh_cli_authenticated: bool,
    pub remote_url: Option<String>,
    pub ready_for_pr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIntegrationsStatusDto {
    pub github: GitHubIntegrationSummary,
    pub ides: Vec<IdeReadinessItem>,
    pub preferred_ide: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectGitHubRequest {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub use_gh_cli: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectGitHubRequest {
    pub login: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindGitRemoteRequest {
    pub remote_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDefaultIdeRequest {
    pub ide: String,
}

#[derive(Debug, Deserialize)]
struct GitHubApiUser {
    login: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
}

fn parse_uuid(raw: &str) -> Result<Uuid, Response> {
    Uuid::parse_str(raw.trim()).map_err(|_| {
        json_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            &format!("Invalid project UUID '{raw}'"),
        )
    })
}

fn mask_token(tok: &str) -> String {
    let t = tok.trim();
    if t.len() <= 8 {
        return "****".to_string();
    }
    format!("{}••••{}", &t[..4], &t[t.len() - 4..])
}

const ALLOWED_IDES: &[&str] = &["antigravity", "cursor", "vscode", "claude-code"];

fn read_ide_pref(ws_root: &Path) -> String {
    let pref_path = ws_root.join(".cowok-design").join("ide-preferences.json");
    if let Ok(meta) = std::fs::symlink_metadata(&pref_path) {
        if meta.file_type().is_symlink() {
            return "antigravity".to_string();
        }
    }
    if let Ok(raw) = std::fs::read_to_string(&pref_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(ide) = val.get("preferredIde").and_then(|v| v.as_str()) {
                if ALLOWED_IDES.contains(&ide) {
                    return ide.to_string();
                }
            }
        }
    }
    "antigravity".to_string()
}

fn write_ide_pref(ws_root: &Path, ide: &str) -> Result<(), Response> {
    let dir = ws_root.join(".cowok-design");
    std::fs::create_dir_all(&dir).map_err(|e| {
        json_err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "IO_ERROR",
            &e.to_string(),
        )
    })?;
    let pref_path = dir.join("ide-preferences.json");
    if let Ok(meta) = std::fs::symlink_metadata(&pref_path) {
        if meta.file_type().is_symlink() {
            return Err(json_err_resp(
                StatusCode::BAD_REQUEST,
                "SYMLINK_REJECTED",
                "ide-preferences.json symlink target rejected",
            ));
        }
    }
    let payload = serde_json::json!({ "preferredIde": ide });
    std::fs::write(&pref_path, payload.to_string()).map_err(|e| {
        json_err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "IO_ERROR",
            &e.to_string(),
        )
    })
}

fn detect_ides() -> Vec<IdeReadinessItem> {
    let catalog = [
        ("antigravity", "Antigravity", "agy", "antigravity://"),
        ("cursor", "Cursor", "cursor", "cursor://"),
        ("vscode", "VS Code", "code", "vscode://"),
        ("claude-code", "Claude Code", "claude", "claude-code://"),
    ];
    catalog
        .into_iter()
        .map(|(id, label, cmd, scheme)| {
            let installed = resolve_cli_binary_path_pub(cmd).is_some()
                || (id == "antigravity" && resolve_cli_binary_path_pub("antigravity").is_some());
            IdeReadinessItem {
                id: id.to_string(),
                label: label.to_string(),
                command: cmd.to_string(),
                launch_scheme: scheme.to_string(),
                installed,
            }
        })
        .collect()
}

fn read_vault_secret<'a>(
    secrets: &'a std::collections::BTreeMap<String, String>,
    tenant_id: &str,
    provider_id: &str,
) -> Option<&'a String> {
    let scoped = VaultStore::scoped_key(tenant_id, provider_id);
    secrets.get(&scoped).or_else(|| {
        if tenant_id.trim() == "local-default" {
            secrets.get(provider_id)
        } else {
            None
        }
    })
}

fn read_vault_accounts(vault: &VaultStore, tenant_id: &str) -> Vec<GitHubAccountEntry> {
    let secrets = vault.unseal_secrets(None).unwrap_or_default();
    let Some(raw) = read_vault_secret(&secrets, tenant_id, GITHUB_ACCOUNTS_VAULT_KEY) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<GitHubAccountEntry>>(raw).unwrap_or_default()
}

fn write_vault_accounts(
    vault: &VaultStore,
    tenant_id: &str,
    accounts: &[GitHubAccountEntry],
) -> Result<(), Response> {
    let serialized = serde_json::to_string(accounts).map_err(|e| {
        json_err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "VAULT_ERR",
            &e.to_string(),
        )
    })?;
    vault
        .upsert_secret_scoped(tenant_id, GITHUB_ACCOUNTS_VAULT_KEY, &serialized, None)
        .map_err(|e| json_err_resp(StatusCode::INTERNAL_SERVER_ERROR, "VAULT_ERR", &e))?;
    Ok(())
}

fn inspect_project_status(
    daemon_ws_root: &Path,
    project_root: &Path,
    tenant_id: &str,
) -> ProjectIntegrationsStatusDto {
    let vault = VaultStore::from_workspace_root(daemon_ws_root);
    let secrets = vault.unseal_secrets(None).unwrap_or_default();
    let active_tok = read_vault_secret(&secrets, tenant_id, ACTIVE_GITHUB_VAULT_KEY)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let mut accounts = read_vault_accounts(&vault, tenant_id);
    if accounts.is_empty() {
        if let Some(ref tok) = active_tok {
            accounts.push(GitHubAccountEntry {
                login: "github-vault-token".to_string(),
                display_name: "GitHub Vault Token".to_string(),
                avatar_url: None,
                source: "vault_pat".to_string(),
                masked_hint: mask_token(tok),
                active: true,
            });
        }
    }
    let active_entry = accounts
        .iter()
        .find(|a| a.active)
        .cloned()
        .or_else(|| accounts.first().cloned());
    let remote_out = run_bounded_cmd_with_pgid(
        "git",
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "remote",
            "get-url",
            "origin",
        ],
        project_root,
        &[],
    );
    let remote_url = if remote_out.success && !remote_out.stdout.trim().is_empty() {
        Some(remote_out.stdout.trim().to_string())
    } else {
        None
    };
    let gh_ver = run_bounded_cmd_with_pgid("gh", &["--version"], project_root, &[]);
    let gh_cli_installed = !gh_ver.not_found && gh_ver.success;
    let gh_cli_authenticated = if gh_cli_installed {
        run_bounded_cmd_with_pgid("gh", &["auth", "status"], project_root, &[]).success
    } else {
        false
    };
    let connected = active_tok.is_some() || !accounts.is_empty();
    let ready_for_pr = connected && remote_url.is_some() && gh_cli_installed;
    ProjectIntegrationsStatusDto {
        github: GitHubIntegrationSummary {
            connected,
            active_login: active_entry.as_ref().map(|a| a.login.clone()),
            active_source: active_entry.map(|a| a.source),
            accounts,
            gh_cli_installed,
            gh_cli_authenticated,
            remote_url,
            ready_for_pr,
        },
        ides: detect_ides(),
        preferred_ide: read_ide_pref(daemon_ws_root),
    }
}

async fn verify_github_token_live(token: &str) -> Result<GitHubApiUser, Response> {
    let target = "https://api.github.com/user";
    let (host, addr) = resolve_and_pin_ssrf_addr(target)
        .await
        .map_err(|reason| json_err_resp(StatusCode::BAD_GATEWAY, "GH_SSRF_BLOCKED", &reason))?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(&host, addr)
        .timeout(Duration::from_millis(4000))
        .build()
        .map_err(|e| json_err_resp(StatusCode::BAD_GATEWAY, "GH_CLIENT_ERR", &e.to_string()))?;
    let resp = client
        .get(target)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "cowok-design-daemon")
        .send()
        .await
        .map_err(|e| {
            json_err_resp(
                StatusCode::BAD_GATEWAY,
                "GH_API_UNREACHABLE",
                &format!("Cannot reach api.github.com: {e}"),
            )
        })?;
    if !resp.status().is_success() {
        return Err(json_err_resp(
            StatusCode::UNAUTHORIZED,
            "GH_TOKEN_INVALID",
            &format!("GitHub rejected token with HTTP {}", resp.status().as_u16()),
        ));
    }
    resp.json::<GitHubApiUser>()
        .await
        .map_err(|e| json_err_resp(StatusCode::BAD_GATEWAY, "GH_API_DECODE", &e.to_string()))
}

fn validate_remote_url_safe(raw: &str) -> Result<String, Response> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('-')
        || trimmed.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(json_err_resp(
            StatusCode::BAD_REQUEST,
            "INVALID_REMOTE_URL",
            "Remote URL must be a valid https:// or git@... URL without spaces or leading '-'",
        ));
    }
    if !(trimmed.starts_with("https://") || trimmed.starts_with("git@")) {
        return Err(json_err_resp(
            StatusCode::BAD_REQUEST,
            "INVALID_REMOTE_URL",
            "Only https:// and git@ remotes are permitted",
        ));
    }
    if let Some(after_at) = trimmed.strip_prefix("git@") {
        if after_at.starts_with('-') {
            return Err(json_err_resp(
                StatusCode::BAD_REQUEST,
                "INVALID_REMOTE_URL",
                "SSH host in git@ remote must not start with '-'",
            ));
        }
    }
    Ok(trimmed.to_string())
}

pub(crate) async fn handle_get_integrations_status(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<ProjectIntegrationsStatusDto>, Response> {
    let project_id = parse_uuid(&id)?;
    let (_, canon_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let ws_root = state.registry.pool().workspace_root().to_path_buf();
    let dto = tokio::task::spawn_blocking(move || {
        inspect_project_status(&ws_root, &canon_root, &tenant_id)
    })
    .await
    .map_err(|e| {
        json_err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "TASK_ERR",
            &e.to_string(),
        )
    })?;
    Ok(Json(dto))
}

pub(crate) async fn handle_connect_github_account(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ConnectGitHubRequest>,
) -> Result<Json<ProjectIntegrationsStatusDto>, Response> {
    let project_id = parse_uuid(&id)?;
    let (_, canon_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let ws_root = state.registry.pool().workspace_root().to_path_buf();
    let token = if req.use_gh_cli {
        let out = tokio::task::spawn_blocking({
            let root = canon_root.clone();
            move || run_bounded_cmd_with_pgid("gh", &["auth", "token"], &root, &[])
        })
        .await
        .map_err(|e| {
            json_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TASK_ERR",
                &e.to_string(),
            )
        })?;
        if !out.success || out.stdout.trim().is_empty() {
            return Err(json_err_resp(
                StatusCode::UNAUTHORIZED,
                "GH_CLI_NOT_LOGGED_IN",
                "Run 'gh auth login' in terminal or paste a GitHub Personal Access Token.",
            ));
        }
        out.stdout.trim().to_string()
    } else {
        req.token.unwrap_or_default().trim().to_string()
    };
    if token.len() < 16 {
        return Err(json_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "GitHub token must be at least 16 characters.",
        ));
    }
    let user = verify_github_token_live(&token).await?;
    let source = if req.use_gh_cli {
        "gh_cli"
    } else {
        "vault_pat"
    }
    .to_string();
    let dto =
        tokio::task::spawn_blocking(move || -> Result<ProjectIntegrationsStatusDto, Response> {
            let vault = VaultStore::from_workspace_root(&ws_root);
            let acct_key = format!("github:{}", user.login);
            vault
                .upsert_secret_scoped(&tenant_id, &acct_key, &token, None)
                .map_err(|e| json_err_resp(StatusCode::INTERNAL_SERVER_ERROR, "VAULT_ERR", &e))?;
            vault
                .upsert_secret_scoped(&tenant_id, ACTIVE_GITHUB_VAULT_KEY, &token, None)
                .map_err(|e| json_err_resp(StatusCode::INTERNAL_SERVER_ERROR, "VAULT_ERR", &e))?;
            let mut accounts = read_vault_accounts(&vault, &tenant_id);
            for a in &mut accounts {
                a.active = false;
            }
            accounts.retain(|a| a.login != user.login);
            accounts.insert(
                0,
                GitHubAccountEntry {
                    login: user.login.clone(),
                    display_name: user.name.unwrap_or(user.login),
                    avatar_url: user.avatar_url,
                    source,
                    masked_hint: mask_token(&token),
                    active: true,
                },
            );
            write_vault_accounts(&vault, &tenant_id, &accounts)?;
            Ok(inspect_project_status(&ws_root, &canon_root, &tenant_id))
        })
        .await
        .map_err(|e| {
            json_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TASK_ERR",
                &e.to_string(),
            )
        })??;
    Ok(Json(dto))
}

pub(crate) async fn handle_select_github_account(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<SelectGitHubRequest>,
) -> Result<Json<ProjectIntegrationsStatusDto>, Response> {
    let project_id = parse_uuid(&id)?;
    let (_, canon_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let ws_root = state.registry.pool().workspace_root().to_path_buf();
    let target_login = req.login.trim().to_string();
    let dto =
        tokio::task::spawn_blocking(move || -> Result<ProjectIntegrationsStatusDto, Response> {
            let vault = VaultStore::from_workspace_root(&ws_root);
            let secrets = vault.unseal_secrets(None).unwrap_or_default();
            let key = format!("github:{target_login}");
            let tok = read_vault_secret(&secrets, &tenant_id, &key).ok_or_else(|| {
                json_err_resp(
                    StatusCode::NOT_FOUND,
                    "ACCOUNT_NOT_FOUND",
                    "GitHub account not found in Vault",
                )
            })?;
            vault
                .upsert_secret_scoped(&tenant_id, ACTIVE_GITHUB_VAULT_KEY, tok, None)
                .map_err(|e| json_err_resp(StatusCode::INTERNAL_SERVER_ERROR, "VAULT_ERR", &e))?;
            let mut accounts = read_vault_accounts(&vault, &tenant_id);
            for a in &mut accounts {
                a.active = a.login == target_login;
            }
            write_vault_accounts(&vault, &tenant_id, &accounts)?;
            Ok(inspect_project_status(&ws_root, &canon_root, &tenant_id))
        })
        .await
        .map_err(|e| {
            json_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TASK_ERR",
                &e.to_string(),
            )
        })??;
    Ok(Json(dto))
}

pub(crate) async fn handle_bind_git_remote(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<BindGitRemoteRequest>,
) -> Result<Json<ProjectIntegrationsStatusDto>, Response> {
    let project_id = parse_uuid(&id)?;
    let (_, canon_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let remote = validate_remote_url_safe(&req.remote_url)?;
    let ws_root = state.registry.pool().workspace_root().to_path_buf();
    let dto =
        tokio::task::spawn_blocking(move || -> Result<ProjectIntegrationsStatusDto, Response> {
            if !canon_root.join(".git").exists() {
                let init_out = run_bounded_cmd_with_pgid(
                    "git",
                    &["-c", "core.hooksPath=/dev/null", "init"],
                    &canon_root,
                    &[],
                );
                if !init_out.success {
                    return Err(json_err_resp(
                        StatusCode::BAD_GATEWAY,
                        "GIT_INIT_FAILED",
                        &init_out.stderr,
                    ));
                }
            }
            let check_origin = run_bounded_cmd_with_pgid(
                "git",
                &[
                    "-c",
                    "core.hooksPath=/dev/null",
                    "remote",
                    "get-url",
                    "origin",
                ],
                &canon_root,
                &[],
            );
            let sub_args = if check_origin.success {
                vec![
                    "-c",
                    "core.hooksPath=/dev/null",
                    "remote",
                    "set-url",
                    "origin",
                    &remote,
                ]
            } else {
                vec![
                    "-c",
                    "core.hooksPath=/dev/null",
                    "remote",
                    "add",
                    "origin",
                    &remote,
                ]
            };
            let set_out = run_bounded_cmd_with_pgid("git", &sub_args, &canon_root, &[]);
            if !set_out.success {
                return Err(json_err_resp(
                    StatusCode::BAD_GATEWAY,
                    "GIT_REMOTE_FAILED",
                    &set_out.stderr,
                ));
            }
            Ok(inspect_project_status(&ws_root, &canon_root, &tenant_id))
        })
        .await
        .map_err(|e| {
            json_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TASK_ERR",
                &e.to_string(),
            )
        })??;
    Ok(Json(dto))
}

pub(crate) async fn handle_set_default_ide(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<SetDefaultIdeRequest>,
) -> Result<Json<ProjectIntegrationsStatusDto>, Response> {
    let project_id = parse_uuid(&id)?;
    let (_, canon_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let ide = req.ide.trim().to_ascii_lowercase();
    if !matches!(
        ide.as_str(),
        "antigravity" | "cursor" | "vscode" | "claude-code"
    ) {
        return Err(json_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid IDE target",
        ));
    }
    let ws_root = state.registry.pool().workspace_root().to_path_buf();
    let dto =
        tokio::task::spawn_blocking(move || -> Result<ProjectIntegrationsStatusDto, Response> {
            write_ide_pref(&ws_root, &ide)?;
            Ok(inspect_project_status(&ws_root, &canon_root, &tenant_id))
        })
        .await
        .map_err(|e| {
            json_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TASK_ERR",
                &e.to_string(),
            )
        })??;
    Ok(Json(dto))
}

pub(crate) fn integration_routes() -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/api/v1/projects/:id/integrations/status",
            get(handle_get_integrations_status),
        )
        .route(
            "/api/v1/projects/:id/integrations/github/connect",
            post(handle_connect_github_account),
        )
        .route(
            "/api/v1/projects/:id/integrations/github/select",
            post(handle_select_github_account),
        )
        .route(
            "/api/v1/projects/:id/integrations/git-remote",
            post(handle_bind_git_remote),
        )
        .route(
            "/api/v1/projects/:id/integrations/ide/default",
            post(handle_set_default_ide),
        )
}
