// @story 04.3, 04.5
// @implements Clean-Room EV-001 & EV-002 (Local CLI Identity, Model Catalog & Honest 5h/Weekly Quota Telemetry)
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use axum::extract::{Extension, Query};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::daemon_auth::FlowPrincipal;
use crate::routes::doctor_vault::{
    resolve_cli_binary_path_pub, CliDoctorToolDto, SUPPORTED_CLI_DEFINITIONS_PUB,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorModelOptionDto {
    pub id: String,
    pub label: String,
    pub provider_id: String,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindowDto {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at_epoch_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_minutes: Option<i64>,
    pub reset_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsageDto {
    pub id: String,
    pub name: String,
    pub provider_group: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub checked_at: String,
    pub checked_at_epoch_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<QuotaWindowDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly: Option<QuotaWindowDto>,
    #[serde(default)]
    pub models: Vec<DoctorModelOptionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorUsageResponse {
    pub providers: Vec<ProviderUsageDto>,
    pub checked_at: String,
    pub checked_at_epoch_ms: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorUsageQuery {
    #[serde(default)]
    pub refresh: Option<String>,
}

pub fn provider_group_for_cli(id: &str) -> &'static str {
    match id {
        "codex" => "openai",
        "claude" => "anthropic",
        "gemini" | "agy" => "google",
        "qwen" => "qwen",
        "copilot" => "github",
        "opencode" => "opencode",
        "pi" => "pi",
        "ollama" => "local",
        "cursor" => "cursor",
        _ => "byos",
    }
}

pub fn default_models_for_cli(id: &str) -> Vec<DoctorModelOptionDto> {
    let items: &[(&str, &str, bool)] = match id {
        "codex" => &[
            ("codex:gpt-5.3-codex", "GPT-5.3 Codex", true),
            ("codex:gpt-5.2-codex", "GPT-5.2 Codex", false),
            ("openai-gpt4o", "GPT-4o / o3-mini", false),
        ],
        "claude" => &[
            ("claude-37", "Claude Sonnet 4 / 3.7", true),
            ("claude:opus-4", "Claude Opus 4", false),
        ],
        "agy" | "gemini" => &[
            ("gemini-25", "Gemini 2.5 Pro", true),
            ("gemini:flash-25", "Gemini 2.5 Flash", false),
        ],
        "opencode" => &[("opencode:auto", "OpenCode Multi-Model", true)],
        "qwen" => &[("qwen:qwen3-coder", "Qwen3 Coder", true)],
        "pi" => &[("pi:agent-default", "Pi Coding Agent", true)],
        "copilot" => &[("copilot:gpt-4.1", "GitHub Copilot Agent", true)],
        "ollama" => &[("deepseek-r1", "DeepSeek R1 (Ollama Local)", true)],
        "cursor" => &[("cursor:agent", "Cursor Agent CLI", true)],
        _ => &[],
    };
    items
        .iter()
        .map(|(m_id, label, rec)| DoctorModelOptionDto {
            id: (*m_id).to_string(),
            label: (*label).to_string(),
            provider_id: id.to_string(),
            recommended: *rec,
        })
        .collect()
}

fn decode_b64_url_segment(seg: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u8 = 0;
    for b in seg.bytes() {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

fn email_from_jwt_payload(jwt: &str) -> Option<String> {
    let seg = jwt.split('.').nth(1)?;
    let bytes = decode_b64_url_segment(seg)?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    let email = v.get("email").and_then(Value::as_str)?.trim();
    if email.contains('@') && email.len() <= 160 {
        Some(email.to_string())
    } else {
        None
    }
}

pub fn extract_cli_account_label(id: &str) -> Option<String> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    match id {
        "codex" => extract_codex_label(&home),
        "claude" => extract_claude_label(&home),
        "gemini" | "agy" => extract_gemini_label(&home),
        "opencode" => has_any_nonempty(
            &home,
            &[
                ".config/opencode/opencode.jsonc",
                ".local/share/opencode/auth.json",
            ],
        )
        .then(|| "Local OpenCode config".to_string()),
        "qwen" => has_any_nonempty(&home, &[".qwen/oauth_creds.json", ".qwen/settings.json"])
            .then(|| "Qwen local profile".to_string()),
        "pi" => has_any_nonempty(&home, &[".pi/agent/auth.json", ".pi/agent/settings.json"])
            .then(|| "Pi local agent".to_string()),
        "copilot" => extract_gh_label(&home),
        _ => None,
    }
}

fn has_any_nonempty(home: &Path, subpaths: &[&str]) -> bool {
    subpaths.iter().any(|rel| {
        let p = home.join(rel);
        p.is_file() && std::fs::metadata(&p).map(|m| m.len() > 2).unwrap_or(false)
    })
}

fn extract_codex_label(home: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(home.join(".codex/auth.json")).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    if let Some(tok) = json
        .get("tokens")
        .and_then(|t| t.get("id_token"))
        .and_then(Value::as_str)
    {
        if let Some(email) = email_from_jwt_payload(tok) {
            return Some(email);
        }
    }
    json.get("auth_mode")
        .and_then(Value::as_str)
        .map(|m| format!("Codex ({m})"))
}

fn extract_claude_label(home: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(home.join(".claude.json")).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    json.get("oauthAccount")
        .and_then(|o| o.get("emailAddress"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_gemini_label(home: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(home.join(".gemini/google_accounts.json")).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    json.get("active")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_gh_label(home: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(home.join(".config/gh/hosts.yml")).ok()?;
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("user:") {
            let user = rest.trim().trim_matches('"');
            if !user.is_empty() {
                return Some(format!("@{user}"));
            }
        }
    }
    None
}

fn epoch_sec_to_iso(sec: i64) -> Option<(String, i64)> {
    let ms = sec.checked_mul(1000)?;
    DateTime::<Utc>::from_timestamp(sec, 0).map(|dt| (dt.to_rfc3339(), ms))
}

fn parse_codex_window(win: &Value, now_ms: i64) -> Option<QuotaWindowDto> {
    let used = win.get("used_percent").and_then(Value::as_f64)?;
    let clamped_used = used.clamp(0.0, 100.0);
    let remaining = (100.0 - clamped_used).max(0.0);
    let window_minutes = win.get("window_minutes").and_then(Value::as_i64);
    let (resets_at, resets_at_epoch_ms, reset_passed) =
        match win.get("resets_at").and_then(Value::as_i64) {
            Some(sec) => match epoch_sec_to_iso(sec) {
                Some((iso, ms)) => (Some(iso), Some(ms), ms <= now_ms),
                None => (None, None, false),
            },
            None => (None, None, false),
        };
    Some(QuotaWindowDto {
        available: true,
        used_percent: Some(clamped_used),
        remaining_percent: Some(remaining),
        resets_at,
        resets_at_epoch_ms,
        window_minutes,
        reset_passed,
    })
}

fn collect_recent_codex_logs(dir: &Path, files: &mut Vec<(SystemTime, PathBuf)>, depth: u8) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_recent_codex_logs(&p, files, depth + 1);
        } else if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            files.push((mtime, p));
        }
    }
}

struct CodexQuotaSnapshot {
    plan: Option<String>,
    five_hour: Option<QuotaWindowDto>,
    weekly: Option<QuotaWindowDto>,
    note: Option<String>,
}

fn read_latest_codex_quota(now_ms: i64) -> CodexQuotaSnapshot {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return CodexQuotaSnapshot {
            plan: None,
            five_hour: None,
            weekly: None,
            note: None,
        };
    };
    let sessions_dir = home.join(".codex/sessions");
    let mut files = Vec::new();
    collect_recent_codex_logs(&sessions_dir, &mut files, 0);
    files.sort_by_key(|a| std::cmp::Reverse(a.0));

    for (_, path) in files.into_iter().take(20) {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in raw.lines().rev() {
            if !line.contains("rate_limits") {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let rl = v
                .get("payload")
                .and_then(|p| p.get("rate_limits"))
                .or_else(|| v.get("rate_limits"));
            let Some(rl) = rl else {
                continue;
            };
            let five_hour = rl
                .get("primary")
                .and_then(|w| parse_codex_window(w, now_ms));
            let weekly = rl
                .get("secondary")
                .and_then(|w| parse_codex_window(w, now_ms));
            if five_hour.is_some() || weekly.is_some() {
                let plan = rl
                    .get("plan_type")
                    .and_then(Value::as_str)
                    .map(|s| format!("ChatGPT {s}"));
                return CodexQuotaSnapshot {
                    plan,
                    five_hour,
                    weekly,
                    note: None,
                };
            }
        }
    }
    CodexQuotaSnapshot {
        plan: Some("ChatGPT / Codex CLI".to_string()),
        five_hour: None,
        weekly: None,
        note: Some("Chưa có log rate_limits cục bộ từ phiên Codex gần đây.".to_string()),
    }
}

async fn fetch_ollama_models() -> Option<Vec<DoctorModelOptionDto>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(350))
        .build()
        .ok()?;
    let resp = client
        .get("http://127.0.0.1:11434/api/tags")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: Value = resp.json().await.ok()?;
    let models = body.get("models")?.as_array()?;
    let mapped: Vec<DoctorModelOptionDto> = models
        .iter()
        .take(12)
        .filter_map(|m| m.get("name").and_then(Value::as_str))
        .enumerate()
        .map(|(idx, name)| DoctorModelOptionDto {
            id: format!("ollama:{name}"),
            label: format!("{name} (Ollama Local)"),
            provider_id: "ollama".to_string(),
            recommended: idx == 0,
        })
        .collect();
    if mapped.is_empty() {
        None
    } else {
        Some(mapped)
    }
}

pub async fn enrich_cli_tool_dto(mut dto: CliDoctorToolDto) -> CliDoctorToolDto {
    dto.provider_group = Some(provider_group_for_cli(&dto.id).to_string());
    if dto.installed {
        dto.account_label = extract_cli_account_label(&dto.id).or(dto.account_label);
    }
    let mut models = default_models_for_cli(&dto.id);
    if dto.id == "ollama" && dto.authenticated {
        if let Some(live) = fetch_ollama_models().await {
            models = live;
        }
    }
    dto.models = models;
    dto
}

pub async fn handle_get_doctor_usage(
    Extension(_principal): Extension<FlowPrincipal>,
    Query(_query): Query<DoctorUsageQuery>,
) -> Json<DoctorUsageResponse> {
    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    let now_iso = now.to_rfc3339();

    let blocking_scan = tokio::task::spawn_blocking(move || {
        let mut codex_snapshot: Option<CodexQuotaSnapshot> = None;
        let mut discovered = Vec::new();
        for def in SUPPORTED_CLI_DEFINITIONS_PUB.iter().copied() {
            let installed = resolve_cli_binary_path_pub(def.command).is_some();
            if !installed && def.id != "ollama" {
                continue;
            }
            if def.id == "codex" && installed {
                codex_snapshot = Some(read_latest_codex_quota(now_ms));
            }
            let account_label = extract_cli_account_label(def.id);
            discovered.push((def, installed, account_label));
        }
        (discovered, codex_snapshot)
    })
    .await
    .unwrap_or_else(|_| (Vec::new(), None));

    let (discovered, codex_snapshot) = blocking_scan;
    let ollama_installed = discovered
        .iter()
        .any(|(def, installed, _)| def.id == "ollama" && *installed);
    let ollama_live = if ollama_installed {
        fetch_ollama_models().await
    } else {
        None
    };

    let mut providers = Vec::new();
    for (def, installed, account_label) in discovered {
        let is_ollama_ready = def.id == "ollama" && ollama_live.is_some();
        let (five_hour, weekly, plan, note) = match def.id {
            "codex" => codex_snapshot
                .as_ref()
                .map_or((None, None, None, None), |snap| {
                    (
                        snap.five_hour.clone(),
                        snap.weekly.clone(),
                        snap.plan.clone(),
                        snap.note.clone(),
                    )
                }),
            "ollama" => (
                None,
                None,
                Some("Local Runtime".to_string()),
                if is_ollama_ready {
                    Some("Local GPU / CPU runtime unlimited".to_string())
                } else {
                    Some("Ollama daemon offline".to_string())
                },
            ),
            _ => (None, None, None, None),
        };
        let models = if def.id == "ollama" {
            ollama_live
                .clone()
                .unwrap_or_else(|| default_models_for_cli("ollama"))
        } else {
            default_models_for_cli(def.id)
        };
        let ok = if def.id == "ollama" {
            is_ollama_ready
        } else {
            account_label.is_some()
        };
        providers.push(ProviderUsageDto {
            id: def.id.to_string(),
            name: def.name.to_string(),
            provider_group: provider_group_for_cli(def.id).to_string(),
            status: if ok {
                "ready".to_string()
            } else if installed {
                "auth_needed".to_string()
            } else {
                "not_installed".to_string()
            },
            account_label,
            plan,
            ok,
            note,
            checked_at: now_iso.clone(),
            checked_at_epoch_ms: now_ms,
            five_hour,
            weekly,
            models,
        });
    }

    Json(DoctorUsageResponse {
        providers,
        checked_at: now_iso,
        checked_at_epoch_ms: now_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_five_hour_and_weekly_quota_windows() {
        let now_ms = 1_794_500_000_000;
        let five_h = serde_json::json!({
            "used_percent": 22.0,
            "window_minutes": 300,
            "resets_at": 1_794_600_000
        });
        let w5 = parse_codex_window(&five_h, now_ms).expect("5h window");
        assert!(w5.available);
        assert_eq!(w5.used_percent, Some(22.0));
        assert_eq!(w5.remaining_percent, Some(78.0));
        assert_eq!(w5.window_minutes, Some(300));
        assert!(!w5.reset_passed);

        let past_win = serde_json::json!({
            "used_percent": 100.0,
            "window_minutes": 10080,
            "resets_at": 1_794_400_000
        });
        let wk = parse_codex_window(&past_win, now_ms).expect("weekly window");
        assert!(wk.reset_passed);
        assert_eq!(wk.remaining_percent, Some(0.0));
    }

    #[test]
    fn decodes_safe_jwt_email_claim_only() {
        // header {"alg":"none"}, payload {"email":"dev@cowok.ai"}, sig
        let jwt = [
            "eyJhbGciOiJub25lIn0",
            "eyJlbWFpbCI6ImRldkBjb3dvay5haSJ9",
            "",
        ]
        .join(".");
        assert_eq!(
            email_from_jwt_payload(&jwt).as_deref(),
            Some("dev@cowok.ai")
        );
    }

    #[tokio::test]
    async fn handle_get_doctor_usage_returns_clean_payload() {
        let principal = FlowPrincipal {
            tenant_id: "default".to_string(),
            user_id: "qa-user".to_string(),
            role: "editor".to_string(),
        };
        let Json(resp) = handle_get_doctor_usage(
            Extension(principal),
            Query(DoctorUsageQuery { refresh: None }),
        )
        .await;
        assert!(resp.checked_at_epoch_ms > 0);
        assert!(resp.providers.iter().any(|p| p.id == "ollama"));
    }
}
