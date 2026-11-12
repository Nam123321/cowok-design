use std::path::Path;
use std::sync::mpsc::{self, Receiver};

use domain::{CanonicalSpecKey, HarnessError};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use uuid::Uuid;

use crate::registry::{SpecRegistry, SyncOutcome};
use crate::sync::resolve_jailed_spec_path;

/// Live filesystem watcher for `<workspace_root>/product/specs/` with Hash-Gated Echo Suppression.
pub struct SpecWatcher {
    registry: SpecRegistry,
    project_id: Uuid,
    _watcher: RecommendedWatcher,
    rx: Receiver<notify::Result<Event>>,
}

impl SpecWatcher {
    /// Starts watching `<workspace_root>/product/specs/` for `project_id` after enforcing workspace jail & symlink checks.
    pub fn start(registry: SpecRegistry, project_id: Uuid) -> Result<Self, HarnessError> {
        let workspace_root = registry.verify_project_scope(project_id)?;
        let jailed_sample =
            resolve_jailed_spec_path(&workspace_root, CanonicalSpecKey::ProductOverview)?;
        let specs_dir = jailed_sample
            .parent()
            .ok_or_else(|| HarnessError::PathTraversalDenied {
                path: jailed_sample.display().to_string(),
            })?
            .to_path_buf();

        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default()).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("WATCHER_INIT_ERROR: {e}"),
            }
        })?;

        watcher
            .watch(&specs_dir, RecursiveMode::NonRecursive)
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("WATCHER_WATCH_ERROR: {e}"),
            })?;

        Ok(Self {
            registry,
            project_id,
            _watcher: watcher,
            rx,
        })
    }

    /// Processes a specific path notification directly through the registry's hash-gated reconciliation pipeline.
    pub fn process_path(&self, candidate_path: &Path) -> Result<SyncOutcome, HarnessError> {
        self.registry
            .handle_watcher_path_event(self.project_id, candidate_path)
    }

    /// Drains any pending `notify` events currently in the channel and reconciles canonical spec files.
    pub fn drain_events(&self) -> Vec<Result<SyncOutcome, HarnessError>> {
        let mut results = Vec::new();
        while let Ok(event_res) = self.rx.try_recv() {
            if let Ok(event) = event_res {
                for path in event.paths {
                    if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                        // Ignore temporary sibling files (.01-product-overview.md.tmp.<uuid>)
                        if fname.starts_with('.') && fname.contains(".tmp.") {
                            continue;
                        }
                    }
                    results.push(self.process_path(&path));
                }
            }
        }
        results
    }
}
