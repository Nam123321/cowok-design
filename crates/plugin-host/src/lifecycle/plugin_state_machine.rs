use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    Discovered,
    Validated,
    Installed,
    Active,
    Deprecated,
    Archived,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PluginLifecycleError {
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition { from: PluginState, to: PluginState },
    #[error("Breaking change detected: keeping active version {active_version} and isolating {new_version} in staging")]
    BreakingChangeIsolated {
        active_version: String,
        new_version: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub state: PluginState,
    pub breaking_change_isolated: bool,
    pub staging_version: Option<String>,
    pub updated_at: i64,
}

impl ManagedPlugin {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            state: PluginState::Discovered,
            breaking_change_isolated: false,
            staging_version: None,
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Transitions between valid 6-state lifecycle stages
    pub fn transition_to(&mut self, next: PluginState) -> Result<(), PluginLifecycleError> {
        let valid = match (self.state, next) {
            (PluginState::Discovered, PluginState::Validated) => true,
            (PluginState::Validated, PluginState::Installed) => true,
            (PluginState::Installed, PluginState::Active) => true,
            (PluginState::Active, PluginState::Deprecated) => true,
            (PluginState::Deprecated, PluginState::Archived) => true,
            (PluginState::Active, PluginState::Archived) => true,
            (PluginState::Discovered, PluginState::Archived) => true,
            // Re-activation
            (PluginState::Deprecated, PluginState::Active) => true,
            _ => false,
        };

        if !valid {
            return Err(PluginLifecycleError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }

        self.state = next;
        self.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }

    /// Handles incoming version update with AC-03 Breaking Change boundary guard
    pub fn apply_update(
        &mut self,
        new_version: &str,
        is_breaking: bool,
    ) -> Result<(), PluginLifecycleError> {
        if is_breaking && self.state == PluginState::Active {
            // Keep active version intact and isolate candidate into staging
            self.breaking_change_isolated = true;
            self.staging_version = Some(new_version.to_string());
            self.updated_at = chrono::Utc::now().timestamp();
            return Err(PluginLifecycleError::BreakingChangeIsolated {
                active_version: self.version.clone(),
                new_version: new_version.to_string(),
            });
        }

        self.version = new_version.to_string();
        self.breaking_change_isolated = false;
        self.staging_version = None;
        self.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_lifecycle_transitions() {
        let mut p = ManagedPlugin::new("plug-1", "sdi-clarifier", "1.0.0");
        assert_eq!(p.state, PluginState::Discovered);

        p.transition_to(PluginState::Validated).unwrap();
        assert_eq!(p.state, PluginState::Validated);

        p.transition_to(PluginState::Installed).unwrap();
        assert_eq!(p.state, PluginState::Installed);

        p.transition_to(PluginState::Active).unwrap();
        assert_eq!(p.state, PluginState::Active);

        p.transition_to(PluginState::Deprecated).unwrap();
        assert_eq!(p.state, PluginState::Deprecated);

        p.transition_to(PluginState::Archived).unwrap();
        assert_eq!(p.state, PluginState::Archived);
    }

    #[test]
    fn test_rejects_invalid_transition() {
        let mut p = ManagedPlugin::new("plug-2", "taste-guard", "1.0.0");
        let err = p.transition_to(PluginState::Active).unwrap_err();
        assert_eq!(
            err,
            PluginLifecycleError::InvalidTransition {
                from: PluginState::Discovered,
                to: PluginState::Active
            }
        );
    }

    #[test]
    fn test_ac03_breaking_change_isolation() {
        let mut p = ManagedPlugin::new("plug-3", "spec-syn", "1.0.0");
        p.transition_to(PluginState::Validated).unwrap();
        p.transition_to(PluginState::Installed).unwrap();
        p.transition_to(PluginState::Active).unwrap();

        // Breaking update 2.0.0
        let res = p.apply_update("2.0.0", true);
        assert!(res.is_err());
        assert_eq!(p.version, "1.0.0"); // Active version preserved!
        assert!(p.breaking_change_isolated);
        assert_eq!(p.staging_version.as_deref(), Some("2.0.0"));
    }
}
