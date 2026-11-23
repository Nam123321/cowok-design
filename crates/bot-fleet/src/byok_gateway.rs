// @story 11.1
// @implements AC-02, AC-03 (Unified BYOK Gateway, DNS-Pinned SSRF Hardening, Tenant-Scoped 429 Cooldown & Bounded Stream)
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::adapters::validate_model_id;
use crate::posix_sandbox::redact_secrets_from_text;
use crate::BotFleetError;

pub const CANONICAL_BYOK_PROVIDERS: &[&str] = &[
    "gemini",
    "claude",
    "openai",
    "openrouter",
    "deepseek",
    "byos",
];

/// Prohibited local infrastructure ports (`EC-SYNC-022` — FalkorDB, Redis, Distro, Ollama/Postgres infra).
pub const PROHIBITED_LOCAL_PORTS: &[u16] = &[
    3689, 5000, 6333, 6334, 6379, 6380, 6381, 6382, 6383, 6390, 7000, 54321, 54322, 54323, 54324,
];

const MAX_UPSTREAM_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const DNS_LOOKUP_TIMEOUT_SECS: u64 = 2;

pub fn canonical_provider_id(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => "claude".to_string(),
        "google" | "gemini" => "gemini".to_string(),
        "openai" => "openai".to_string(),
        "openrouter" => "openrouter".to_string(),
        "deepseek" => "deepseek".to_string(),
        "byos" | "ollama" => "byos".to_string(),
        other => other.to_string(),
    }
}

pub fn default_model_for_provider(provider_id: &str) -> &'static str {
    match canonical_provider_id(provider_id).as_str() {
        "gemini" => "gemini-2.5-pro",
        "claude" => "claude-sonnet-4-5",
        "openai" => "gpt-4.1",
        "openrouter" => "anthropic/claude-sonnet-4",
        "deepseek" => "deepseek-chat",
        "byos" => "qwen2.5-coder:7b",
        _ => "gpt-4.1",
    }
}

#[derive(Clone)]
pub struct ByokCredential {
    pub provider_id: String,
    pub api_key: Zeroizing<String>,
    pub endpoint_url: Option<String>,
    pub model_id: Option<String>,
}

impl std::fmt::Debug for ByokCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ByokCredential")
            .field("provider_id", &self.provider_id)
            .field("api_key", &"[REDACTED]")
            .field("endpoint_url", &self.endpoint_url)
            .field("model_id", &self.model_id)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCooldownDto {
    pub provider_id: String,
    pub configured: bool,
    pub in_cooldown: bool,
    pub retry_after_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown_until_ms: Option<i64>,
    pub consecutive_429s: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
struct CooldownEntry {
    until: Instant,
    until_epoch_ms: i64,
    consecutive_429s: u32,
    last_error: Option<String>,
}

type TenantProviderKey = (String, String);

/// Tenant-scoped HTTP 429 cooldown tracker keyed by `(tenant_id, canonical_provider_id)`.
#[derive(Debug, Clone, Default)]
pub struct CooldownTracker {
    entries: Arc<RwLock<HashMap<TenantProviderKey, CooldownEntry>>>,
}

impl CooldownTracker {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(tenant_id: &str, provider_id: &str) -> TenantProviderKey {
        (
            tenant_id.trim().to_string(),
            canonical_provider_id(provider_id),
        )
    }

    /// Records a 429 rate-limit cooldown latch scoped strictly to `(tenant_id, provider_id)`.
    pub fn record_rate_limit(
        &self,
        tenant_id: &str,
        provider_id: &str,
        retry_after_secs: Option<u64>,
        reason: Option<String>,
    ) -> u64 {
        let key = Self::key(tenant_id, provider_id);
        let mut guard = self.entries.write().unwrap_or_else(|e| e.into_inner());
        let prev_count = guard.get(&key).map(|e| e.consecutive_429s).unwrap_or(0);
        let next_count = prev_count.saturating_add(1);
        let backoff = retry_after_secs
            .filter(|s| *s > 0)
            .unwrap_or_else(|| (1u64 << next_count.min(6)).clamp(2, 60))
            .clamp(1, 300);
        let until = Instant::now() + Duration::from_secs(backoff);
        let until_epoch_ms = Utc::now().timestamp_millis() + (backoff as i64 * 1000);
        guard.insert(
            key,
            CooldownEntry {
                until,
                until_epoch_ms,
                consecutive_429s: next_count,
                last_error: reason,
            },
        );
        backoff
    }

    /// Returns remaining cooldown seconds if `(tenant_id, provider_id)` is currently latched.
    pub fn remaining_cooldown_secs(&self, tenant_id: &str, provider_id: &str) -> Option<u64> {
        let key = Self::key(tenant_id, provider_id);
        let now = Instant::now();
        let guard = self.entries.read().unwrap_or_else(|e| e.into_inner());
        guard.get(&key).and_then(|entry| {
            if entry.until > now {
                Some(entry.until.saturating_duration_since(now).as_secs().max(1))
            } else {
                None
            }
        })
    }

    pub fn min_remaining_cooldown_secs(
        &self,
        tenant_id: &str,
        providers: &[String],
    ) -> Option<u64> {
        providers
            .iter()
            .filter_map(|p| self.remaining_cooldown_secs(tenant_id, p))
            .min()
    }

    pub fn clear(&self, tenant_id: &str, provider_id: &str) {
        let key = Self::key(tenant_id, provider_id);
        let mut guard = self.entries.write().unwrap_or_else(|e| e.into_inner());
        guard.remove(&key);
    }

    pub fn snapshot(
        &self,
        tenant_id: &str,
        configured_providers: &[String],
    ) -> Vec<ProviderCooldownDto> {
        let now = Instant::now();
        let guard = self.entries.read().unwrap_or_else(|e| e.into_inner());
        let configured_canon: Vec<String> = configured_providers
            .iter()
            .map(|p| canonical_provider_id(p))
            .collect();

        CANONICAL_BYOK_PROVIDERS
            .iter()
            .map(|pid| {
                let entry = guard.get(&(tenant_id.trim().to_string(), (*pid).to_string()));
                let remaining = entry
                    .filter(|e| e.until > now)
                    .map(|e| e.until.saturating_duration_since(now).as_secs().max(1))
                    .unwrap_or(0);
                let in_cooldown = remaining > 0;
                ProviderCooldownDto {
                    provider_id: (*pid).to_string(),
                    configured: configured_canon.iter().any(|c| c == pid),
                    in_cooldown,
                    retry_after_secs: remaining,
                    cooldown_until_ms: if in_cooldown {
                        entry.map(|e| e.until_epoch_ms)
                    } else {
                        None
                    },
                    consecutive_429s: entry.map(|e| e.consecutive_429s).unwrap_or(0),
                    last_error: entry.and_then(|e| e.last_error.clone()),
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ByokExecutionResult {
    pub provider_id: String,
    pub model_id: String,
    pub raw_content: String,
    pub cooldown_applied: bool,
    pub failover_providers: Vec<String>,
    pub duration_ms: u64,
    pub http_status: u16,
}

fn normalize_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        IpAddr::V4(_) => ip,
    }
}

fn is_forbidden_non_public_ip(raw_ip: IpAddr) -> bool {
    match normalize_ip(raw_ip) {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_multicast()
                || a == 0
                || a >= 240
                || (a == 100 && (b & 0b1100_0000) == 64)
                || (a == 192 && b == 0 && c == 2)
                || (a == 198 && (b == 18 || b == 19))
                || (a == 198 && b == 51 && c == 100)
                || (a == 203 && b == 0 && c == 113)
        }
        IpAddr::V6(v6) => {
            v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Validates and DNS-pins an upstream provider URL against SSRF, IPv4-mapped IPv6 bypasses,
/// prohibited infra ports, non-HTTPS remote hosts, and DNS rebinding.
pub async fn resolve_and_pin_provider_endpoint(
    url_str: &str,
    provider: &str,
    allow_loopback_for_cloud: bool,
) -> Result<(String, SocketAddr), BotFleetError> {
    let parsed = reqwest::Url::parse(url_str)
        .map_err(|e| BotFleetError::InvalidEndpoint(format!("Malformed provider URL: {e}")))?;
    let scheme = parsed.scheme();
    if scheme != "https" && scheme != "http" {
        return Err(BotFleetError::InvalidEndpoint(
            "Provider endpoint must use https:// or local BYOS http://".into(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| BotFleetError::InvalidEndpoint("Endpoint missing host".into()))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| BotFleetError::InvalidEndpoint("Endpoint missing port".into()))?;

    let addrs = tokio::time::timeout(
        Duration::from_secs(DNS_LOOKUP_TIMEOUT_SECS),
        tokio::net::lookup_host((host.as_str(), port)),
    )
    .await
    .map_err(|_| BotFleetError::InvalidEndpoint("Provider DNS lookup timed out".into()))?
    .map_err(|e| BotFleetError::InvalidEndpoint(format!("Provider DNS lookup failed: {e}")))?;

    let mut pinned: Option<SocketAddr> = None;
    for sa in addrs {
        let norm_ip = normalize_ip(sa.ip());
        if norm_ip.is_loopback() {
            let is_byos = canonical_provider_id(provider) == "byos";
            if !is_byos && !allow_loopback_for_cloud {
                return Err(BotFleetError::InvalidEndpoint(format!(
                    "SSRF blocked loopback endpoint for cloud provider '{provider}'"
                )));
            }
            if port < 1024 || PROHIBITED_LOCAL_PORTS.contains(&port) {
                return Err(BotFleetError::InvalidEndpoint(format!(
                    "SSRF blocked prohibited loopback port {port}"
                )));
            }
        } else {
            if scheme != "https" {
                return Err(BotFleetError::InvalidEndpoint(
                    "Non-loopback provider endpoint MUST use https://".into(),
                ));
            }
            if is_forbidden_non_public_ip(norm_ip) {
                return Err(BotFleetError::InvalidEndpoint(
                    "SSRF blocked private/link-local/CGNAT/reserved IP address".into(),
                ));
            }
        }
        if pinned.is_none() {
            pinned = Some(SocketAddr::new(norm_ip, sa.port()));
        }
    }

    let chosen = pinned.ok_or_else(|| {
        BotFleetError::InvalidEndpoint(format!("DNS returned no addresses for '{host}'"))
    })?;
    Ok((host, chosen))
}

fn resolve_provider_endpoint_url(
    cred: &ByokCredential,
    model: &str,
) -> Result<String, BotFleetError> {
    let provider = canonical_provider_id(&cred.provider_id);
    let allow_slash = provider != "gemini";
    let safe_model = validate_model_id(model, allow_slash)?;

    if let Some(ref custom) = cred.endpoint_url {
        let trimmed = custom.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            if provider == "claude" && !trimmed.ends_with("/messages") {
                return Ok(format!("{trimmed}/v1/messages"));
            }
            if provider == "gemini" && !trimmed.contains(":generateContent") {
                return Ok(format!("{trimmed}/models/{safe_model}:generateContent"));
            }
            if matches!(
                provider.as_str(),
                "openai" | "openrouter" | "deepseek" | "byos"
            ) && !trimmed.ends_with("/chat/completions")
                && !trimmed.ends_with("/responses")
            {
                return Ok(format!("{trimmed}/chat/completions"));
            }
            return Ok(trimmed.to_string());
        }
    }

    match provider.as_str() {
        "gemini" => Ok(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{safe_model}:generateContent"
        )),
        "claude" => Ok("https://api.anthropic.com/v1/messages".to_string()),
        "openai" => Ok("https://api.openai.com/v1/chat/completions".to_string()),
        "openrouter" => Ok("https://openrouter.ai/api/v1/chat/completions".to_string()),
        "deepseek" => Ok("https://api.deepseek.com/chat/completions".to_string()),
        "byos" => Err(BotFleetError::InvalidEndpoint(
            "BYOS provider requires an explicit custom endpointUrl".into(),
        )),
        _ => Err(BotFleetError::InvalidEndpoint(format!(
            "Unsupported BYOK provider '{provider}'"
        ))),
    }
}

fn parse_retry_after_secs(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|secs| *secs > 0)
}

async fn read_bounded_response_text(mut resp: reqwest::Response) -> Result<String, BotFleetError> {
    let mut buf = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| BotFleetError::GatewayError(format!("Failed reading upstream chunk: {e}")))?
    {
        if buf.len().saturating_add(chunk.len()) > MAX_UPSTREAM_RESPONSE_BYTES {
            return Err(BotFleetError::GatewayError(
                "Upstream response exceeded 4 MiB safety ceiling".into(),
            ));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn extract_text_from_provider_json(provider: &str, body: &Value) -> Option<String> {
    match provider {
        "gemini" => body
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(Value::as_array)
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|s| !s.is_empty()),
        "claude" => body
            .get("content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|s| !s.is_empty()),
        _ => body
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                body.get("output_text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                body.get("response")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
    }
}

#[derive(Clone)]
pub struct ByokGatewayRouter {
    cooldowns: CooldownTracker,
    allow_loopback_test_endpoints: bool,
}

impl Default for ByokGatewayRouter {
    fn default() -> Self {
        Self::new(CooldownTracker::new())
    }
}

impl ByokGatewayRouter {
    pub fn new(cooldowns: CooldownTracker) -> Self {
        Self {
            cooldowns,
            allow_loopback_test_endpoints: false,
        }
    }

    /// Creates a router allowing loopback mock endpoints for cloud providers in offline tests.
    pub fn with_loopback_test_endpoints(cooldowns: CooldownTracker) -> Self {
        Self {
            cooldowns,
            allow_loopback_test_endpoints: true,
        }
    }

    pub fn cooldowns(&self) -> &CooldownTracker {
        &self.cooldowns
    }

    fn build_candidate_order<'a>(
        &self,
        credentials: &'a [ByokCredential],
        preferred: Option<&str>,
    ) -> Vec<&'a ByokCredential> {
        let pref_canon = preferred.map(canonical_provider_id);
        let mut ordered: Vec<&ByokCredential> = Vec::with_capacity(credentials.len());

        if let Some(ref p) = pref_canon {
            for cred in credentials {
                if canonical_provider_id(&cred.provider_id) == *p {
                    ordered.push(cred);
                }
            }
        }
        for canonical in CANONICAL_BYOK_PROVIDERS {
            for cred in credentials {
                let cid = canonical_provider_id(&cred.provider_id);
                if cid == *canonical && !ordered.iter().any(|x| std::ptr::eq(*x, cred)) {
                    ordered.push(cred);
                }
            }
        }
        for cred in credentials {
            if !ordered.iter().any(|x| std::ptr::eq(*x, cred)) {
                ordered.push(cred);
            }
        }
        ordered
    }

    /// Routes a prompt across configured BYOK providers for `tenant_id` with DNS-pinned SSRF protection,
    /// tenant-scoped 429 `Retry-After` cooldown latching, bounded 4 MiB response reads, and auto-failover.
    pub async fn execute_with_failover(
        &self,
        tenant_id: &str,
        prompt: &str,
        preferred_provider: Option<&str>,
        model_override: Option<&str>,
        credentials: &[ByokCredential],
        timeout_ms: u64,
    ) -> Result<ByokExecutionResult, BotFleetError> {
        if credentials.is_empty() {
            return Err(BotFleetError::NoConfiguredProvider(
                "No BYOK provider credentials configured in VaultStore".into(),
            ));
        }

        let all_secrets: Vec<Zeroizing<String>> =
            credentials.iter().map(|c| c.api_key.clone()).collect();
        let candidates = self.build_candidate_order(credentials, preferred_provider);
        let timeout = Duration::from_millis(timeout_ms.clamp(500, 120_000));

        let start = Instant::now();
        let mut cooldown_applied = false;
        let mut failover_providers = Vec::new();
        let mut last_err = String::from("All configured BYOK providers unavailable");

        for cred in candidates {
            let provider = canonical_provider_id(&cred.provider_id);
            if let Some(rem) = self.cooldowns.remaining_cooldown_secs(tenant_id, &provider) {
                cooldown_applied = true;
                failover_providers.push(provider.clone());
                last_err = format!("Provider '{provider}' in HTTP 429 cooldown ({rem}s remaining)");
                continue;
            }

            let model = model_override
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .or_else(|| cred.model_id.as_deref().filter(|s| !s.trim().is_empty()))
                .unwrap_or_else(|| default_model_for_provider(&provider))
                .to_string();

            let endpoint = match resolve_provider_endpoint_url(cred, &model) {
                Ok(url) => url,
                Err(e) => {
                    last_err = redact_secrets_from_text(&e.to_string(), &all_secrets);
                    failover_providers.push(provider);
                    continue;
                }
            };

            let (pinned_host, pinned_addr) = match resolve_and_pin_provider_endpoint(
                &endpoint,
                &provider,
                self.allow_loopback_test_endpoints,
            )
            .await
            {
                Ok(pair) => pair,
                Err(e) => {
                    last_err = redact_secrets_from_text(&e.to_string(), &all_secrets);
                    failover_providers.push(provider);
                    continue;
                }
            };

            let client = match reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .resolve(&pinned_host, pinned_addr)
                .timeout(timeout)
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    last_err = redact_secrets_from_text(
                        &format!("HTTP client init failed: {e}"),
                        &all_secrets,
                    );
                    failover_providers.push(provider);
                    continue;
                }
            };

            let (req_builder, payload) = match provider.as_str() {
                "gemini" => (
                    client
                        .post(&endpoint)
                        .header("x-goog-api-key", cred.api_key.as_str()),
                    json!({
                        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }]
                    }),
                ),
                "claude" => (
                    client
                        .post(&endpoint)
                        .header("x-api-key", cred.api_key.as_str())
                        .header("anthropic-version", "2023-06-01"),
                    json!({
                        "model": model,
                        "max_tokens": 2048,
                        "messages": [{ "role": "user", "content": prompt }]
                    }),
                ),
                _ => {
                    let mut b = client.post(&endpoint);
                    if !cred.api_key.trim().is_empty() {
                        b = b.bearer_auth(cred.api_key.as_str());
                    }
                    (
                        b,
                        json!({
                            "model": model,
                            "messages": [{ "role": "user", "content": prompt }]
                        }),
                    )
                }
            };

            match req_builder.json(&payload).send().await {
                Ok(resp) => {
                    let code = resp.status().as_u16();
                    let retry_after = parse_retry_after_secs(resp.headers());
                    let body_text = match read_bounded_response_text(resp).await {
                        Ok(t) => t,
                        Err(e) => {
                            last_err = redact_secrets_from_text(&e.to_string(), &all_secrets);
                            failover_providers.push(provider);
                            continue;
                        }
                    };

                    if (200..300).contains(&code) {
                        self.cooldowns.clear(tenant_id, &provider);
                        let parsed = serde_json::from_str::<Value>(&body_text).ok();
                        let content = parsed
                            .as_ref()
                            .and_then(|v| extract_text_from_provider_json(&provider, v))
                            .unwrap_or(body_text);
                        let safe_content = redact_secrets_from_text(&content, &all_secrets);
                        return Ok(ByokExecutionResult {
                            provider_id: provider,
                            model_id: model,
                            raw_content: safe_content,
                            cooldown_applied,
                            failover_providers,
                            duration_ms: start.elapsed().as_millis() as u64,
                            http_status: code,
                        });
                    }

                    if code == 429 {
                        let applied_secs = self.cooldowns.record_rate_limit(
                            tenant_id,
                            &provider,
                            retry_after,
                            Some("HTTP 429 rate limited".to_string()),
                        );
                        cooldown_applied = true;
                        failover_providers.push(provider.clone());
                        last_err = format!(
                            "Provider '{provider}' rate-limited (HTTP 429, Retry-After={applied_secs}s)"
                        );
                        continue;
                    }

                    let safe_snippet = redact_secrets_from_text(
                        &body_text.chars().take(180).collect::<String>(),
                        &all_secrets,
                    );
                    failover_providers.push(provider.clone());
                    last_err =
                        format!("Provider '{provider}' returned HTTP {code}: {safe_snippet}");
                }
                Err(err) => {
                    let safe_err = redact_secrets_from_text(&err.to_string(), &all_secrets);
                    failover_providers.push(provider.clone());
                    last_err = format!("Provider '{provider}' request failed: {safe_err}");
                }
            }
        }

        let min_retry = self
            .cooldowns
            .min_remaining_cooldown_secs(tenant_id, &failover_providers);
        Err(BotFleetError::RateLimitedOrUnavailable {
            message: redact_secrets_from_text(&last_err, &all_secrets),
            cooldown_applied,
            failover_providers,
            retry_after_secs: min_retry,
        })
    }
}
