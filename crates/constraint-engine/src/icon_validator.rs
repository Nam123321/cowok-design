// @story 03.2
// @implements AC-01, AC-02, AC-03
use domain::HarnessError;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ts")]
use ts_rs::TS;

/// Embedded canonical flagship 143-icon catalog from cowok-brand-product-system-v4.3
pub const FLAGSHIP_ICON_MANIFEST_JSON: &str = include_str!(
    "../../../Brand-guideline/cowok-brand-product-system-v4.3/iconography/icon-manifest.json"
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct IconManifestEntry {
    pub name: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct IconManifest {
    pub name: String,
    #[serde(default)]
    pub status: Option<String>,
    pub count: usize,
    #[serde(default)]
    pub icons: Vec<IconManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct IconValidationReport {
    pub requested_icon: String,
    pub is_valid: bool,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct TypographyContract {
    pub allowed_font_families: Vec<String>,
    pub allowed_font_weights: Vec<u32>,
    pub allowed_font_sizes: Vec<String>,
    pub min_wcag_contrast: f64,
}

impl Default for TypographyContract {
    fn default() -> Self {
        Self {
            allowed_font_families: vec![
                "Geist".to_string(),
                "Inter".to_string(),
                "system-ui".to_string(),
                "monospace".to_string(),
            ],
            allowed_font_weights: vec![400, 500, 600, 700],
            allowed_font_sizes: vec![
                "12px".to_string(),
                "14px".to_string(),
                "16px".to_string(),
                "18px".to_string(),
                "20px".to_string(),
                "24px".to_string(),
                "32px".to_string(),
            ],
            min_wcag_contrast: 4.5,
        }
    }
}

pub struct IconValidator;

impl IconValidator {
    /// Parses an icon manifest JSON string.
    pub fn parse_manifest(json_str: &str) -> Result<IconManifest, HarnessError> {
        serde_json::from_str::<IconManifest>(json_str).map_err(|e| HarnessError::ValidationFailed {
            reason: format!("Failed to parse icon manifest JSON: {}", e),
        })
    }

    /// Loads the embedded flagship cowok-brand-product-system-v4.3 143-icon catalog.
    pub fn get_flagship_manifest() -> IconManifest {
        Self::parse_manifest(FLAGSHIP_ICON_MANIFEST_JSON)
            .expect("Embedded flagship icon manifest must be valid JSON")
    }

    /// Loads an icon manifest from a file path on disk.
    pub fn load_from_file(path: &std::path::Path) -> Result<IconManifest, HarnessError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| HarnessError::ValidationFailed {
                reason: format!(
                    "Failed to read icon manifest from '{}': {e}",
                    path.display()
                ),
            })?;
        Self::parse_manifest(&content)
    }

    /// Validates an icon name against the given manifest.
    /// If the icon is not found, calculates normalized Levenshtein distances and
    /// returns the Top-3 nearest valid suggestions (AC-03).
    pub fn validate_icon(manifest: &IconManifest, requested_icon: &str) -> IconValidationReport {
        let norm_req = requested_icon.trim().to_lowercase();

        if norm_req.is_empty() || norm_req.len() > 64 {
            let default_suggestions = manifest
                .icons
                .iter()
                .take(3)
                .map(|i| i.name.clone())
                .collect();
            return IconValidationReport {
                requested_icon: requested_icon.to_string(),
                is_valid: false,
                suggestions: default_suggestions,
            };
        }

        // Direct match
        if manifest
            .icons
            .iter()
            .any(|i| i.name.to_lowercase() == norm_req)
        {
            return IconValidationReport {
                requested_icon: requested_icon.to_string(),
                is_valid: true,
                suggestions: Vec::new(),
            };
        }

        // Compute Levenshtein distance against all available icons in the manifest
        let mut scored: Vec<(&str, usize, bool)> = manifest
            .icons
            .iter()
            .map(|entry| {
                let name = entry.name.as_str();
                let lower_name = name.to_lowercase();
                let dist = Self::levenshtein_distance(&norm_req, &lower_name);
                let is_prefix =
                    lower_name.starts_with(&norm_req) || norm_req.starts_with(&lower_name);
                (name, dist, is_prefix)
            })
            .collect();

        // Sort by distance ascending, breaking ties by prefix matching, then lexicographically
        scored.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| a.0.cmp(b.0))
        });

        let suggestions: Vec<String> = scored
            .into_iter()
            .take(3)
            .map(|(name, _, _)| name.to_string())
            .collect();

        IconValidationReport {
            requested_icon: requested_icon.to_string(),
            is_valid: false,
            suggestions,
        }
    }

    /// Efficient Wagner-Fischer algorithm for Levenshtein distance with stack arrays for length <= 64.
    pub fn levenshtein_distance(a: &str, b: &str) -> usize {
        let m = a.chars().count();
        let n = b.chars().count();

        if m == 0 {
            return n;
        }
        if n == 0 {
            return m;
        }

        if m > 64 || n > 64 {
            let a_chars: Vec<char> = a.chars().collect();
            let b_chars: Vec<char> = b.chars().collect();
            let mut prev_row: Vec<usize> = (0..=n).collect();
            let mut curr_row: Vec<usize> = vec![0; n + 1];

            for i in 1..=m {
                curr_row[0] = i;
                for j in 1..=n {
                    let cost = if a_chars[i - 1] == b_chars[j - 1] {
                        0
                    } else {
                        1
                    };
                    curr_row[j] = (prev_row[j] + 1)
                        .min(curr_row[j - 1] + 1)
                        .min(prev_row[j - 1] + cost);
                }
                prev_row.copy_from_slice(&curr_row);
            }
            return prev_row[n];
        }

        let mut a_chars = ['\0'; 65];
        for (i, ch) in a.chars().enumerate() {
            a_chars[i] = ch;
        }
        let mut b_chars = ['\0'; 65];
        for (j, ch) in b.chars().enumerate() {
            b_chars[j] = ch;
        }

        let mut prev_row = [0usize; 65];
        let mut curr_row = [0usize; 65];

        for j in 0..=n {
            prev_row[j] = j;
        }

        for i in 1..=m {
            curr_row[0] = i;
            for j in 1..=n {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                curr_row[j] = (prev_row[j] + 1)
                    .min(curr_row[j - 1] + 1)
                    .min(prev_row[j - 1] + cost);
            }
            prev_row[..=n].copy_from_slice(&curr_row[..=n]);
        }

        prev_row[n]
    }

    /// Validates a font style against the project typography contract.
    pub fn validate_typography(
        contract: &TypographyContract,
        font_family: &str,
        font_weight: u32,
        font_size: Option<&str>,
        contrast_ratio: f64,
    ) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        let family_clean = font_family.trim().trim_matches('\'').trim_matches('"');
        if !contract
            .allowed_font_families
            .iter()
            .any(|f| f.eq_ignore_ascii_case(family_clean))
        {
            errors.push(format!(
                "Font family '{}' is not in allowed list: {:?}",
                font_family, contract.allowed_font_families
            ));
        }

        if !contract.allowed_font_weights.contains(&font_weight) {
            errors.push(format!(
                "Font weight '{}' is not in allowed list: {:?}",
                font_weight, contract.allowed_font_weights
            ));
        }

        if let Some(size) = font_size {
            let size_clean = size.trim();
            if !contract
                .allowed_font_sizes
                .iter()
                .any(|s| s.eq_ignore_ascii_case(size_clean))
            {
                errors.push(format!(
                    "Font size '{}' is not in allowed list: {:?}",
                    size, contract.allowed_font_sizes
                ));
            }
        }

        if contrast_ratio.is_nan() || contrast_ratio < contract.min_wcag_contrast {
            errors.push(format!(
                "Contrast ratio {:.2} fails minimum WCAG threshold of {:.2}",
                contrast_ratio, contract.min_wcag_contrast
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
