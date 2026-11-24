// @story 11.1
// @implements AC-02, AC-03 (Unified BYOK Gateway, DNS-Pinned SSRF Hardening, Tenant-Scoped 429 Cooldown & Bounded Stream)
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::adapters::validate_model_id;
use crate::model_registry::{
    builtin_seed_models, resolve_cache_strategy, resolve_model_for_provider, CacheStrategy,
    ModelRegistryEntry,
};
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

/// Resolves the dynamic default model ID for `provider_id` using `model_registry` entries
/// (or built-in JSON seed defaults when `entries` is empty).
pub fn resolve_default_model_id(entries: &[ModelRegistryEntry], provider_id: &str) -> String {
    resolve_model_for_provider(entries, provider_id)
        .map(|entry| entry.model_id)
        .unwrap_or_else(|| "gpt-4.1".to_string())
}

/// Backward-compatible static helper delegating directly to `builtin_seed_models()`
/// (zero duplicate hardcoded provider match table).
pub fn default_model_for_provider(provider_id: &str) -> &'static str {
    static SEED_MODELS: OnceLock<Vec<ModelRegistryEntry>> = OnceLock::new();
    let seeds = SEED_MODELS.get_or_init(builtin_seed_models);
    let canon = canonical_provider_id(provider_id);
    seeds
        .iter()
        .find(|e| {
            e.enabled && e.is_default_for_provider && canonical_provider_id(&e.provider_id) == canon
        })
        .or_else(|| {
            seeds
                .iter()
                .find(|e| e.enabled && canonical_provider_id(&e.provider_id) == canon)
        })
        .map(|e| e.model_id.as_str())
        .unwrap_or("gpt-4.1")
}

/// Default TTL (`1 hour`) for Gemini explicit `cachedContents` handles.
pub const DEFAULT_GEMINI_CACHE_TTL_SECS: u64 = 3600;
const MAX_GEMINI_CACHE_HANDLES: usize = 64;

/// Request specification for creating a Gemini explicit `cachedContents` resource (`POST /v1beta/cachedContents`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCachedContentCreateRequest {
    pub model: String,
    pub display_name: String,
    pub system_instruction: String,
    pub ttl_secs: u64,
}

/// Builds the wire-compliant Gemini `cachedContents.create` JSON payload with bounded TTL (`60s..=86400s`).
pub fn build_gemini_cached_content_create_payload(req: &GeminiCachedContentCreateRequest) -> Value {
    let model_path = if req.model.trim().starts_with("models/") {
        req.model.trim().to_string()
    } else {
        format!("models/{}", req.model.trim())
    };
    let ttl_secs = req.ttl_secs.clamp(60, 86_400);
    json!({
        "model": model_path,
        "displayName": req.display_name.trim(),
        "systemInstruction": {
            "parts": [{ "text": req.system_instruction }]
        },
        "ttl": format!("{ttl_secs}s")
    })
}

/// Resolves the Gemini `cachedContents` creation endpoint for a given BYOK credential.
pub fn resolve_gemini_cached_contents_endpoint(cred: &ByokCredential) -> String {
    if let Some(ref custom) = cred.endpoint_url {
        let trimmed = custom.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            if let Some((base, _)) = trimmed.split_once("/models/") {
                return format!("{base}/cachedContents");
            }
            if trimmed.ends_with("/cachedContents") {
                return trimmed.to_string();
            }
            return format!("{trimmed}/cachedContents");
        }
    }
    "https://generativelanguage.googleapis.com/v1beta/cachedContents".to_string()
}

#[derive(Debug, Clone)]
pub struct GeminiCachedContentHandle {
    pub name: String,
    pub catalog_sha256: String,
    pub expires_at: Instant,
}

type GeminiCacheLookupKey = (String, String, String, String);

#[derive(Debug, Default)]
struct GeminiCacheRegistryInner {
    order: VecDeque<GeminiCacheLookupKey>,
    entries: HashMap<GeminiCacheLookupKey, GeminiCachedContentHandle>,
}

/// Bounded in-memory TTL registry for Gemini `cachedContents/*` handles keyed by
/// `(tenant_id, project_id, model_id, catalog_sha256)`.
#[derive(Debug, Clone, Default)]
pub struct GeminiCacheHandleRegistry {
    inner: Arc<RwLock<GeminiCacheRegistryInner>>,
}

impl GeminiCacheHandleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_live(
        &self,
        tenant_id: &str,
        project_id: &str,
        model_id: &str,
        catalog_sha256: &str,
    ) -> Option<String> {
        let key = (
            tenant_id.trim().to_string(),
            project_id.trim().to_string(),
            model_id.trim().to_string(),
            catalog_sha256.trim().to_string(),
        );
        let now = Instant::now();
        let guard = self.inner.read().unwrap_or_else(|e| e.into_inner());
        guard
            .entries
            .get(&key)
            .filter(|h| h.expires_at > now)
            .map(|h| h.name.clone())
    }

    pub fn insert(
        &self,
        tenant_id: &str,
        project_id: &str,
        model_id: &str,
        catalog_sha256: &str,
        name: String,
        ttl_secs: u64,
    ) {
        let key = (
            tenant_id.trim().to_string(),
            project_id.trim().to_string(),
            model_id.trim().to_string(),
            catalog_sha256.trim().to_string(),
        );
        let handle = GeminiCachedContentHandle {
            name,
            catalog_sha256: catalog_sha256.trim().to_string(),
            expires_at: Instant::now() + Duration::from_secs(ttl_secs.clamp(60, 86_400)),
        };
        let mut guard = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if !guard.entries.contains_key(&key) {
            while guard.order.len() >= MAX_GEMINI_CACHE_HANDLES {
                if let Some(evicted) = guard.order.pop_front() {
                    guard.entries.remove(&evicted);
                }
            }
            guard.order.push_back(key.clone());
        }
        guard.entries.insert(key, handle);
    }

    pub fn invalidate_tenant(&self, tenant_id: &str) {
        let target = tenant_id.trim();
        let mut guard = self.inner.write().unwrap_or_else(|e| e.into_inner());
        guard.order.retain(|k| k.0 != target);
        guard.entries.retain(|k, _| k.0 != target);
    }
}

/// Process-shared Gemini `cachedContents/*` handle registry used by live daemon BYOK routers
/// and invalidated alongside `UnifiedSkillCatalog` reloads.
pub fn shared_gemini_cache_registry() -> GeminiCacheHandleRegistry {
    static REGISTRY: OnceLock<GeminiCacheHandleRegistry> = OnceLock::new();
    REGISTRY.get_or_init(GeminiCacheHandleRegistry::new).clone()
}

/// Wire-native prompt caching metadata passed from the 2-Breakpoint Prompt Compiler (`Story 08.7`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCacheHints {
    pub project_id: String,
    pub catalog_sha256: String,
    #[serde(default)]
    pub zone1_static_prefix: Option<String>,
    #[serde(default)]
    pub cached_content_name: Option<String>,
    #[serde(default)]
    pub history_breakpoint_block: Option<String>,
}

/// Zero-PII provider cache usage counters extracted from upstream responses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCacheTelemetry {
    pub cache_strategy: String,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub fallback_uncached: bool,
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
    pub cache_telemetry: PromptCacheTelemetry,
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

fn sanitize_cache_key_segment(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(48)
        .collect();
    if cleaned.is_empty() {
        "default".to_string()
    } else {
        cleaned
    }
}

/// Deterministic OpenAI `prompt_cache_key` derived from `(project_id, catalog_sha256[..16])`.
pub fn format_openai_prompt_cache_key(project_id: &str, catalog_sha256: &str) -> String {
    let proj = sanitize_cache_key_segment(project_id);
    let prefix: String = catalog_sha256
        .trim()
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(16)
        .collect();
    let hash_part = if prefix.is_empty() {
        "0000000000000000".to_string()
    } else {
        prefix.to_ascii_lowercase()
    };
    format!("cw:{proj}:{hash_part}")
}

/// Builds provider-specific wire-native JSON payloads supporting 2-breakpoint prompt caching:
/// - **Anthropic (`claude`)**: `system` array with `cache_control: { "type": "ephemeral" }` + 2nd breakpoint on tool/history tail.
/// - **Gemini (`gemini`)**: `cachedContent` handle field when present + `systemInstruction` separation.
/// - **OpenAI / OpenRouter**: deterministic `prompt_cache_key` (`cw:<project_id>:<catalog_sha256[..16]>`) + separated `system` prefix message.
pub fn build_provider_wire_payload(
    provider: &str,
    model: &str,
    prompt: &str,
    cache_strategy: CacheStrategy,
    cache_hints: Option<&PromptCacheHints>,
) -> Value {
    let zone1_opt = cache_hints
        .and_then(|h| h.zone1_static_prefix.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let history_opt = cache_hints
        .and_then(|h| h.history_breakpoint_block.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let combined_user_prompt = || -> String {
        if let Some(hist) = history_opt {
            if !prompt.contains(hist) {
                return format!("{hist}\n\n{prompt}");
            }
        }
        prompt.to_string()
    };

    match provider {
        "gemini" => {
            let mut obj = serde_json::Map::new();
            let mut has_cached_content = false;
            if cache_strategy == CacheStrategy::GeminiCachedContent {
                if let Some(handle) = cache_hints
                    .and_then(|h| h.cached_content_name.as_deref())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    obj.insert("cachedContent".to_string(), json!(handle));
                    has_cached_content = true;
                }
            }
            // Gemini API rejects requests that specify both `cachedContent` and `systemInstruction` (HTTP 400).
            // Omit `systemInstruction` whenever `cachedContent` is attached; include it on uncached/fallback calls.
            if !has_cached_content {
                if let Some(z1) = zone1_opt {
                    obj.insert(
                        "systemInstruction".to_string(),
                        json!({ "parts": [{ "text": z1 }] }),
                    );
                }
            }
            obj.insert(
                "contents".to_string(),
                json!([{ "role": "user", "parts": [{ "text": combined_user_prompt() }] }]),
            );
            Value::Object(obj)
        }
        "claude" => {
            if cache_strategy == CacheStrategy::AnthropicEphemeral {
                let mut obj = serde_json::Map::new();
                obj.insert("model".to_string(), json!(model));
                obj.insert("max_tokens".to_string(), json!(2048));
                if let Some(z1) = zone1_opt {
                    obj.insert(
                        "system".to_string(),
                        json!([{
                            "type": "text",
                            "text": z1,
                            "cache_control": { "type": "ephemeral" }
                        }]),
                    );
                }
                let mut user_blocks = Vec::new();
                if let Some(hist) = history_opt {
                    user_blocks.push(json!({
                        "type": "text",
                        "text": hist,
                        "cache_control": { "type": "ephemeral" }
                    }));
                }
                user_blocks.push(json!({
                    "type": "text",
                    "text": prompt
                }));
                obj.insert(
                    "messages".to_string(),
                    json!([{ "role": "user", "content": user_blocks }]),
                );
                Value::Object(obj)
            } else {
                let mut obj = serde_json::Map::new();
                obj.insert("model".to_string(), json!(model));
                obj.insert("max_tokens".to_string(), json!(2048));
                if let Some(z1) = zone1_opt {
                    obj.insert("system".to_string(), json!(z1));
                }
                obj.insert(
                    "messages".to_string(),
                    json!([{ "role": "user", "content": combined_user_prompt() }]),
                );
                Value::Object(obj)
            }
        }
        _ => {
            let mut obj = serde_json::Map::new();
            obj.insert("model".to_string(), json!(model));
            let mut messages = Vec::new();
            if let Some(z1) = zone1_opt {
                messages.push(json!({ "role": "system", "content": z1 }));
            }
            messages.push(json!({ "role": "user", "content": combined_user_prompt() }));
            obj.insert("messages".to_string(), Value::Array(messages));
            if cache_strategy == CacheStrategy::OpenaiCacheKey {
                if let Some(hints) = cache_hints {
                    let key =
                        format_openai_prompt_cache_key(&hints.project_id, &hints.catalog_sha256);
                    obj.insert("prompt_cache_key".to_string(), json!(key));
                }
            }
            Value::Object(obj)
        }
    }
}

/// Extracts Zero-PII cache token usage counters from provider JSON responses.
pub fn extract_cache_telemetry_from_provider_json(
    provider: &str,
    body: Option<&Value>,
    cache_strategy: CacheStrategy,
    fallback_uncached: bool,
) -> PromptCacheTelemetry {
    let mut cache_read_tokens = 0u64;
    let mut cache_creation_tokens = 0u64;
    if let Some(v) = body {
        match provider {
            "claude" => {
                if let Some(usage) = v.get("usage") {
                    cache_read_tokens = usage
                        .get("cache_read_input_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    cache_creation_tokens = usage
                        .get("cache_creation_input_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                }
            }
            "gemini" => {
                if let Some(usage) = v.get("usageMetadata") {
                    cache_read_tokens = usage
                        .get("cachedContentTokenCount")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                }
            }
            _ => {
                if let Some(usage) = v.get("usage") {
                    cache_read_tokens = usage
                        .get("prompt_tokens_details")
                        .and_then(|d| d.get("cached_tokens"))
                        .and_then(Value::as_u64)
                        .or_else(|| usage.get("cache_read_tokens").and_then(Value::as_u64))
                        .unwrap_or(0);
                    cache_creation_tokens = usage
                        .get("cache_creation_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                }
            }
        }
    }
    PromptCacheTelemetry {
        cache_strategy: cache_strategy.as_db_str().to_string(),
        cache_read_tokens,
        cache_creation_tokens,
        fallback_uncached,
    }
}

#[derive(Clone)]
pub struct ByokGatewayRouter {
    cooldowns: CooldownTracker,
    allow_loopback_test_endpoints: bool,
    model_registry: Vec<ModelRegistryEntry>,
    gemini_cache_registry: GeminiCacheHandleRegistry,
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
            model_registry: builtin_seed_models(),
            gemini_cache_registry: shared_gemini_cache_registry(),
        }
    }

    /// Creates a router allowing loopback mock endpoints for cloud providers in offline tests.
    pub fn with_loopback_test_endpoints(cooldowns: CooldownTracker) -> Self {
        Self {
            cooldowns,
            allow_loopback_test_endpoints: true,
            model_registry: builtin_seed_models(),
            gemini_cache_registry: GeminiCacheHandleRegistry::new(),
        }
    }

    /// Attaches live dynamic model registry entries (e.g. loaded from SQLite `model_registry_profiles`).
    pub fn with_model_registry(mut self, entries: Vec<ModelRegistryEntry>) -> Self {
        if !entries.is_empty() {
            self.model_registry = entries;
        }
        self
    }

    /// Attaches an explicit `GeminiCacheHandleRegistry` (defaults to process-shared registry in production).
    pub fn with_gemini_cache_registry(mut self, registry: GeminiCacheHandleRegistry) -> Self {
        self.gemini_cache_registry = registry;
        self
    }

    pub fn cooldowns(&self) -> &CooldownTracker {
        &self.cooldowns
    }

    pub fn model_registry(&self) -> &[ModelRegistryEntry] {
        &self.model_registry
    }

    pub fn gemini_cache_registry(&self) -> &GeminiCacheHandleRegistry {
        &self.gemini_cache_registry
    }

    async fn resolve_or_create_gemini_cached_content(
        &self,
        tenant_id: &str,
        cred: &ByokCredential,
        model: &str,
        hints: &PromptCacheHints,
        timeout: Duration,
    ) -> Option<String> {
        if let Some(existing) = hints
            .cached_content_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(existing.to_string());
        }
        let zone1 = hints
            .zone1_static_prefix
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())?;
        if let Some(live_handle) = self.gemini_cache_registry.get_live(
            tenant_id,
            &hints.project_id,
            model,
            &hints.catalog_sha256,
        ) {
            return Some(live_handle);
        }

        let create_endpoint = resolve_gemini_cached_contents_endpoint(cred);
        let (pinned_host, pinned_addr) = resolve_and_pin_provider_endpoint(
            &create_endpoint,
            "gemini",
            self.allow_loopback_test_endpoints,
        )
        .await
        .ok()?;
        let create_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .resolve(&pinned_host, pinned_addr)
            .timeout(timeout)
            .build()
            .ok()?;

        let create_payload =
            build_gemini_cached_content_create_payload(&GeminiCachedContentCreateRequest {
                model: model.to_string(),
                display_name: format_openai_prompt_cache_key(
                    &hints.project_id,
                    &hints.catalog_sha256,
                ),
                system_instruction: zone1.to_string(),
                ttl_secs: DEFAULT_GEMINI_CACHE_TTL_SECS,
            });
        let resp = Self::build_request_builder(&create_client, "gemini", &create_endpoint, cred)
            .json(&create_payload)
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body_text = read_bounded_response_text(resp).await.ok()?;
        let parsed = serde_json::from_str::<Value>(&body_text).ok()?;
        let handle_name = parsed
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())?
            .to_string();
        self.gemini_cache_registry.insert(
            tenant_id,
            &hints.project_id,
            model,
            &hints.catalog_sha256,
            handle_name.clone(),
            DEFAULT_GEMINI_CACHE_TTL_SECS,
        );
        Some(handle_name)
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
        self.execute_with_failover_and_cache(
            tenant_id,
            prompt,
            preferred_provider,
            model_override,
            credentials,
            timeout_ms,
            None,
        )
        .await
    }

    fn build_request_builder(
        client: &reqwest::Client,
        provider: &str,
        endpoint: &str,
        cred: &ByokCredential,
    ) -> reqwest::RequestBuilder {
        match provider {
            "gemini" => client
                .post(endpoint)
                .header("x-goog-api-key", cred.api_key.as_str()),
            "claude" => client
                .post(endpoint)
                .header("x-api-key", cred.api_key.as_str())
                .header("anthropic-version", "2023-06-01"),
            _ => {
                let mut b = client.post(endpoint);
                if !cred.api_key.trim().is_empty() {
                    b = b.bearer_auth(cred.api_key.as_str());
                }
                b
            }
        }
    }

    /// Routes a prompt with wire-native provider prompt caching (`Story 08.7 AC-03 & AC-05`),
    /// Zero-PII cache token telemetry, and automatic graceful fallback to uncached requests
    /// when a custom endpoint returns HTTP `400`/`501` for unsupported cache parameters.
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_with_failover_and_cache(
        &self,
        tenant_id: &str,
        prompt: &str,
        preferred_provider: Option<&str>,
        model_override: Option<&str>,
        credentials: &[ByokCredential],
        timeout_ms: u64,
        cache_hints: Option<&PromptCacheHints>,
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
                .map(str::to_string)
                .or_else(|| {
                    cred.model_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| resolve_default_model_id(&self.model_registry, &provider));

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

            let initial_strategy = if cache_hints.is_some() {
                resolve_cache_strategy(&self.model_registry, &provider, &model)
            } else {
                CacheStrategy::None
            };

            let mut active_strategy = initial_strategy;
            let mut fallback_uncached = false;
            let mut effective_hints = cache_hints.cloned();

            // Live Gemini explicit `cachedContents` lifecycle (`Story 08.7 AC-03`):
            // reuse TTL handle from `GeminiCacheHandleRegistry`, or POST `/v1beta/cachedContents` on miss,
            // degrading cleanly to uncached `systemInstruction` on creation failure.
            if provider == "gemini" && active_strategy == CacheStrategy::GeminiCachedContent {
                if let Some(ref mut hints) = effective_hints {
                    match self
                        .resolve_or_create_gemini_cached_content(
                            tenant_id, cred, &model, hints, timeout,
                        )
                        .await
                    {
                        Some(handle_name) => {
                            hints.cached_content_name = Some(handle_name);
                        }
                        None => {
                            active_strategy = CacheStrategy::None;
                            fallback_uncached = true;
                        }
                    }
                }
            }

            loop {
                let payload = build_provider_wire_payload(
                    &provider,
                    &model,
                    prompt,
                    active_strategy,
                    effective_hints.as_ref(),
                );
                let req_builder = Self::build_request_builder(&client, &provider, &endpoint, cred);

                match req_builder.json(&payload).send().await {
                    Ok(resp) => {
                        let code = resp.status().as_u16();
                        let retry_after = parse_retry_after_secs(resp.headers());
                        let body_text = match read_bounded_response_text(resp).await {
                            Ok(t) => t,
                            Err(e) => {
                                last_err = redact_secrets_from_text(&e.to_string(), &all_secrets);
                                failover_providers.push(provider.clone());
                                break;
                            }
                        };

                        if (200..300).contains(&code) {
                            self.cooldowns.clear(tenant_id, &provider);
                            let parsed = serde_json::from_str::<Value>(&body_text).ok();
                            let cache_telemetry = extract_cache_telemetry_from_provider_json(
                                &provider,
                                parsed.as_ref(),
                                active_strategy,
                                fallback_uncached,
                            );
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
                                cache_telemetry,
                            });
                        }

                        // AC-05 graceful fallback: retry uncached once when custom endpoint rejects cache params (400/501)
                        // or when a Gemini cachedContents handle expired upstream (404).
                        let is_cache_rejection = code == 400
                            || code == 501
                            || (code == 404
                                && active_strategy == CacheStrategy::GeminiCachedContent);
                        if is_cache_rejection
                            && active_strategy != CacheStrategy::None
                            && !fallback_uncached
                        {
                            if active_strategy == CacheStrategy::GeminiCachedContent {
                                self.gemini_cache_registry.invalidate_tenant(tenant_id);
                            }
                            active_strategy = CacheStrategy::None;
                            fallback_uncached = true;
                            continue;
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
                            break;
                        }

                        let safe_snippet = redact_secrets_from_text(
                            &body_text.chars().take(180).collect::<String>(),
                            &all_secrets,
                        );
                        failover_providers.push(provider.clone());
                        last_err =
                            format!("Provider '{provider}' returned HTTP {code}: {safe_snippet}");
                        break;
                    }
                    Err(err) => {
                        let safe_err = redact_secrets_from_text(&err.to_string(), &all_secrets);
                        failover_providers.push(provider.clone());
                        last_err = format!("Provider '{provider}' request failed: {safe_err}");
                        break;
                    }
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

#[cfg(test)]
mod prompt_cache_wire_tests {
    use super::*;
    use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
    use std::sync::Mutex;

    #[derive(Clone, Default)]
    struct MockServerState {
        captured_payloads: Arc<Mutex<Vec<Value>>>,
        reject_cached_with_status: Option<StatusCode>,
    }

    async fn spawn_mock_endpoint(
        path: &str,
        state: MockServerState,
        response_fn: fn(&Value) -> Value,
    ) -> (String, MockServerState) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback mock");
        let addr = listener.local_addr().unwrap();
        let st_clone = state.clone();
        let app = Router::new()
            .route(
                path,
                post(
                    move |State(st): State<MockServerState>, Json(body): Json<Value>| async move {
                        st.captured_payloads.lock().unwrap().push(body.clone());
                        let has_cache_field = body.get("prompt_cache_key").is_some()
                            || body.get("cachedContent").is_some()
                            || body.get("system").and_then(Value::as_array).is_some();
                        if has_cache_field {
                            if let Some(code) = st.reject_cached_with_status {
                                return (
                                    code,
                                    Json(json!({ "error": "prompt caching unsupported" })),
                                );
                            }
                        }
                        (StatusCode::OK, Json(response_fn(&body)))
                    },
                ),
            )
            .with_state(st_clone);
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://127.0.0.1:{}{}", addr.port(), path), state)
    }

    #[tokio::test]
    async fn emits_wire_native_cache_payloads_for_anthropic_gemini_openai_and_handles_fallback() {
        let hints = PromptCacheHints {
            project_id: "proj-087".into(),
            catalog_sha256: "abcdef0123456789fedcba9876543210abcdef0123456789fedcba9876543210"
                .into(),
            zone1_static_prefix: Some("STATIC_ZONE1_PREFIX".into()),
            cached_content_name: Some("cachedContents/cc-087".into()),
            history_breakpoint_block: Some("HISTORY_BREAKPOINT".into()),
        };
        let router = ByokGatewayRouter::with_loopback_test_endpoints(CooldownTracker::new());

        // 1. Anthropic 2-breakpoint ephemeral cache_control + cache_read_input_tokens telemetry
        let (claude_url, claude_state) =
            spawn_mock_endpoint("/v1/messages", MockServerState::default(), |_| {
                json!({
                    "content": [{ "type": "text", "text": "claude-ok" }],
                    "usage": { "cache_read_input_tokens": 1240, "cache_creation_input_tokens": 310 }
                })
            })
            .await;
        let claude_res = router
            .execute_with_failover_and_cache(
                "tenant-a",
                "dynamic turn",
                Some("claude"),
                None,
                &[ByokCredential {
                    provider_id: "claude".into(),
                    api_key: Zeroizing::new("sk-ant-test".into()),
                    endpoint_url: Some(claude_url),
                    model_id: None,
                }],
                5000,
                Some(&hints),
            )
            .await
            .expect("claude cached call");
        assert_eq!(claude_res.raw_content, "claude-ok");
        assert_eq!(claude_res.cache_telemetry.cache_read_tokens, 1240);
        assert_eq!(claude_res.cache_telemetry.cache_creation_tokens, 310);
        assert!(!claude_res.cache_telemetry.fallback_uncached);
        let claude_req = claude_state.captured_payloads.lock().unwrap()[0].clone();
        assert_eq!(
            claude_req["system"][0]["cache_control"]["type"],
            json!("ephemeral")
        );
        assert_eq!(
            claude_req["messages"][0]["content"][0]["cache_control"]["type"],
            json!("ephemeral")
        );

        // 2. Gemini cachedContent handle + systemInstruction separation
        let (gem_url, gem_state) = spawn_mock_endpoint(
            "/models/gemini-2.5-pro:generateContent",
            MockServerState::default(),
            |_| {
                json!({
                    "candidates": [{ "content": { "parts": [{ "text": "gemini-ok" }] } }],
                    "usageMetadata": { "cachedContentTokenCount": 980 }
                })
            },
        )
        .await;
        let gem_res = router
            .execute_with_failover_and_cache(
                "tenant-a",
                "dynamic turn",
                Some("gemini"),
                None,
                &[ByokCredential {
                    provider_id: "gemini".into(),
                    api_key: Zeroizing::new("gem-test".into()),
                    endpoint_url: Some(gem_url),
                    model_id: None,
                }],
                5000,
                Some(&hints),
            )
            .await
            .expect("gemini cached call");
        assert_eq!(gem_res.cache_telemetry.cache_read_tokens, 980);
        let gem_req = gem_state.captured_payloads.lock().unwrap()[0].clone();
        assert_eq!(gem_req["cachedContent"], json!("cachedContents/cc-087"));
        assert!(
            gem_req.get("systemInstruction").is_none(),
            "Gemini wire payload MUST omit systemInstruction when cachedContent is set to avoid HTTP 400"
        );

        // Verify Gemini uncached payload DOES include systemInstruction and create payload builder works
        let gem_uncached = build_provider_wire_payload(
            "gemini",
            "gemini-2.5-pro",
            "dynamic turn",
            CacheStrategy::None,
            Some(&hints),
        );
        assert!(gem_uncached.get("cachedContent").is_none());
        assert_eq!(
            gem_uncached["systemInstruction"]["parts"][0]["text"],
            json!("STATIC_ZONE1_PREFIX")
        );
        let create_payload =
            build_gemini_cached_content_create_payload(&GeminiCachedContentCreateRequest {
                model: "gemini-2.5-pro".into(),
                display_name: "cw-proj-087".into(),
                system_instruction: "STATIC_ZONE1_PREFIX".into(),
                ttl_secs: DEFAULT_GEMINI_CACHE_TTL_SECS,
            });
        assert_eq!(create_payload["model"], json!("models/gemini-2.5-pro"));
        assert_eq!(create_payload["ttl"], json!("3600s"));
        assert_eq!(
            create_payload["systemInstruction"]["parts"][0]["text"],
            json!("STATIC_ZONE1_PREFIX")
        );

        // 3. OpenAI prompt_cache_key + 400 fallback to uncached succeeds seamlessly
        let (oai_url, oai_state) = spawn_mock_endpoint(
            "/chat/completions",
            MockServerState {
                captured_payloads: Arc::new(Mutex::new(Vec::new())),
                reject_cached_with_status: Some(StatusCode::BAD_REQUEST),
            },
            |_| {
                json!({
                    "choices": [{ "message": { "content": "openai-fallback-ok" } }],
                    "usage": { "prompt_tokens_details": { "cached_tokens": 0 } }
                })
            },
        )
        .await;
        let oai_res = router
            .execute_with_failover_and_cache(
                "tenant-a",
                "dynamic turn",
                Some("openai"),
                None,
                &[ByokCredential {
                    provider_id: "openai".into(),
                    api_key: Zeroizing::new("sk-oai-test".into()),
                    endpoint_url: Some(oai_url),
                    model_id: None,
                }],
                5000,
                Some(&hints),
            )
            .await
            .expect("openai fallback succeeds");
        assert_eq!(oai_res.raw_content, "openai-fallback-ok");
        assert!(oai_res.cache_telemetry.fallback_uncached);
        let reqs = oai_state.captured_payloads.lock().unwrap();
        assert_eq!(reqs.len(), 2);
        assert_eq!(
            reqs[0]["prompt_cache_key"],
            json!("cw:proj-087:abcdef0123456789")
        );
        assert!(reqs[1].get("prompt_cache_key").is_none());
    }

    #[tokio::test]
    async fn gemini_cached_contents_lifecycle_creates_once_reuses_on_turn2_and_falls_back_on_error()
    {
        #[derive(Clone, Default)]
        struct GeminiLifecycleState {
            create_payloads: Arc<Mutex<Vec<Value>>>,
            generate_payloads: Arc<Mutex<Vec<Value>>>,
            fail_create: bool,
        }

        async fn spawn_gemini_mock(state: GeminiLifecycleState) -> (String, GeminiLifecycleState) {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind gemini lifecycle mock");
            let addr = listener.local_addr().unwrap();
            let app = Router::new()
                .route(
                    "/v1beta/cachedContents",
                    post(
                        |State(st): State<GeminiLifecycleState>, Json(body): Json<Value>| async move {
                            st.create_payloads.lock().unwrap().push(body);
                            if st.fail_create {
                                return (
                                    StatusCode::NOT_IMPLEMENTED,
                                    Json(json!({ "error": "cachedContents unsupported" })),
                                );
                            }
                            let seq = st.create_payloads.lock().unwrap().len();
                            (
                                StatusCode::OK,
                                Json(json!({ "name": format!("cachedContents/live-handle-{seq}") })),
                            )
                        },
                    ),
                )
                .route(
                    "/v1beta/models/gemini-2.5-pro:generateContent",
                    post(
                        |State(st): State<GeminiLifecycleState>, Json(body): Json<Value>| async move {
                            st.generate_payloads.lock().unwrap().push(body.clone());
                            let cached_tokens = if body.get("cachedContent").is_some() {
                                1500
                            } else {
                                0
                            };
                            (
                                StatusCode::OK,
                                Json(json!({
                                    "candidates": [{ "content": { "parts": [{ "text": "gemini-live-ok" }] } }],
                                    "usageMetadata": { "cachedContentTokenCount": cached_tokens }
                                })),
                            )
                        },
                    ),
                )
                .with_state(state.clone());
            tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            (
                format!(
                    "http://127.0.0.1:{}/v1beta/models/gemini-2.5-pro:generateContent",
                    addr.port()
                ),
                state,
            )
        }

        let (gem_url, gem_state) = spawn_gemini_mock(GeminiLifecycleState::default()).await;
        let router = ByokGatewayRouter::with_loopback_test_endpoints(CooldownTracker::new());
        let cred = ByokCredential {
            provider_id: "gemini".into(),
            api_key: Zeroizing::new("gem-lifecycle-key".into()),
            endpoint_url: Some(gem_url),
            model_id: Some("gemini-2.5-pro".into()),
        };
        let hints = PromptCacheHints {
            project_id: "proj-gem-live".into(),
            catalog_sha256: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                .into(),
            zone1_static_prefix: Some("ZONE1_STATIC_PREFIX_BODY".into()),
            cached_content_name: None,
            history_breakpoint_block: Some("[CONVERSATION_HISTORY]\n[user]: hi".into()),
        };

        // Turn 1: cached_content_name is None -> POST /v1beta/cachedContents creates handle once
        let t1 = router
            .execute_with_failover_and_cache(
                "tenant-gem",
                "turn 1 dynamic",
                Some("gemini"),
                None,
                std::slice::from_ref(&cred),
                5000,
                Some(&hints),
            )
            .await
            .expect("turn 1 succeeds");
        assert_eq!(t1.cache_telemetry.cache_read_tokens, 1500);
        assert!(!t1.cache_telemetry.fallback_uncached);
        assert_eq!(gem_state.create_payloads.lock().unwrap().len(), 1);
        let gen1 = gem_state.generate_payloads.lock().unwrap()[0].clone();
        assert_eq!(gen1["cachedContent"], json!("cachedContents/live-handle-1"));
        assert!(gen1.get("systemInstruction").is_none());
        assert!(gen1["contents"][0]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("[CONVERSATION_HISTORY]"));

        // Turn 2: same (tenant, project, model, catalog_sha256) reuses cached handle without another POST /cachedContents
        let t2 = router
            .execute_with_failover_and_cache(
                "tenant-gem",
                "turn 2 dynamic",
                Some("gemini"),
                None,
                std::slice::from_ref(&cred),
                5000,
                Some(&hints),
            )
            .await
            .expect("turn 2 succeeds");
        assert_eq!(t2.cache_telemetry.cache_read_tokens, 1500);
        assert_eq!(
            gem_state.create_payloads.lock().unwrap().len(),
            1,
            "Turn 2 MUST reuse Gemini cachedContents handle from registry"
        );
        let gen2 = gem_state.generate_payloads.lock().unwrap()[1].clone();
        assert_eq!(gen2["cachedContent"], json!("cachedContents/live-handle-1"));

        // Invalidate tenant -> Turn 3 creates a fresh handle
        router
            .gemini_cache_registry()
            .invalidate_tenant("tenant-gem");
        let _t3 = router
            .execute_with_failover_and_cache(
                "tenant-gem",
                "turn 3 dynamic",
                Some("gemini"),
                None,
                std::slice::from_ref(&cred),
                5000,
                Some(&hints),
            )
            .await
            .expect("turn 3 after invalidation succeeds");
        assert_eq!(gem_state.create_payloads.lock().unwrap().len(), 2);
        let gen3 = gem_state.generate_payloads.lock().unwrap()[2].clone();
        assert_eq!(gen3["cachedContent"], json!("cachedContents/live-handle-2"));

        // Fallback test: when POST /cachedContents returns 501, degrade cleanly to uncached systemInstruction
        let (fb_url, fb_state) = spawn_gemini_mock(GeminiLifecycleState {
            fail_create: true,
            ..Default::default()
        })
        .await;
        let fb_cred = ByokCredential {
            provider_id: "gemini".into(),
            api_key: Zeroizing::new("gem-fallback-key".into()),
            endpoint_url: Some(fb_url),
            model_id: Some("gemini-2.5-pro".into()),
        };
        let fb_res = router
            .execute_with_failover_and_cache(
                "tenant-gem-fb",
                "turn fallback",
                Some("gemini"),
                None,
                &[fb_cred],
                5000,
                Some(&hints),
            )
            .await
            .expect("fallback call succeeds");
        assert!(fb_res.cache_telemetry.fallback_uncached);
        assert_eq!(fb_res.cache_telemetry.cache_strategy, "none");
        let gen_fb = fb_state.generate_payloads.lock().unwrap()[0].clone();
        assert!(gen_fb.get("cachedContent").is_none());
        assert_eq!(
            gen_fb["systemInstruction"]["parts"][0]["text"],
            json!("ZONE1_STATIC_PREFIX_BODY")
        );
    }
}
