pub mod debug;
pub mod evals;
pub mod exec;
pub mod fix;
pub mod learn;
pub mod loop_mode;
pub mod plan;
pub mod predict;
pub mod probe;
pub mod reason;
pub mod scenario;
pub mod security;
pub mod ship;

use crate::core::config::Mode;
use anyhow::Result;

/// Dispatch to the correct mode handler.
pub fn dispatch(mode: Mode, args: &[String]) -> Result<()> {
    match mode {
        Mode::Loop => loop_mode::run(args),
        Mode::Plan => plan::run(args),
        Mode::Debug => debug::run(args),
        Mode::Fix => fix::run(args),
        Mode::Security => security::run(args),
        Mode::Ship => ship::run(args),
        Mode::Scenario => scenario::run(args),
        Mode::Predict => predict::run(args),
        Mode::Learn => learn::run(args),
        Mode::Reason => reason::run(args),
        Mode::Probe => probe::run(args),
        Mode::Evals => evals::run(args),
        Mode::Exec => exec::run(args),
    }
}
