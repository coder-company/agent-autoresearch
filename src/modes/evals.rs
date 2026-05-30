//! Results analyzer mode.
//!
//! Parses results TSV files, detects trends, identifies plateaus,
//! computes efficiency metrics, and recommends next actions.

use anyhow::{bail, Context, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// Trend direction detected from results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trend {
    Improving,
    Flat,
    Declining,
}

/// Recommendation based on analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    /// Keep going — trend is positive.
    Continue,
    /// Stop — goal reached or diminishing returns.
    Stop,
    /// Change strategy — plateau or regression.
    ChangeStrategy,
}

/// A parsed row from the results TSV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRow {
    pub iteration: u32,
    pub commit: Option<String>,
    pub metric: Decimal,
    pub delta: Decimal,
    pub status: String,
    pub description: String,
}

/// Efficiency metrics computed from results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficiencyMetrics {
    /// Total iterations.
    pub total_iterations: u32,
    /// Number of keeps.
    pub keeps: u32,
    /// Number of discards.
    pub discards: u32,
    /// Keep/total ratio.
    pub keep_ratio: f64,
    /// Total metric improvement.
    pub total_improvement: Decimal,
    /// Average improvement per keep.
    pub avg_improvement_per_keep: Option<Decimal>,
}

/// Complete analysis output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalsAnalysis {
    /// Parsed rows.
    pub rows: Vec<ParsedRow>,
    /// Detected trend.
    pub trend: Trend,
    /// Plateau detected at iteration N (None if no plateau).
    pub plateau_at: Option<u32>,
    /// Efficiency metrics.
    pub efficiency: EfficiencyMetrics,
    /// Recommendation.
    pub recommendation: Recommendation,
    /// Human-readable summary.
    pub summary: String,
}

/// Parse a results TSV string into rows.
pub fn parse_results_tsv(content: &str) -> Result<Vec<ParsedRow>> {
    let mut rows = Vec::new();

    for line in content.lines() {
        // Skip comments and header.
        if line.starts_with('#') || line.starts_with("iteration\t") || line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 7 {
            continue;
        }

        let Ok(iteration) = parts[0].parse::<u32>() else {
            continue;
        };
        let commit = if parts[1] == "-" {
            None
        } else {
            Some(parts[1].to_string())
        };
        let metric = Decimal::from_str(parts[2]).context("Invalid metric value")?;
        let delta_str = parts[3].trim_start_matches('+');
        let delta = Decimal::from_str(delta_str).unwrap_or(Decimal::ZERO);
        let status = parts[5].to_string();
        let description = parts[6].to_string();

        rows.push(ParsedRow {
            iteration,
            commit,
            metric,
            delta,
            status,
            description,
        });
    }

    Ok(rows)
}

/// Detect the trend from parsed rows.
pub fn detect_trend(rows: &[ParsedRow]) -> Trend {
    if rows.len() < 3 {
        return Trend::Flat;
    }

    // Look at the last N rows to determine direction.
    let window = rows.len().min(5);
    let recent = &rows[rows.len() - window..];

    let positive_deltas = recent.iter().filter(|r| r.delta > Decimal::ZERO).count();
    let negative_deltas = recent.iter().filter(|r| r.delta < Decimal::ZERO).count();

    if positive_deltas > negative_deltas + 1 {
        Trend::Improving
    } else if negative_deltas > positive_deltas + 1 {
        Trend::Declining
    } else {
        Trend::Flat
    }
}

/// Detect plateau (N consecutive iterations without improvement).
pub fn detect_plateau(rows: &[ParsedRow], threshold: usize) -> Option<u32> {
    if rows.len() < threshold {
        return None;
    }

    let mut consecutive_no_improvement = 0;
    for row in rows.iter().rev() {
        if row.status == "keep" {
            break;
        }
        consecutive_no_improvement += 1;
    }

    if consecutive_no_improvement >= threshold {
        rows.last().map(|r| r.iteration)
    } else {
        None
    }
}

/// Compute efficiency metrics.
pub fn compute_efficiency(rows: &[ParsedRow]) -> EfficiencyMetrics {
    let attempt_rows: Vec<&ParsedRow> = rows.iter().filter(|r| r.status != "baseline").collect();
    let total = attempt_rows.len() as u32;
    let keeps = attempt_rows.iter().filter(|r| r.status == "keep").count() as u32;
    let discards = attempt_rows
        .iter()
        .filter(|r| r.status == "discard")
        .count() as u32;

    let keep_ratio = if total > 0 {
        keeps as f64 / total as f64
    } else {
        0.0
    };

    let total_improvement: Decimal = rows
        .iter()
        .filter(|r| r.status == "keep")
        .map(|r| r.delta)
        .sum();

    let avg_improvement_per_keep = if keeps > 0 {
        Some(total_improvement / Decimal::from(keeps))
    } else {
        None
    };

    EfficiencyMetrics {
        total_iterations: total,
        keeps,
        discards,
        keep_ratio,
        total_improvement,
        avg_improvement_per_keep,
    }
}

/// Generate a recommendation based on analysis.
pub fn recommend(
    trend: Trend,
    plateau_at: Option<u32>,
    efficiency: &EfficiencyMetrics,
) -> Recommendation {
    if plateau_at.is_some() {
        return Recommendation::ChangeStrategy;
    }

    match trend {
        Trend::Improving => Recommendation::Continue,
        Trend::Declining => Recommendation::ChangeStrategy,
        Trend::Flat => {
            if efficiency.keep_ratio < 0.1 {
                Recommendation::ChangeStrategy
            } else {
                Recommendation::Continue
            }
        }
    }
}

/// The results analyzer mode.
#[derive(Debug, Clone, Default)]
pub struct EvalsMode;

impl ModeRunner for EvalsMode {
    fn name(&self) -> &'static str {
        "evals"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        // Evals mode reads an existing results file — needs workspace_root
        // or the default autoresearch-results directory to exist.
        if config.goal.is_empty() && config.scope.is_empty() {
            bail!("Evals mode requires either a goal (results file path) or workspace_root");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "evals",
            purpose: "Results analyzer: trends, plateaus, efficiency, recommendations",
            default_iterations: None,
            required_fields: &[],
            optional_fields: &["goal", "scope", "workspace_root"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Direction;

    fn make_config() -> RunConfig {
        RunConfig {
            goal: "autoresearch-results/results.tsv".into(),
            scope: vec![],
            metric: String::new(),
            direction: Direction::Higher,
            verify: String::new(),
            guard: None,
            iterations: None,
            run_tag: None,
            stop_condition: None,
            verify_format: Default::default(),
            primary_metric_key: None,
            acceptance_criteria: Vec::new(),
            required_keep_criteria: Vec::new(),
            required_keep_labels: Vec::new(),
            rollback_strategy: Default::default(),
            run_mode: None,
            workspace_root: None,
            primary_repo: None,
        }
    }

    #[test]
    fn test_validate_valid() {
        let mode = EvalsMode;
        assert!(mode.validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_parse_results_tsv() {
        let tsv = "# metric_direction: higher\niteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n\
                   1\tabc1234\t85\t+2\tpass\tkeep\tadd tests\n\
                   2\t-\t84\t-1\t-\tdiscard\tbroke build\n";

        let rows = parse_results_tsv(tsv).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].iteration, 1);
        assert_eq!(rows[0].status, "keep");
        assert_eq!(rows[1].status, "discard");
    }

    #[test]
    fn test_parse_results_tsv_skips_parallel_worker_rows() {
        let tsv = "# metric_direction: lower\niteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n\
                   0\tbase\t41\t0\t-\tbaseline\tinitial\n\
                   1a\tabc1234\t38\t-3\tpass\tkeep\tworker\n\
                   1\tabc1234\t38\t-3\tpass\tkeep\tbatch\n";

        let rows = parse_results_tsv(tsv).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].iteration, 0);
        assert_eq!(rows[1].iteration, 1);
        assert_eq!(rows[1].description, "batch");
    }

    #[test]
    fn test_detect_trend_improving() {
        let rows: Vec<ParsedRow> = (1..=5)
            .map(|i| ParsedRow {
                iteration: i,
                commit: Some("abc".into()),
                metric: Decimal::from(80 + i),
                delta: Decimal::from(1),
                status: "keep".into(),
                description: "test".into(),
            })
            .collect();
        assert_eq!(detect_trend(&rows), Trend::Improving);
    }

    #[test]
    fn test_detect_trend_flat() {
        let rows: Vec<ParsedRow> = (1..=5)
            .map(|i| ParsedRow {
                iteration: i,
                commit: None,
                metric: Decimal::from(80),
                delta: Decimal::ZERO,
                status: "discard".into(),
                description: "test".into(),
            })
            .collect();
        assert_eq!(detect_trend(&rows), Trend::Flat);
    }

    #[test]
    fn test_detect_plateau() {
        let rows: Vec<ParsedRow> = (1..=5)
            .map(|i| ParsedRow {
                iteration: i,
                commit: None,
                metric: Decimal::from(80),
                delta: Decimal::ZERO,
                status: "discard".into(),
                description: "test".into(),
            })
            .collect();
        assert_eq!(detect_plateau(&rows, 3), Some(5));
    }

    #[test]
    fn test_no_plateau_with_recent_keep() {
        let mut rows: Vec<ParsedRow> = (1..=3)
            .map(|i| ParsedRow {
                iteration: i,
                commit: None,
                metric: Decimal::from(80),
                delta: Decimal::ZERO,
                status: "discard".into(),
                description: "test".into(),
            })
            .collect();
        rows.push(ParsedRow {
            iteration: 4,
            commit: Some("abc".into()),
            metric: Decimal::from(82),
            delta: Decimal::from(2),
            status: "keep".into(),
            description: "improved".into(),
        });
        assert_eq!(detect_plateau(&rows, 3), None);
    }

    #[test]
    fn test_efficiency_metrics() {
        let rows = vec![
            ParsedRow {
                iteration: 1,
                commit: Some("a".into()),
                metric: Decimal::from(82),
                delta: Decimal::from(2),
                status: "keep".into(),
                description: "t".into(),
            },
            ParsedRow {
                iteration: 2,
                commit: None,
                metric: Decimal::from(80),
                delta: Decimal::from(-2),
                status: "discard".into(),
                description: "t".into(),
            },
            ParsedRow {
                iteration: 3,
                commit: Some("b".into()),
                metric: Decimal::from(83),
                delta: Decimal::from(3),
                status: "keep".into(),
                description: "t".into(),
            },
        ];
        let eff = compute_efficiency(&rows);
        assert_eq!(eff.total_iterations, 3);
        assert_eq!(eff.keeps, 2);
        assert_eq!(eff.discards, 1);
    }

    #[test]
    fn test_efficiency_metrics_exclude_baseline() {
        let rows = vec![
            ParsedRow {
                iteration: 0,
                commit: Some("base".into()),
                metric: Decimal::from(10),
                delta: Decimal::ZERO,
                status: "baseline".into(),
                description: "initial".into(),
            },
            ParsedRow {
                iteration: 1,
                commit: Some("a".into()),
                metric: Decimal::from(8),
                delta: Decimal::from(-2),
                status: "keep".into(),
                description: "t".into(),
            },
            ParsedRow {
                iteration: 2,
                commit: None,
                metric: Decimal::from(9),
                delta: Decimal::from(1),
                status: "discard".into(),
                description: "t".into(),
            },
        ];
        let eff = compute_efficiency(&rows);
        assert_eq!(eff.total_iterations, 2);
        assert_eq!(eff.keeps, 1);
        assert_eq!(eff.discards, 1);
        assert_eq!(eff.keep_ratio, 0.5);
    }

    #[test]
    fn test_recommend_continue() {
        let eff = EfficiencyMetrics {
            total_iterations: 10,
            keeps: 5,
            discards: 5,
            keep_ratio: 0.5,
            total_improvement: Decimal::from(10),
            avg_improvement_per_keep: Some(Decimal::from(2)),
        };
        assert_eq!(
            recommend(Trend::Improving, None, &eff),
            Recommendation::Continue
        );
    }

    #[test]
    fn test_recommend_change_strategy_on_plateau() {
        let eff = EfficiencyMetrics {
            total_iterations: 10,
            keeps: 1,
            discards: 9,
            keep_ratio: 0.1,
            total_improvement: Decimal::from(1),
            avg_improvement_per_keep: Some(Decimal::from(1)),
        };
        assert_eq!(
            recommend(Trend::Flat, Some(10), &eff),
            Recommendation::ChangeStrategy
        );
    }
}
