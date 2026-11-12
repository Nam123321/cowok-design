use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[cfg(feature = "ts")]
use ts_rs::TS;

use crate::error::HarnessError;

/// The 9 Foundational Project Context Specs (ADR-001 SSOT Whitelist)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(
        export,
        export_to = "../../../packages/shared-types/src/generated/",
        type = "\"01-product-overview.md\" | \"02-database-spec.md\" | \"03-ui-ux-spec.md\" | \"04-architecture.md\" | \"05-epics-and-stories.md\" | \"06-cross-dependencies.md\" | \"07-api-guidelines.md\" | \"08-security-tenancy.md\" | \"09-data-and-copy.md\""
    )
)]
pub enum CanonicalSpecKey {
    ProductOverview,
    DatabaseSpec,
    UiUxSpec,
    Architecture,
    EpicsAndStories,
    CrossDependencies,
    ApiGuidelines,
    SecurityTenancy,
    DataAndCopy,
}

impl CanonicalSpecKey {
    pub const ALL: [CanonicalSpecKey; 9] = [
        CanonicalSpecKey::ProductOverview,
        CanonicalSpecKey::DatabaseSpec,
        CanonicalSpecKey::UiUxSpec,
        CanonicalSpecKey::Architecture,
        CanonicalSpecKey::EpicsAndStories,
        CanonicalSpecKey::CrossDependencies,
        CanonicalSpecKey::ApiGuidelines,
        CanonicalSpecKey::SecurityTenancy,
        CanonicalSpecKey::DataAndCopy,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProductOverview => "01-product-overview.md",
            Self::DatabaseSpec => "02-database-spec.md",
            Self::UiUxSpec => "03-ui-ux-spec.md",
            Self::Architecture => "04-architecture.md",
            Self::EpicsAndStories => "05-epics-and-stories.md",
            Self::CrossDependencies => "06-cross-dependencies.md",
            Self::ApiGuidelines => "07-api-guidelines.md",
            Self::SecurityTenancy => "08-security-tenancy.md",
            Self::DataAndCopy => "09-data-and-copy.md",
        }
    }

    /// Strictly parses a raw string against the 9 whitelisted keys.
    /// Traversal attempts (`../`, `\`), null bytes, or unknown keys return `HarnessError::InvalidSpecKey`.
    pub fn parse(raw: &str) -> Result<Self, HarnessError> {
        match raw {
            "01-product-overview.md" => Ok(Self::ProductOverview),
            "02-database-spec.md" => Ok(Self::DatabaseSpec),
            "03-ui-ux-spec.md" => Ok(Self::UiUxSpec),
            "04-architecture.md" => Ok(Self::Architecture),
            "05-epics-and-stories.md" => Ok(Self::EpicsAndStories),
            "06-cross-dependencies.md" => Ok(Self::CrossDependencies),
            "07-api-guidelines.md" => Ok(Self::ApiGuidelines),
            "08-security-tenancy.md" => Ok(Self::SecurityTenancy),
            "09-data-and-copy.md" => Ok(Self::DataAndCopy),
            _ => Err(HarnessError::InvalidSpecKey {
                key: raw.to_string(),
            }),
        }
    }
}

impl fmt::Display for CanonicalSpecKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for CanonicalSpecKey {
    type Err = HarnessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for CanonicalSpecKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

struct CanonicalSpecKeyVisitor;

impl<'de> Visitor<'de> for CanonicalSpecKeyVisitor {
    type Value = CanonicalSpecKey;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("one of the 9 canonical spec filenames, e.g. '01-product-overview.md'")
    }

    fn visit_str<E>(self, value: &str) -> Result<CanonicalSpecKey, E>
    where
        E: de::Error,
    {
        CanonicalSpecKey::parse(value).map_err(|_| {
            de::Error::custom(format!(
                "INVALID_SPEC_KEY: '{}' is not one of the 9 canonical spec keys",
                value
            ))
        })
    }
}

impl<'de> Deserialize<'de> for CanonicalSpecKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CanonicalSpecKeyVisitor)
    }
}
