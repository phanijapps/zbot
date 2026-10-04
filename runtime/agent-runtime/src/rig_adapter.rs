//! Rig adapter boundary metadata.
//!
//! The implementation adapter lands in later migration tasks. This module owns
//! the dependency pin so Rig stays confined to `agent-runtime`.

mod checkpoint_tail;
pub mod client;
pub mod config;
mod context_inputs;
mod context_policy;
pub mod engine;
pub mod factory;
pub mod model;
mod progress_policy;
mod resources;
pub mod structured;
pub mod tool;
mod tool_hook;
mod tool_results;
mod turn_events;
mod turn_signal;

#[cfg(test)]
mod capability_tests;
#[cfg(test)]
mod mcp_capability_tests;
#[cfg(test)]
mod mcp_lifecycle_tests;

pub use client::LlmCompletionClient;
pub use config::{RigAgentConfig, RigConfigError, RigModelConfig};
pub use structured::prompt_typed;
pub use tool::{RigToolAdapter, SharedToolContext};

/// SDK tracing roots that record raw payloads before host policy hooks.
pub(crate) const PAYLOAD_DIAGNOSTIC_TARGETS: &[&str] = &["rig", "rig_core", "rig_agent"];

/// Rig package source selected for the migration.
pub const RIG_REPOSITORY: &str = "https://github.com/0xplaygrounds/rig";

/// Exact published Rig release.
pub const RIG_VERSION: &str = "0.43.0";

/// Reviewable Rig dependency pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RigDependencyPin {
    /// Git repository URL.
    pub repository: &'static str,
    /// Exact published crate version.
    pub version: &'static str,
}

/// Return the active Rig dependency pin.
#[must_use]
pub const fn dependency_pin() -> RigDependencyPin {
    RigDependencyPin {
        repository: RIG_REPOSITORY,
        version: RIG_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn dependency_pin_matches_manifest_decision() {
        let pin = dependency_pin();
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest = fs::read_to_string(manifest_dir.join("Cargo.toml"))
            .expect("agent-runtime manifest should be readable");
        assert!(
            manifest.contains(&format!("version = \"={}\"", pin.version)),
            "agent-runtime manifest should declare Rig version {}",
            pin.version
        );

        let workspace_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .expect("agent-runtime should be two levels under the workspace");
        let lockfile = fs::read_to_string(workspace_root.join("Cargo.lock"))
            .expect("Cargo.lock should be readable");
        let rig_package = lockfile
            .split("[[package]]")
            .find(|section| section.contains("name = \"rig\""))
            .expect("Cargo.lock should contain the rig package");
        assert!(
            rig_package.contains(&format!("version = \"{}\"", pin.version)),
            "Cargo.lock Rig package should use version {}",
            pin.version
        );
        assert!(rig_package.contains("registry+https://github.com/rust-lang/crates.io-index"));
        assert!(rig_package.contains("checksum ="));
    }

    #[test]
    fn rig_crate_is_available_to_runtime_adapter() {
        let type_name = std::any::type_name::<rig::completion::ToolDefinition>();
        assert!(type_name.contains("ToolDefinition"));
    }
}
