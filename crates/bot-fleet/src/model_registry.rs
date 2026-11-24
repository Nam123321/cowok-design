//! Dynamic Model Registry (`Story 08.7 / ADR-009`).
//! Replaces hardcoded `default_model_for_provider` match arms with configurable,
//! data-driven model profiles carrying `supports_prompt_cache`, `cache_strategy`,
//! `context_window`, and `tier` capability metadata.

// @story 08.7
// @implements AC-03

use serde::{Deserialize, Serialize};

use crate::byok_gateway::canonical_provider_id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    Tier1Frontier,
    Tier2Fast,
    Tier3Local,
}

impl ModelTier {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Tier1Frontier => "tier1_frontier",
            Self::Tier2Fast => "tier2_fast",
            Self::Tier3Local => "tier3_local",
        }
    }

    pub fn from_db_str(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "tier1_frontier" => Self::Tier1Frontier,
            "tier2_fast" => Self::Tier2Fast,
            "tier3_local" => Self::Tier3Local,
            _ => Self::Tier2Fast,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStrategy {
    AnthropicEphemeral,
    GeminiCachedContent,
    OpenaiCacheKey,
    None,
}

impl CacheStrategy {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::AnthropicEphemeral => "anthropic_ephemeral",
            Self::GeminiCachedContent => "gemini_cached_content",
            Self::OpenaiCacheKey => "openai_cache_key",
            Self::None => "none",
        }
    }

    pub fn from_db_str(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "anthropic_ephemeral" => Self::AnthropicEphemeral,
            "gemini_cached_content" => Self::GeminiCachedContent,
            "openai_cache_key" => Self::OpenaiCacheKey,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRegistryEntry {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
    pub tier: ModelTier,
    pub context_window: usize,
    pub supports_prompt_cache: bool,
    pub cache_strategy: CacheStrategy,
    pub is_default_for_provider: bool,
    pub enabled: bool,
}

const DEFAULT_MODEL_REGISTRY_SEED_JSON: &str = r#"[
  {
    "id": "seed-claude-sonnet",
    "providerId": "claude",
    "modelId": "claude-sonnet-4-5",
    "displayName": "Claude Sonnet 4.5",
    "tier": "tier1_frontier",
    "contextWindow": 200000,
    "supportsPromptCache": true,
    "cacheStrategy": "anthropic_ephemeral",
    "isDefaultForProvider": true,
    "enabled": true
  },
  {
    "id": "seed-gemini-pro",
    "providerId": "gemini",
    "modelId": "gemini-2.5-pro",
    "displayName": "Gemini 2.5 Pro",
    "tier": "tier1_frontier",
    "contextWindow": 1000000,
    "supportsPromptCache": true,
    "cacheStrategy": "gemini_cached_content",
    "isDefaultForProvider": true,
    "enabled": true
  },
  {
    "id": "seed-openai-gpt41",
    "providerId": "openai",
    "modelId": "gpt-4.1",
    "displayName": "GPT-4.1",
    "tier": "tier1_frontier",
    "contextWindow": 1000000,
    "supportsPromptCache": true,
    "cacheStrategy": "openai_cache_key",
    "isDefaultForProvider": true,
    "enabled": true
  },
  {
    "id": "seed-openrouter-claude",
    "providerId": "openrouter",
    "modelId": "anthropic/claude-sonnet-4",
    "displayName": "OpenRouter Claude Sonnet 4",
    "tier": "tier1_frontier",
    "contextWindow": 200000,
    "supportsPromptCache": true,
    "cacheStrategy": "openai_cache_key",
    "isDefaultForProvider": true,
    "enabled": true
  },
  {
    "id": "seed-deepseek-chat",
    "providerId": "deepseek",
    "modelId": "deepseek-chat",
    "displayName": "DeepSeek Chat",
    "tier": "tier2_fast",
    "contextWindow": 128000,
    "supportsPromptCache": false,
    "cacheStrategy": "none",
    "isDefaultForProvider": true,
    "enabled": true
  },
  {
    "id": "seed-byos-qwen",
    "providerId": "byos",
    "modelId": "qwen2.5-coder:7b",
    "displayName": "Local Qwen 2.5 Coder 7B (BYOS)",
    "tier": "tier3_local",
    "contextWindow": 32768,
    "supportsPromptCache": false,
    "cacheStrategy": "none",
    "isDefaultForProvider": true,
    "enabled": true
  }
]"#;

/// Loads the built-in seed registry entries from embedded JSON (can be overridden by SQLite rows or config JSON).
pub fn builtin_seed_models() -> Vec<ModelRegistryEntry> {
    serde_json::from_str(DEFAULT_MODEL_REGISTRY_SEED_JSON).unwrap_or_default()
}

/// Parses a JSON array of [`ModelRegistryEntry`] overrides (enabling live model updates without rebuilding).
pub fn parse_registry_json(raw: &str) -> Result<Vec<ModelRegistryEntry>, serde_json::Error> {
    serde_json::from_str(raw)
}

/// Lists enabled models from `entries` (optionally filtered by canonical `provider_id`).
pub fn list_registry_models(
    entries: &[ModelRegistryEntry],
    provider_filter: Option<&str>,
) -> Vec<ModelRegistryEntry> {
    let target_provider = provider_filter.map(canonical_provider_id);
    let mut out: Vec<ModelRegistryEntry> = entries
        .iter()
        .filter(|e| e.enabled)
        .filter(|e| match &target_provider {
            Some(pid) => canonical_provider_id(&e.provider_id) == *pid,
            None => true,
        })
        .cloned()
        .collect();
    out.sort_by(|a, b| {
        a.provider_id
            .cmp(&b.provider_id)
            .then_with(|| b.is_default_for_provider.cmp(&a.is_default_for_provider))
            .then_with(|| a.model_id.cmp(&b.model_id))
    });
    out
}

/// Resolves the active default [`ModelRegistryEntry`] for a canonical provider from `entries`,
/// falling back gracefully to `builtin_seed_models()`.
pub fn resolve_model_for_provider(
    entries: &[ModelRegistryEntry],
    provider_id: &str,
) -> Option<ModelRegistryEntry> {
    let canon = canonical_provider_id(provider_id);
    let find_in = |slice: &[ModelRegistryEntry]| -> Option<ModelRegistryEntry> {
        slice
            .iter()
            .find(|e| {
                e.enabled
                    && e.is_default_for_provider
                    && canonical_provider_id(&e.provider_id) == canon
            })
            .or_else(|| {
                slice
                    .iter()
                    .find(|e| e.enabled && canonical_provider_id(&e.provider_id) == canon)
            })
            .cloned()
    };
    find_in(entries).or_else(|| {
        let seeds = builtin_seed_models();
        find_in(&seeds)
    })
}

/// Resolves the [`CacheStrategy`] for a given `(provider_id, model_id)` pair from dynamic registry metadata.
pub fn resolve_cache_strategy(
    entries: &[ModelRegistryEntry],
    provider_id: &str,
    model_id: &str,
) -> CacheStrategy {
    let canon = canonical_provider_id(provider_id);
    let seeds = builtin_seed_models();
    let search = |slice: &[ModelRegistryEntry]| -> Option<CacheStrategy> {
        slice
            .iter()
            .find(|e| {
                e.enabled
                    && canonical_provider_id(&e.provider_id) == canon
                    && e.model_id == model_id
            })
            .or_else(|| {
                slice.iter().find(|e| {
                    e.enabled
                        && e.is_default_for_provider
                        && canonical_provider_id(&e.provider_id) == canon
                })
            })
            .map(|e| {
                if e.supports_prompt_cache {
                    e.cache_strategy
                } else {
                    CacheStrategy::None
                }
            })
    };
    search(entries)
        .or_else(|| search(&seeds))
        .unwrap_or(CacheStrategy::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_seeded_and_overridden_models_dynamically() {
        let seeds = builtin_seed_models();
        assert_eq!(seeds.len(), 6);
        let claude = resolve_model_for_provider(&seeds, "anthropic").unwrap();
        assert_eq!(claude.model_id, "claude-sonnet-4-5");
        assert_eq!(claude.cache_strategy, CacheStrategy::AnthropicEphemeral);

        let custom = vec![ModelRegistryEntry {
            id: "override-openai".into(),
            provider_id: "openai".into(),
            model_id: "gpt-5-preview".into(),
            display_name: "GPT-5 Preview".into(),
            tier: ModelTier::Tier1Frontier,
            context_window: 1_000_000,
            supports_prompt_cache: true,
            cache_strategy: CacheStrategy::OpenaiCacheKey,
            is_default_for_provider: true,
            enabled: true,
        }];
        let resolved = resolve_model_for_provider(&custom, "openai").unwrap();
        assert_eq!(resolved.model_id, "gpt-5-preview");
        assert_eq!(
            resolve_cache_strategy(&custom, "openai", "gpt-5-preview"),
            CacheStrategy::OpenaiCacheKey
        );
    }
}
