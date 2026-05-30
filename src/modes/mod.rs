//! Mode implementations for the autoresearch engine.
//!
//! Each mode provides configuration validation, description metadata,
//! and mode-specific data structures. The actual execution is driven by
//! the CLI commands and /goal system — modes are the configuration layer.

pub mod debug;
pub mod evals;
pub mod fix;
pub mod improve;
pub mod learn;
pub mod loop_mode;
pub mod plan;
pub mod predict;
pub mod probe;
pub mod reason;
pub mod scenario;
pub mod security;
pub mod ship;

use anyhow::Result;
use serde::Serialize;

use crate::core::config::RunConfig;

/// Description metadata for a mode.
#[derive(Debug, Clone, Serialize)]
pub struct ModeDescription {
    /// Short machine name.
    pub name: &'static str,
    /// Human-readable purpose.
    pub purpose: &'static str,
    /// Default iteration cap, if any.
    pub default_iterations: Option<u32>,
    /// Fields that must be present in RunConfig for this mode.
    pub required_fields: &'static [&'static str],
    /// Fields that are optional but influence behavior.
    pub optional_fields: &'static [&'static str],
}

/// Trait implemented by all mode runners.
pub trait ModeRunner {
    /// Human-readable name for this mode.
    fn name(&self) -> &'static str;
    /// Validate that configuration is sufficient for this mode.
    fn validate_config(&self, config: &RunConfig) -> Result<()>;
    /// Generate the structured output description for this mode.
    fn describe(&self) -> ModeDescription;
}

// Re-export mode structs for convenience.
pub use debug::DebugMode;
pub use evals::EvalsMode;
pub use fix::FixMode;
pub use improve::ImproveMode;
pub use learn::LearnMode;
pub use loop_mode::LoopMode;
pub use plan::PlanMode;
pub use predict::PredictMode;
pub use probe::ProbeMode;
pub use reason::ReasonMode;
pub use scenario::ScenarioMode;
pub use security::SecurityMode;
pub use ship::ShipMode;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Mode;

    #[test]
    fn mode_descriptions_match_catalog_defaults() {
        let descriptions = [
            (Mode::Loop, LoopMode.describe()),
            (Mode::Plan, PlanMode.describe()),
            (Mode::Debug, DebugMode.describe()),
            (Mode::Fix, FixMode.describe()),
            (Mode::Security, SecurityMode.describe()),
            (Mode::Ship, ShipMode.describe()),
            (Mode::Scenario, ScenarioMode.describe()),
            (Mode::Predict, PredictMode.describe()),
            (Mode::Learn, LearnMode.describe()),
            (Mode::Reason, ReasonMode.describe()),
            (Mode::Probe, ProbeMode.describe()),
            (Mode::Improve, ImproveMode.describe()),
            (Mode::Evals, EvalsMode.describe()),
        ];

        for (mode, description) in descriptions {
            assert_eq!(description.name, mode.as_str());
            assert_eq!(description.default_iterations, mode.default_iterations());
        }
    }
}
