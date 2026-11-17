// @story 04.3
// @implements Gate H6 (CLI Doctor 20+ Tools, Live SSRF-Pinned BYOK Probe & 0o600 Encrypted Vault)
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use axum::{
    extract::{Extension, Path as AxumPath, Query, State},
    http::{StatusCode, Uri},
    response::Response,
    Json,
};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use spec_registry::{map_sqlite_error, retry_on_busy};
use uuid::Uuid;

use crate::daemon_auth::FlowPrincipal;
use crate::routes::doctor_usage::{enrich_cli_tool_dto, DoctorModelOptionDto};
use crate::routes::legacy_workbench_v6::{
    check_non_empty_id, check_ssrf_socket_addr, epoch_ms_to_iso, json_err, map_harness_resp,
    run_db, sanitize_credential_ref, validate_ssrf_safe_url,
};
use crate::routes::DaemonAppState;
use crate::vault::VaultStore;

// -----------------------------------------------------------------------------
// 1. Live Provider Key Verification (`POST /api/v1/doctor/verify-key`)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyKeyRequest {
    pub provider_id: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub endpoint_url: String,
    #[serde(default)]
    pub dry_run_format_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyKeyResponse {
    pub valid: bool,
    pub status: String,
    pub provider_id: String,
    pub message: String,
    pub credential_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_probed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
}

fn validate_key_syntax(
    provider: &str,
    key: &str,
    endpoint: &str,
) -> Result<&'static str, &'static str> {
    match provider {
        "anthropic" | "claude" => {
            if key.starts_with("sk-ant-") && key.len() >= 24 {
                Ok("Anthropic key format valid")
            } else {
                Err("Expected Anthropic key starting with 'sk-ant-' (min 24 chars)")
            }
        }
        "openai" => {
            if key.starts_with("sk-") && key.len() >= 20 {
                Ok("OpenAI key format valid")
            } else {
                Err("Expected OpenAI key starting with 'sk-' (min 20 chars)")
            }
        }
        "gemini" | "google" => {
            if key.starts_with("AIza") && key.len() >= 24 {
                Ok("Gemini key format valid")
            } else {
                Err("Expected Gemini key starting with 'AIza' (min 24 chars)")
            }
        }
        "deepseek" => {
            if key.starts_with("sk-") && key.len() >= 18 {
                Ok("DeepSeek key format valid")
            } else {
                Err("Expected DeepSeek key starting with 'sk-'")
            }
        }
        "openrouter" => {
            if key.starts_with("sk-or-") && key.len() >= 20 {
                Ok("OpenRouter key format valid")
            } else {
                Err("Expected OpenRouter key starting with 'sk-or-'")
            }
        }
        "ollama" | "byos" => {
            if provider == "byos" && endpoint.is_empty() {
                Err("Expected valid http://127.0.0.1:<port> or https:// endpointUrl")
            } else {
                Ok("Local/BYOS endpoint format valid")
            }
        }
        _ => {
            if key.len() >= 16 {
                Ok("Provider key format valid")
            } else {
                Err("API key must be at least 16 characters")
            }
        }
    }
}

fn canonical_probe_url(provider: &str, custom_endpoint: &str) -> Option<String> {
    if !custom_endpoint.is_empty() {
        let trimmed = custom_endpoint.trim_end_matches('/');
        if provider == "ollama" && !trimmed.ends_with("/api/tags") {
            return Some(format!("{trimmed}/api/tags"));
        }
        if matches!(provider, "openai" | "deepseek" | "openrouter" | "byos")
            && !trimmed.ends_with("/models")
        {
            return Some(format!("{trimmed}/models"));
        }
        return Some(trimmed.to_string());
    }
    match provider {
        "anthropic" | "claude" => Some("https://api.anthropic.com/v1/models".to_string()),
        "openai" => Some("https://api.openai.com/v1/models".to_string()),
        "gemini" | "google" => {
            Some("https://generativelanguage.googleapis.com/v1beta/models".to_string())
        }
        "deepseek" => Some("https://api.deepseek.com/models".to_string()),
        "openrouter" => Some("https://openrouter.ai/api/v1/models".to_string()),
        "ollama" => Some("http://127.0.0.1:11434/api/tags".to_string()),
        _ => None,
    }
}

async fn resolve_and_pin_ssrf_addr(target_url: &str) -> Result<(String, SocketAddr), String> {
    validate_ssrf_safe_url(target_url, true)?;
    let uri = target_url
        .parse::<Uri>()
        .map_err(|e| format!("Invalid target URL: {e}"))?;
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

    let mut pinned: Option<SocketAddr> = None;
    for sa in addrs {
        check_ssrf_socket_addr(sa, true)?;
        if pinned.is_none() {
            pinned = Some(sa);
        }
    }
    let chosen = pinned.ok_or_else(|| format!("DNS returned no addresses for '{host}'"))?;
    // Require TLS (https://) whenever BYOK probe sends API keys to any non-loopback address
    if !chosen.ip().is_loopback() && uri.scheme_str() != Some("https") {
        return Err(
            "Non-loopback provider endpointUrl must use https:// to protect secrets in transit"
                .to_string(),
        );
    }
    Ok((host, chosen))
}

async fn probe_provider_upstream(
    provider: &str,
    api_key: &str,
    target_url: &str,
) -> (bool, String, String, Option<u16>) {
    let (host, pinned_addr) = match resolve_and_pin_ssrf_addr(target_url).await {
        Ok(pair) => pair,
        Err(reason) => {
            return (
                false,
                "ssrf_or_dns_blocked".to_string(),
                format!("Upstream probe blocked before connect: {reason}"),
                None,
            );
        }
    };

    let client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(&host, pinned_addr)
        .timeout(Duration::from_millis(2800))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                false,
                "probe_client_error".to_string(),
                format!("Failed initializing HTTP probe client: {e}"),
                None,
            );
        }
    };

    let mut req = client.get(target_url);
    match provider {
        "anthropic" | "claude" => {
            req = req
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01");
        }
        "gemini" | "google" => {
            req = req.header("x-goog-api-key", api_key);
        }
        "ollama" => {}
        _ => {
            if !api_key.is_empty() {
                req = req.bearer_auth(api_key);
            }
        }
    }

    match req.send().await {
        Ok(resp) => {
            let code = resp.status().as_u16();
            if resp.status().is_success() {
                (
                    true,
                    "live_verified".to_string(),
                    format!("Upstream provider '{provider}' authenticated live (HTTP {code})"),
                    Some(code),
                )
            } else if code == 401 || code == 403 {
                (
                    false,
                    "unauthorized".to_string(),
                    format!("Upstream provider '{provider}' rejected credentials (HTTP {code})"),
                    Some(code),
                )
            } else if code == 429 {
                (
                    true,
                    "rate_limited".to_string(),
                    format!("Key accepted by '{provider}' gateway but rate-limited (HTTP 429)"),
                    Some(code),
                )
            } else {
                (
                    false,
                    "upstream_error".to_string(),
                    format!("Upstream provider '{provider}' returned unexpected HTTP {code}"),
                    Some(code),
                )
            }
        }
        Err(e) => (
            false,
            "upstream_unreachable".to_string(),
            format!("Live probe could not reach '{provider}': {e}"),
            None,
        ),
    }
}

pub async fn handle_verify_provider_key(
    Extension(_principal): Extension<FlowPrincipal>,
    Json(req): Json<VerifyKeyRequest>,
) -> Result<Json<VerifyKeyResponse>, Response> {
    let provider = req.provider_id.trim().to_ascii_lowercase();
    let key = req.api_key.trim();
    let endpoint = req.endpoint_url.trim();

    if provider.is_empty() {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "providerId is required",
        ));
    }

    if !endpoint.is_empty() {
        if let Err(e) = validate_ssrf_safe_url(endpoint, true) {
            return Ok(Json(VerifyKeyResponse {
                valid: false,
                status: "invalid_endpoint".to_string(),
                provider_id: provider,
                message: format!("SSRF validation rejected endpointUrl: {e}"),
                credential_ref: sanitize_credential_ref(key),
                live_probed: Some(false),
                http_status: None,
            }));
        }
        if let Ok(parsed_ep) = endpoint.parse::<Uri>() {
            let h = parsed_ep
                .host()
                .unwrap_or("")
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_ascii_lowercase();
            let is_local = h == "localhost"
                || h.ends_with(".localhost")
                || h.parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback());
            if !is_local && parsed_ep.scheme_str() != Some("https") {
                return Ok(Json(VerifyKeyResponse {
                    valid: false,
                    status: "invalid_endpoint".to_string(),
                    provider_id: provider,
                    message: "Non-loopback provider endpointUrl must use https:// to protect API keys in transit".to_string(),
                    credential_ref: sanitize_credential_ref(key),
                    live_probed: Some(false),
                    http_status: None,
                }));
            }
        }
    }

    let syntax_ok = match validate_key_syntax(&provider, key, endpoint) {
        Ok(msg) => msg,
        Err(err_msg) => {
            let st = if matches!(provider.as_str(), "ollama" | "byos") {
                "invalid_endpoint"
            } else {
                "invalid_key"
            };
            return Ok(Json(VerifyKeyResponse {
                valid: false,
                status: st.to_string(),
                provider_id: provider,
                message: err_msg.to_string(),
                credential_ref: sanitize_credential_ref(key),
                live_probed: Some(false),
                http_status: None,
            }));
        }
    };

    if req.dry_run_format_only {
        return Ok(Json(VerifyKeyResponse {
            valid: true,
            status: "format_checked".to_string(),
            provider_id: provider,
            message: format!(
                "{syntax_ok} (offline format check only — live upstream probe skipped)"
            ),
            credential_ref: sanitize_credential_ref(key),
            live_probed: Some(false),
            http_status: None,
        }));
    }

    let Some(target_url) = canonical_probe_url(&provider, endpoint) else {
        return Ok(Json(VerifyKeyResponse {
            valid: true,
            status: "format_checked".to_string(),
            provider_id: provider,
            message: format!("{syntax_ok}; no endpointUrl provided for custom provider live probe"),
            credential_ref: sanitize_credential_ref(key),
            live_probed: Some(false),
            http_status: None,
        }));
    };

    let (valid, status, message, http_status) =
        probe_provider_upstream(&provider, key, &target_url).await;

    Ok(Json(VerifyKeyResponse {
        valid,
        status,
        provider_id: provider,
        message,
        credential_ref: sanitize_credential_ref(key),
        live_probed: Some(true),
        http_status,
    }))
}

// -----------------------------------------------------------------------------
// 2. CLI Doctor Matrix (`GET /api/v1/doctor/cli-matrix`) — 20 CLIs + Safe Probes
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorMatrixQuery {
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliDoctorToolDto {
    pub id: String,
    pub name: String,
    pub command: String,
    pub category: String,  // "cli" | "ide" | "local"
    pub scan_mode: String, // "passive" | "active"
    pub installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub authenticated: bool,
    pub status: String, // "ready" | "not_installed" | "auth_needed" | "error"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_reason: Option<String>,
    pub latency_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_group: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<DoctorModelOptionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCliMatrixResponse {
    pub tools: Vec<CliDoctorToolDto>,
    pub clis: Vec<CliDoctorToolDto>,
    pub scan_mode: String,
    pub scanned_at: String,
}

#[derive(Clone, Copy)]
pub struct CliTargetDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
    pub category: &'static str,
    pub env_keys: &'static [&'static str],
    pub config_subpaths: &'static [&'static str],
}

pub const SUPPORTED_CLI_DEFINITIONS_PUB: &[CliTargetDefinition] = SUPPORTED_CLI_DEFINITIONS;

const SUPPORTED_CLI_DEFINITIONS: &[CliTargetDefinition] = &[
    CliTargetDefinition {
        id: "claude",
        name: "Claude Code",
        command: "claude",
        category: "cli",
        env_keys: &["ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN"],
        config_subpaths: &[".claude/.credentials.json", ".claude.json"],
    },
    CliTargetDefinition {
        id: "codex",
        name: "OpenAI Codex CLI",
        command: "codex",
        category: "cli",
        env_keys: &["OPENAI_API_KEY", "CODEX_API_KEY"],
        config_subpaths: &[".codex/auth.json"],
    },
    CliTargetDefinition {
        id: "agy",
        name: "Google Antigravity CLI",
        command: "agy",
        category: "ide",
        env_keys: &["GEMINI_API_KEY", "GOOGLE_API_KEY", "ANTIGRAVITY_API_KEY"],
        config_subpaths: &[
            ".gemini/oauth_creds.json",
            ".gemini/google_accounts.json",
            ".gemini/antigravity/mcp_config.json",
        ],
    },
    CliTargetDefinition {
        id: "gemini",
        name: "Gemini CLI",
        command: "gemini",
        category: "cli",
        env_keys: &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        config_subpaths: &[
            ".gemini/oauth_creds.json",
            ".gemini/google_accounts.json",
            ".config/gemini/oauth_creds.json",
        ],
    },
    CliTargetDefinition {
        id: "cursor",
        name: "Cursor Agent CLI",
        command: "cursor",
        category: "ide",
        env_keys: &["CURSOR_API_KEY", "OPENAI_API_KEY"],
        config_subpaths: &[".cursor/cli-config.json"],
    },
    CliTargetDefinition {
        id: "windsurf",
        name: "Windsurf CLI",
        command: "windsurf",
        category: "ide",
        env_keys: &["WINDSURF_API_KEY", "CODEIUM_API_KEY"],
        config_subpaths: &[".codeium/windsurf/mcp_config.json"],
    },
    CliTargetDefinition {
        id: "trae",
        name: "Trae IDE CLI",
        command: "trae",
        category: "ide",
        env_keys: &["TRAE_API_KEY"],
        config_subpaths: &[".trae/user_config.json"],
    },
    CliTargetDefinition {
        id: "opencode",
        name: "OpenCode CLI",
        command: "opencode",
        category: "cli",
        env_keys: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OPENCODE_API_KEY"],
        config_subpaths: &[
            ".local/share/opencode/auth.json",
            ".config/opencode/opencode.jsonc",
        ],
    },
    CliTargetDefinition {
        id: "pi",
        name: "Pi Coding Agent",
        command: "pi",
        category: "cli",
        env_keys: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "PI_API_KEY"],
        config_subpaths: &[".pi/agent/auth.json", ".pi/agent/settings.json"],
    },
    CliTargetDefinition {
        id: "omp",
        name: "Oh My Prompt CLI",
        command: "omp",
        category: "cli",
        env_keys: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GEMINI_API_KEY"],
        config_subpaths: &[".omp/config.json", ".omp/auth.json"],
    },
    CliTargetDefinition {
        id: "ollama",
        name: "Ollama Local LLM",
        command: "ollama",
        category: "local",
        env_keys: &["OLLAMA_API_KEY"],
        config_subpaths: &[".ollama/id_ed25519"],
    },
    CliTargetDefinition {
        id: "aider",
        name: "Aider CLI",
        command: "aider",
        category: "cli",
        env_keys: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"],
        config_subpaths: &[".aider.conf.yml"],
    },
    CliTargetDefinition {
        id: "goose",
        name: "Block Goose CLI",
        command: "goose",
        category: "cli",
        env_keys: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "DATABRICKS_HOST"],
        config_subpaths: &[".config/goose/config.yaml", ".config/goose/profiles.yaml"],
    },
    CliTargetDefinition {
        id: "qwen",
        name: "Qwen Code CLI",
        command: "qwen",
        category: "cli",
        env_keys: &["QWEN_API_KEY", "DASHSCOPE_API_KEY", "OPENAI_API_KEY"],
        config_subpaths: &[".qwen/oauth_creds.json", ".qwen/settings.json"],
    },
    CliTargetDefinition {
        id: "cline",
        name: "Cline CLI",
        command: "cline",
        category: "ide",
        env_keys: &["CLINE_API_KEY", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"],
        config_subpaths: &[".cline/config.json"],
    },
    CliTargetDefinition {
        id: "roo",
        name: "Roo Code CLI",
        command: "roo",
        category: "ide",
        env_keys: &["ROO_API_KEY", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"],
        config_subpaths: &[".roo/config.json"],
    },
    CliTargetDefinition {
        id: "continue",
        name: "Continue Dev CLI",
        command: "cn",
        category: "cli",
        env_keys: &["CONTINUE_API_KEY", "OPENAI_API_KEY", "ANTHROPIC_API_KEY"],
        config_subpaths: &[".continue/config.json", ".continue/config.yaml"],
    },
    CliTargetDefinition {
        id: "copilot",
        name: "GitHub Copilot CLI",
        command: "copilot",
        category: "cli",
        env_keys: &["GITHUB_TOKEN", "GH_TOKEN"],
        config_subpaths: &[".config/github-copilot/hosts.json", ".config/gh/hosts.yml"],
    },
    CliTargetDefinition {
        id: "kilo",
        name: "Kilo Code CLI",
        command: "kilo",
        category: "cli",
        env_keys: &["KILOCODE_API_KEY", "OPENROUTER_API_KEY"],
        config_subpaths: &[".kilocode/config.json"],
    },
    CliTargetDefinition {
        id: "amp",
        name: "Sourcegraph Amp CLI",
        command: "amp",
        category: "cli",
        env_keys: &["AMP_API_KEY", "SRC_ACCESS_TOKEN"],
        config_subpaths: &[".config/amp/secrets.json", ".local/share/amp/secrets.json"],
    },
    CliTargetDefinition {
        id: "kiro",
        name: "Kiro CLI",
        command: "kiro-cli",
        category: "ide",
        env_keys: &["AWS_ACCESS_KEY_ID", "KIRO_API_KEY"],
        config_subpaths: &[".kiro/settings.json"],
    },
];

pub fn resolve_cli_binary_path_pub(cmd: &str) -> Option<std::path::PathBuf> {
    resolve_cli_binary_path(cmd)
}

fn resolve_cli_binary_path(cmd: &str) -> Option<std::path::PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(cmd);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let mut standard_dirs = vec![
        std::path::PathBuf::from("/usr/local/bin"),
        std::path::PathBuf::from("/opt/homebrew/bin"),
        std::path::PathBuf::from("/usr/bin"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        let h = std::path::Path::new(&home);
        standard_dirs.push(h.join(".local/bin"));
        standard_dirs.push(h.join(".npm-global/bin"));
        standard_dirs.push(h.join(".cargo/bin"));
    }
    for dir in standard_dirs {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn validate_config_file_non_empty(path: &Path) -> bool {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "{}" || trimmed == "[]" {
        return false;
    }
    if path.extension().and_then(|e| e.to_str()) == Some("json") {
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(serde_json::Value::Object(map)) => !map.is_empty(),
            Ok(serde_json::Value::Array(arr)) => !arr.is_empty(),
            _ => false,
        }
    } else {
        trimmed.len() >= 4
    }
}

fn check_cli_passive_auth(def: &CliTargetDefinition) -> (bool, Option<String>) {
    for key in def.env_keys {
        if let Ok(val) = std::env::var(key) {
            if !val.trim().is_empty() {
                return (true, None);
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let home_path = Path::new(&home);
        for subpath in def.config_subpaths {
            let p = home_path.join(subpath);
            if p.is_file() && validate_config_file_non_empty(&p) {
                return (true, None);
            }
        }
    }

    let default_key = def.env_keys.first().copied().unwrap_or("API_KEY");
    (
        false,
        Some(format!(
            "Missing {default_key} or valid CLI credential config"
        )),
    )
}

async fn check_ollama_live_health() -> (bool, Option<String>, Option<String>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(350))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return (false, Some("Ollama client init failed".to_string()), None);
        }
    };
    match client.get("http://127.0.0.1:11434/api/tags").send().await {
        Ok(resp) if resp.status().is_success() => (true, None, Some("daemon:11434".to_string())),
        Ok(resp) => (
            false,
            Some(format!("Ollama returned HTTP {}", resp.status().as_u16())),
            None,
        ),
        Err(_) => (
            false,
            Some("Ollama daemon not running on 127.0.0.1:11434".to_string()),
            None,
        ),
    }
}

const MAX_PROBE_CONCURRENCY: usize = 4;
const MAX_PROBE_TIMEOUT_MS: u64 = 1500;
static ACTIVE_PROBE_SEMAPHORE: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(MAX_PROBE_CONCURRENCY);

fn ide_extension_installed(id: &str, config_subpaths: &[&str]) -> bool {
    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
        return false;
    };
    let prefix = match id {
        "cline" => Some("saoudrizwan.claude-dev"),
        "roo" => Some("rooveterinaryinc.roo-cline"),
        "continue" => Some("continue.continue"),
        _ => None,
    };
    let in_vscode = prefix.is_some_and(|pfx| {
        std::fs::read_dir(home.join(".vscode/extensions"))
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(pfx))
    });
    in_vscode || config_subpaths.iter().any(|rel| home.join(rel).is_file())
}

fn spawn_bounded_pipe_drain<R: std::io::Read + Send + 'static>(
    mut reader: R,
) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            if out.len() < 8192 {
                let take = (8192 - out.len()).min(n);
                out.extend_from_slice(&buf[..take]);
            }
        }
        String::from_utf8_lossy(&out).to_string()
    })
}

fn first_version_line(stdout: &str, stderr: &str) -> Option<String> {
    stdout
        .lines()
        .chain(stderr.lines())
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(64).collect())
}

async fn probe_cli_version_bounded(bin_path: &Path) -> (Option<String>, bool) {
    let Ok(_permit) = ACTIVE_PROBE_SEMAPHORE.acquire().await else {
        return (None, false);
    };
    let bin = bin_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new(&bin);
        cmd.arg("--version")
            .env("CI", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let Ok(mut child) = cmd.spawn() else {
            return (None, false);
        };
        let pid = child.id();
        let out_drain = child.stdout.take().map(spawn_bounded_pipe_drain);
        let err_drain = child.stderr.take().map(spawn_bounded_pipe_drain);
        let deadline = Instant::now() + Duration::from_millis(MAX_PROBE_TIMEOUT_MS);
        loop {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    let stdout = out_drain.and_then(|h| h.join().ok()).unwrap_or_default();
                    let stderr = err_drain.and_then(|h| h.join().ok()).unwrap_or_default();
                    return (first_version_line(&stdout, &stderr), false);
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        #[cfg(unix)]
                        unsafe {
                            libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = child.kill();
                        }
                        let _ = child.wait();
                        if let Some(h) = out_drain {
                            let _ = h.join();
                        }
                        if let Some(h) = err_drain {
                            let _ = h.join();
                        }
                        return (None, true);
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
                Err(_) => return (None, false),
            }
        }
    })
    .await
    .unwrap_or((None, false))
}

async fn evaluate_cli_target(def: CliTargetDefinition, active_probe: bool) -> CliDoctorToolDto {
    let start = Instant::now();
    let scan_mode = if active_probe { "active" } else { "passive" };
    let bin_opt = tokio::task::spawn_blocking(move || resolve_cli_binary_path(def.command))
        .await
        .ok()
        .flatten();

    let Some(bin_path) = bin_opt else {
        if matches!(def.id, "cline" | "roo" | "continue") {
            let has_ext = ide_extension_installed(def.id, def.config_subpaths);
            return enrich_cli_tool_dto(CliDoctorToolDto {
                id: def.id.to_string(),
                name: def.name.to_string(),
                command: def.command.to_string(),
                category: def.category.to_string(),
                scan_mode: scan_mode.to_string(),
                installed: has_ext,
                version: None,
                authenticated: false,
                status: "manual_ide_setup".to_string(),
                auth_reason: Some(
                    "IDE extension requires manual provider setup inside host editor".to_string(),
                ),
                latency_ms: start.elapsed().as_millis() as u64,
                account_label: None,
                provider_group: None,
                models: Vec::new(),
            })
            .await;
        }
        return enrich_cli_tool_dto(CliDoctorToolDto {
            id: def.id.to_string(),
            name: def.name.to_string(),
            command: def.command.to_string(),
            category: def.category.to_string(),
            scan_mode: scan_mode.to_string(),
            installed: false,
            version: None,
            authenticated: false,
            status: "not_installed".to_string(),
            auth_reason: Some(format!("Command '{}' not found in PATH", def.command)),
            latency_ms: start.elapsed().as_millis() as u64,
            account_label: None,
            provider_group: None,
            models: Vec::new(),
        })
        .await;
    };

    let (mut authenticated, mut auth_reason, passive_hint) = if def.id == "ollama" {
        check_ollama_live_health().await
    } else {
        let (ok, reason) = check_cli_passive_auth(&def);
        (ok, reason, None)
    };

    let (version, timed_out) = if active_probe {
        let (v, to) = probe_cli_version_bounded(&bin_path).await;
        (v.or(passive_hint), to)
    } else {
        (passive_hint, false)
    };

    let status = if timed_out {
        authenticated = false;
        auth_reason = Some("Active probe exceeded 1500ms PGID budget".to_string());
        "timeout".to_string()
    } else if authenticated {
        "ready".to_string()
    } else if matches!(def.id, "cline" | "roo" | "continue")
        && ide_extension_installed(def.id, def.config_subpaths)
    {
        auth_reason =
            Some("IDE extension requires manual provider setup inside host editor".to_string());
        "manual_ide_setup".to_string()
    } else {
        "auth_needed".to_string()
    };

    enrich_cli_tool_dto(CliDoctorToolDto {
        id: def.id.to_string(),
        name: def.name.to_string(),
        command: def.command.to_string(),
        category: def.category.to_string(),
        scan_mode: scan_mode.to_string(),
        installed: true,
        version,
        authenticated,
        status,
        auth_reason,
        latency_ms: start.elapsed().as_millis() as u64,
        account_label: None,
        provider_group: None,
        models: Vec::new(),
    })
    .await
}

pub async fn handle_get_doctor_cli_matrix(
    Extension(_principal): Extension<FlowPrincipal>,
    Query(query): Query<DoctorMatrixQuery>,
) -> Result<Json<DoctorCliMatrixResponse>, Response> {
    let active_probe = matches!(
        query.mode.as_deref().map(str::trim),
        Some("active") | Some("ACTIVE")
    );
    let futs: Vec<_> = SUPPORTED_CLI_DEFINITIONS
        .iter()
        .copied()
        .map(|def| evaluate_cli_target(def, active_probe))
        .collect();
    let tools = futures_util::future::join_all(futs).await;

    Ok(Json(DoctorCliMatrixResponse {
        clis: tools.clone(),
        tools,
        scan_mode: if active_probe {
            "active".to_string()
        } else {
            "passive".to_string()
        },
        scanned_at: Utc::now().to_rfc3339(),
    }))
}

// -----------------------------------------------------------------------------
// 3. Explicit Per-CLI Auth & Version Probe (`POST /api/v1/doctor/probe-cli`)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeCliToolRequest {
    #[serde(default)]
    pub tool_id: String,
    #[serde(default)]
    pub cli_id: String,
}

pub async fn handle_probe_cli_tool(
    Extension(_principal): Extension<FlowPrincipal>,
    Json(req): Json<ProbeCliToolRequest>,
) -> Result<Json<CliDoctorToolDto>, Response> {
    let target_id = if !req.tool_id.is_empty() {
        req.tool_id.trim().to_ascii_lowercase()
    } else {
        req.cli_id.trim().to_ascii_lowercase()
    };

    if target_id.is_empty() {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "toolId or cliId is required",
        ));
    }

    let found = SUPPORTED_CLI_DEFINITIONS
        .iter()
        .copied()
        .find(|d| d.id == target_id || d.command == target_id);

    match found {
        Some(def) => Ok(Json(evaluate_cli_target(def, true).await)),
        None => Err(json_err(
            StatusCode::NOT_FOUND,
            "CLI_NOT_FOUND",
            format!("CLI tool '{target_id}' is not in supported doctor registry"),
        )),
    }
}

// -----------------------------------------------------------------------------
// 4. Encrypted 0o600 Vault Keys & Custody Settings
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultKeyDto {
    pub id: String,
    pub provider_id: String,
    pub key_hint: String,
    pub vault_ref: String,
    pub vault_mode: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertVaultKeyRequest {
    pub provider_id: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_vault_mode")]
    pub vault_mode: String,
}

fn default_vault_mode() -> String {
    "keychain".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteVaultKeyResponse {
    pub deleted: bool,
    pub id: String,
}

pub async fn handle_list_vault_keys(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
) -> Result<Json<Vec<VaultKeyDto>>, Response> {
    let pool = state.registry.pool().clone();
    let rows = run_db(move || {
        let conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, provider_id, credential_ref, vault_mode, status, created_at, updated_at
                 FROM agent_settings
                 WHERE tenant_id = ?1 AND credential_ref != ''
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| map_harness_resp(map_sqlite_error(e, "prep_vault_keys"), "list"))?;

        let items = stmt
            .query_map(params![principal.tenant_id], |row| {
                let provider_id: String = row.get(1)?;
                let vault_mode: String = row.get(3)?;
                let key_hint: String = row.get(2)?;
                Ok(VaultKeyDto {
                    id: row.get(0)?,
                    vault_ref: format!("vault://{vault_mode}/{provider_id}"),
                    provider_id,
                    key_hint,
                    vault_mode,
                    status: row.get(4)?,
                    created_at: epoch_ms_to_iso(row.get(5)?),
                    updated_at: epoch_ms_to_iso(row.get(6)?),
                })
            })
            .map_err(|e| map_harness_resp(map_sqlite_error(e, "query_vault_keys"), "list"))?;

        let mut out = Vec::new();
        for item in items {
            out.push(item.map_err(|e| map_harness_resp(map_sqlite_error(e, "row"), "list"))?);
        }
        Ok(out)
    })
    .await?;

    Ok(Json(rows))
}

pub async fn handle_upsert_vault_key(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Json(req): Json<UpsertVaultKeyRequest>,
) -> Result<Json<VaultKeyDto>, Response> {
    let provider_id = check_non_empty_id(&req.provider_id, "providerId")?.to_ascii_lowercase();
    let api_key_trimmed = req.api_key.trim().to_string();
    if api_key_trimmed.is_empty() {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "apiKey is required to store in vault",
        ));
    }
    let vault_mode = req.vault_mode.trim().to_string();
    if !matches!(
        vault_mode.as_str(),
        "keychain" | "vault_0600" | "ram_only" | "ephemeral"
    ) {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid vaultMode",
        ));
    }

    let store = VaultStore::from_workspace_root(state.registry.pool().workspace_root());
    if vault_mode == "ephemeral" && store.is_vault_writable() {
        return Err(json_err(
            StatusCode::CONFLICT,
            "VAULT_DOWNGRADE_FORBIDDEN",
            "Cannot downgrade credential custody to ephemeral when encrypted 0o600 vault is writable",
        ));
    }

    let safe_cred = sanitize_credential_ref(&api_key_trimmed);
    let vault_ref = if matches!(vault_mode.as_str(), "keychain" | "vault_0600") {
        let store_clone = store.clone();
        let pid_clone = provider_id.clone();
        let key_clone = api_key_trimmed.clone();
        tokio::task::spawn_blocking(move || store_clone.upsert_secret(&pid_clone, &key_clone, None))
            .await
            .map_err(|e| json_err(StatusCode::INTERNAL_SERVER_ERROR, "VAULT_IO", e.to_string()))?
            .map_err(|e| json_err(StatusCode::INTERNAL_SERVER_ERROR, "VAULT_IO", e))?
    } else if vault_mode == "ram_only" {
        state
            .ram_vault
            .upsert(&principal.tenant_id, &provider_id, &api_key_trimmed);
        format!("vault://ram_only/{provider_id}")
    } else {
        state.ram_vault.remove(&principal.tenant_id, &provider_id);
        format!("vault://ephemeral/{provider_id}")
    };
    if vault_mode != "ram_only" {
        state.ram_vault.remove(&principal.tenant_id, &provider_id);
    }

    let pool = state.registry.pool().clone();
    let tenant_id_for_clear = principal.tenant_id.clone();
    let provider_id_for_clear = provider_id.clone();
    let saved = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        let now_ms = Utc::now().timestamp_millis();
        let new_id = Uuid::new_v4().to_string();

        let dto = retry_on_busy("upsert_vault_key", || {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let existing: Option<String> = tx
                .query_row(
                    "SELECT id FROM agent_settings WHERE tenant_id = ?1 AND provider_id = ?2 AND project_id IS NULL",
                    params![principal.tenant_id, provider_id],
                    |r| r.get(0),
                )
                .optional()?;

            let final_id = if let Some(ex_id) = existing {
                tx.execute(
                    "UPDATE agent_settings
                     SET credential_ref = ?1, vault_mode = ?2, status = 'configured', updated_at = ?3
                     WHERE id = ?4",
                    params![safe_cred, vault_mode, now_ms, ex_id],
                )?;
                ex_id
            } else {
                tx.execute(
                    "INSERT INTO agent_settings (
                        id, tenant_id, project_id, provider_id, model_id, endpoint_url,
                        credential_ref, is_default, status, permission_mode, vault_mode,
                        onboarding_done, created_at, updated_at
                     ) VALUES (?1, ?2, NULL, ?3, ?3, '', ?4, 0, 'configured', 'ask-every-time', ?5, 1, ?6, ?6)",
                    params![new_id, principal.tenant_id, provider_id, safe_cred, vault_mode, now_ms],
                )?;
                new_id.clone()
            };

            let row = tx.query_row(
                "SELECT id, provider_id, credential_ref, vault_mode, status, created_at, updated_at
                 FROM agent_settings WHERE id = ?1",
                params![final_id],
                |r| {
                    let pid: String = r.get(1)?;
                    let vm: String = r.get(3)?;
                    Ok(VaultKeyDto {
                        id: r.get(0)?,
                        provider_id: pid,
                        key_hint: r.get(2)?,
                        vault_ref: vault_ref.clone(),
                        vault_mode: vm,
                        status: r.get(4)?,
                        created_at: epoch_ms_to_iso(r.get(5)?),
                        updated_at: epoch_ms_to_iso(r.get(6)?),
                    })
                },
            )?;
            tx.commit()?;
            Ok::<VaultKeyDto, rusqlite::Error>(row)
        })
        .map_err(|e| map_harness_resp(e, "upsert_vault_key"))?;

        Ok(dto)
    })
    .await?;

    if matches!(provider_id_for_clear.as_str(), "github" | "github_plugins") {
        let pool_clear = state.registry.pool().clone();
        let tid_clear = tenant_id_for_clear;
        let _ = run_db(move || {
            spec_registry::plugin_store::clear_auth_required_schedules_for_tenant(
                &pool_clear,
                &tid_clear,
                Utc::now().timestamp_millis(),
            )
            .map_err(|e| map_harness_resp(e, "clear_auth_required_schedules"))
        })
        .await;
    }

    Ok(Json(saved))
}

pub async fn handle_delete_vault_key(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(key_id): AxumPath<String>,
) -> Result<Json<DeleteVaultKeyResponse>, Response> {
    let pool = state.registry.pool().clone();
    let store = VaultStore::from_workspace_root(pool.workspace_root());
    let key_lookup = key_id.clone();

    let tenant_for_db = principal.tenant_id.clone();
    let (cnt, matched_providers) = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        let mut providers = Vec::new();
        {
            let mut stmt = conn
                .prepare("SELECT provider_id FROM agent_settings WHERE tenant_id = ?1 AND (id = ?2 OR provider_id = ?2)")
                .map_err(|e| map_harness_resp(map_sqlite_error(e, "lookup_vault_key"), "delete"))?;
            let rows = stmt
                .query_map(params![tenant_for_db, key_lookup], |r| {
                    r.get::<_, String>(0)
                })
                .map_err(|e| map_harness_resp(map_sqlite_error(e, "lookup_vault_key"), "delete"))?;
            for row_res in rows {
                let p = row_res.map_err(|e| {
                    map_harness_resp(map_sqlite_error(e, "lookup_vault_key_row"), "delete")
                })?;
                providers.push(p);
            }
        }
        let cnt = retry_on_busy("delete_vault_key", || {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let c = tx.execute(
                "UPDATE agent_settings SET credential_ref = '' WHERE tenant_id = ?1 AND (id = ?2 OR provider_id = ?2)",
                params![tenant_for_db, key_lookup],
            )?;
            tx.commit()?;
            Ok::<usize, rusqlite::Error>(c)
        })
        .map_err(|e| map_harness_resp(e, "delete_vault_key"))?;
        Ok((cnt, providers))
    })
    .await?;

    if cnt == 0 {
        return Err(json_err(
            StatusCode::NOT_FOUND,
            "KEY_NOT_FOUND",
            format!("Vault key '{key_id}' was not found"),
        ));
    }

    for pid in &matched_providers {
        state.ram_vault.remove(&principal.tenant_id, pid);
    }
    let store_clone = store.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        for pid in matched_providers {
            store_clone.delete_secret(&pid, None)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| json_err(StatusCode::INTERNAL_SERVER_ERROR, "VAULT_IO", e.to_string()))?
    .map_err(|e| json_err(StatusCode::INTERNAL_SERVER_ERROR, "VAULT_IO", e))?;

    Ok(Json(DeleteVaultKeyResponse {
        deleted: true,
        id: key_id,
    }))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateVaultSettingsRequest {
    pub vault_mode: String,
    #[serde(default)]
    pub master_passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateVaultSettingsResponse {
    pub vault_mode: String,
    pub vault_writable: bool,
    pub encrypted_vault_present: bool,
    pub updated_at: String,
}

pub async fn handle_update_vault_settings(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Json(req): Json<UpdateVaultSettingsRequest>,
) -> Result<Json<UpdateVaultSettingsResponse>, Response> {
    let vault_mode = req.vault_mode.trim().to_string();
    if !matches!(
        vault_mode.as_str(),
        "keychain" | "vault_0600" | "ram_only" | "ephemeral"
    ) {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid vaultMode",
        ));
    }

    let store = VaultStore::from_workspace_root(state.registry.pool().workspace_root());
    let writable = store.is_vault_writable();
    if vault_mode == "ephemeral" && writable {
        return Err(json_err(
            StatusCode::CONFLICT,
            "VAULT_DOWNGRADE_FORBIDDEN",
            "Cannot downgrade credential custody to ephemeral when encrypted 0o600 vault is writable",
        ));
    }

    if vault_mode == "vault_0600" && !store.vault_path().exists() {
        let store_clone = store.clone();
        let pass_opt = req.master_passphrase.clone();
        tokio::task::spawn_blocking(move || {
            store_clone.seal_secrets(&BTreeMap::new(), pass_opt.as_deref())
        })
        .await
        .map_err(|e| json_err(StatusCode::INTERNAL_SERVER_ERROR, "VAULT_IO", e.to_string()))?
        .map_err(|e| json_err(StatusCode::INTERNAL_SERVER_ERROR, "VAULT_IO", e))?;
    }

    let tenant_for_settings = principal.tenant_id.clone();
    let vm_clone = vault_mode.clone();
    let pool = state.registry.pool().clone();
    run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        let now_ms = Utc::now().timestamp_millis();
        retry_on_busy("update_vault_settings", || {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE agent_settings SET vault_mode = ?1, updated_at = ?2 WHERE tenant_id = ?3",
                params![vm_clone, now_ms, tenant_for_settings],
            )?;
            tx.commit()?;
            Ok::<(), rusqlite::Error>(())
        })
        .map_err(|e| map_harness_resp(e, "update_vault_settings"))
    })
    .await?;

    if vault_mode != "ram_only" {
        state.ram_vault.clear_tenant(&principal.tenant_id);
    }

    Ok(Json(UpdateVaultSettingsResponse {
        vault_mode,
        vault_writable: writable,
        encrypted_vault_present: store.vault_path().exists(),
        updated_at: Utc::now().to_rfc3339(),
    }))
}
