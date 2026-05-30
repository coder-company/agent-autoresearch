use anyhow::{Context, Result};
use chrono::Utc;
use rust_decimal::Decimal;
use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::config::Direction;
use super::state::IterationStatus;

/// TSV header comment + column header.
pub fn tsv_header(direction: Direction) -> String {
    format!(
        "# metric_direction: {}\niteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription",
        direction.as_str()
    )
}

/// A single row in the results TSV.
#[derive(Debug, Clone)]
pub struct ResultRow {
    pub iteration: u32,
    pub commit: Option<String>,
    pub metric: Decimal,
    pub delta: Decimal,
    pub guard: GuardResult,
    pub status: IterationStatus,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardResult {
    Pass,
    Fail,
    Skip,
}

impl GuardResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Skip => "-",
        }
    }
}

impl ResultRow {
    pub fn to_tsv(&self) -> String {
        self.to_tsv_with_iteration_label(&self.iteration.to_string())
    }

    pub fn to_tsv_with_iteration_label(&self, iteration: &str) -> String {
        let commit = self.commit.as_deref().unwrap_or("-");
        let description = sanitize_tsv_field(&self.description);
        let delta_str = if self.delta.is_zero() {
            "0".to_string()
        } else if self.delta.is_sign_positive() {
            format!("+{}", self.delta)
        } else {
            self.delta.to_string()
        };

        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            iteration,
            commit,
            self.metric,
            delta_str,
            self.guard.as_str(),
            self.status.as_str(),
            description,
        )
    }
}

fn sanitize_tsv_field(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\t' | '\n' | '\r' => ' ',
            _ => ch,
        })
        .collect()
}

/// Results log manager.
pub struct ResultsLog {
    path: PathBuf,
}

impl ResultsLog {
    /// Create a new results log at the given path with the TSV header.
    pub fn create(dir: &Path, direction: Direction) -> Result<Self> {
        fs::create_dir_all(dir).context("Failed to create results directory")?;
        let path = dir.join("results.tsv");
        let header = tsv_header(direction);
        fs::write(&path, format!("{header}\n")).context("Failed to write results header")?;
        Ok(Self { path })
    }

    /// Open an existing results log.
    pub fn open(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            anyhow::bail!("Results file does not exist: {}", path.display());
        }
        Ok(Self { path })
    }

    /// Append a row to the log.
    pub fn append(&self, row: &ResultRow) -> Result<()> {
        self.append_labeled(&row.iteration.to_string(), row)
    }

    /// Append a row with a custom iteration label, such as `5a` for parallel worker detail.
    pub fn append_labeled(&self, iteration: &str, row: &ResultRow) -> Result<()> {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .context("Failed to open results TSV for append")?;
        writeln!(file, "{}", row.to_tsv_with_iteration_label(iteration))
            .context("Failed to write result row")?;
        Ok(())
    }

    /// Read the last N data rows from the log.
    pub fn tail(&self, n: usize) -> Result<Vec<String>> {
        let content = fs::read_to_string(&self.path).context("Failed to read results TSV")?;
        let rows: Vec<&str> = content
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("iteration\t") && !l.is_empty())
            .collect();
        let start = rows.len().saturating_sub(n);
        Ok(rows[start..].iter().map(|s| s.to_string()).collect())
    }

    /// Count authoritative main rows. Parallel worker rows such as `5a` are audit detail only.
    pub fn count(&self) -> Result<usize> {
        let content = fs::read_to_string(&self.path).context("Failed to read results TSV")?;
        Ok(content
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("iteration\t") && !l.is_empty())
            .filter(|line| {
                line.split('\t')
                    .next()
                    .is_some_and(|iteration| iteration.parse::<u32>().is_ok())
            })
            .count())
    }

    /// Validate that data rows are structurally parseable before health/runtime trust them.
    pub fn validate(&self) -> Result<()> {
        let content = fs::read_to_string(&self.path).context("Failed to read results TSV")?;
        let mut saw_direction = false;
        let mut saw_header = false;
        let mut layout = None;
        let mut expected_main_iteration = 0u32;
        let mut pending_worker_iteration = None;

        for (index, line) in content.lines().enumerate() {
            if line.is_empty() {
                continue;
            }
            if let Some(value) = line.strip_prefix("# metric_direction:") {
                if saw_direction {
                    anyhow::bail!(
                        "results.tsv line {} has a duplicate metric_direction header",
                        index + 1
                    );
                }
                let value = value.trim();
                if parse_metric_direction_value(value).is_none() {
                    anyhow::bail!(
                        "results.tsv line {} has invalid metric_direction {:?}",
                        index + 1,
                        value
                    );
                }
                saw_direction = true;
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            if line.starts_with("iteration\t") {
                if !saw_direction {
                    anyhow::bail!("results.tsv is missing the metric_direction header");
                }
                if saw_header {
                    anyhow::bail!(
                        "results.tsv line {} has a duplicate column header",
                        index + 1
                    );
                }
                if expected_main_iteration != 0 {
                    anyhow::bail!(
                        "results.tsv line {} has the column header after data rows",
                        index + 1
                    );
                }
                layout = Some(ResultsTsvLayout::parse(line)?);
                saw_header = true;
                continue;
            }
            if !saw_header {
                anyhow::bail!(
                    "results.tsv line {} has data before the column header",
                    index + 1
                );
            }

            let columns = line.split('\t').collect::<Vec<_>>();
            let layout = layout
                .as_ref()
                .expect("layout is set once saw_header is true");
            if columns.len() != layout.width {
                anyhow::bail!(
                    "results.tsv line {} has {} columns; expected {}",
                    index + 1,
                    columns.len(),
                    layout.width
                );
            }
            let iteration_label = columns[layout.iteration];
            let main_iteration = iteration_label.parse::<u32>().ok();
            if main_iteration.is_none() {
                match worker_iteration_prefix(iteration_label) {
                    Some(worker_iteration) if worker_iteration == expected_main_iteration => {
                        pending_worker_iteration = Some(worker_iteration);
                    }
                    Some(worker_iteration) => anyhow::bail!(
                        "results.tsv line {} has worker iteration {}; expected pending main iteration {}",
                        index + 1,
                        worker_iteration,
                        expected_main_iteration
                    ),
                    None => anyhow::bail!(
                        "results.tsv line {} has invalid iteration label {:?}",
                        index + 1,
                        iteration_label
                    ),
                }
            }
            if let Some(iteration) = main_iteration {
                if iteration != expected_main_iteration {
                    anyhow::bail!(
                        "results.tsv line {} has main iteration {}; expected {}",
                        index + 1,
                        iteration,
                        expected_main_iteration
                    );
                }
                if pending_worker_iteration == Some(iteration) {
                    pending_worker_iteration = None;
                }
                expected_main_iteration += 1;
            }
            columns[layout.metric]
                .parse::<Decimal>()
                .with_context(|| format!("results.tsv line {} has invalid metric", index + 1))?;
            columns[layout.delta]
                .trim_start_matches('+')
                .parse::<Decimal>()
                .with_context(|| format!("results.tsv line {} has invalid delta", index + 1))?;
            if !is_valid_guard(columns[layout.guard]) {
                anyhow::bail!(
                    "results.tsv line {} has invalid guard {:?}",
                    index + 1,
                    columns[layout.guard]
                );
            }
            let status = columns[layout.status];
            if !is_valid_status(status) {
                anyhow::bail!(
                    "results.tsv line {} has invalid status {:?}",
                    index + 1,
                    status
                );
            }
            if main_iteration.is_none() && status == "baseline" {
                anyhow::bail!(
                    "results.tsv line {} has worker row with baseline status",
                    index + 1
                );
            }
            if let Some(iteration) = main_iteration {
                match (iteration, status) {
                    (0, "baseline") => {}
                    (0, status) => anyhow::bail!(
                        "results.tsv line {} has baseline iteration with status {:?}",
                        index + 1,
                        status
                    ),
                    (_, "baseline") => anyhow::bail!(
                        "results.tsv line {} has baseline status after iteration 0",
                        index + 1
                    ),
                    _ => {}
                }
            }
        }

        if !saw_header {
            anyhow::bail!("results.tsv is missing the column header");
        }
        if !saw_direction {
            anyhow::bail!("results.tsv is missing the metric_direction header");
        }
        if expected_main_iteration == 0 {
            anyhow::bail!("results.tsv is missing the baseline row");
        }
        if let Some(iteration) = pending_worker_iteration {
            anyhow::bail!(
                "results.tsv has worker rows for iteration {iteration} without an authoritative main row"
            );
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn is_valid_guard(value: &str) -> bool {
    matches!(value, "pass" | "fail" | "-" | "skip")
}

#[derive(Debug, Clone)]
struct ResultsTsvLayout {
    iteration: usize,
    metric: usize,
    delta: usize,
    guard: usize,
    status: usize,
    width: usize,
}

impl ResultsTsvLayout {
    fn parse(header: &str) -> Result<Self> {
        let columns = header.split('\t').collect::<Vec<_>>();
        require_tsv_column(&columns, "commit", &["commit"])?;
        require_tsv_column(&columns, "description", &["description"])?;
        Ok(Self {
            iteration: require_tsv_column(&columns, "iteration", &["iteration"])?,
            metric: require_tsv_column(
                &columns,
                "metric",
                &["metric", "metric_value", "error_count"],
            )?,
            delta: require_tsv_column(&columns, "delta", &["delta"])?,
            guard: require_tsv_column(&columns, "guard", &["guard"])?,
            status: require_tsv_column(&columns, "status", &["status"])?,
            width: columns.len(),
        })
    }
}

fn require_tsv_column(headers: &[&str], label: &str, names: &[&str]) -> Result<usize> {
    headers
        .iter()
        .position(|header| names.iter().any(|name| header == name))
        .with_context(|| format!("results.tsv is missing required {label} column"))
}

fn is_valid_status(value: &str) -> bool {
    matches!(
        value,
        "baseline"
            | "keep"
            | "keep (reworked)"
            | "discard"
            | "crash"
            | "no-op"
            | "blocked"
            | "hook-blocked"
            | "metric-error"
            | "pivot"
            | "refine"
            | "search"
            | "drift"
    )
}

pub fn worker_iteration_prefix(value: &str) -> Option<u32> {
    let Some(suffix_start) = value.find(|ch: char| !ch.is_ascii_digit()) else {
        return None;
    };
    let (main, suffix) = value.split_at(suffix_start);
    if main.is_empty() || suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_lowercase()) {
        return None;
    }
    main.parse::<u32>().ok()
}

pub fn parse_metric_direction_value(value: &str) -> Option<Direction> {
    match value {
        "higher" | "higher_is_better" => Some(Direction::Higher),
        "lower" | "lower_is_better" => Some(Direction::Lower),
        _ => None,
    }
}

/// Generate a completion summary.
#[allow(clippy::too_many_arguments)]
pub fn completion_summary(
    baseline: Decimal,
    final_metric: Decimal,
    best: Decimal,
    keeps: u32,
    discards: u32,
    crashes: u32,
    total: u32,
    direction: Direction,
) -> String {
    let mut out = String::new();
    let improvement = final_metric - baseline;
    let pct = if !baseline.is_zero() {
        (improvement / baseline * Decimal::from(100)).round_dp(1)
    } else {
        Decimal::ZERO
    };

    writeln!(out, "## Autoresearch Complete").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| Stat | Value |").unwrap();
    writeln!(out, "|------|-------|").unwrap();
    writeln!(out, "| Iterations | {total} |").unwrap();
    writeln!(out, "| Kept | {keeps} |").unwrap();
    writeln!(out, "| Discarded | {discards} |").unwrap();
    writeln!(out, "| Crashes | {crashes} |").unwrap();
    writeln!(out, "| Baseline | {baseline} |").unwrap();
    writeln!(out, "| Final | {final_metric} |").unwrap();
    writeln!(out, "| Best | {best} |").unwrap();
    writeln!(
        out,
        "| Improvement | {improvement} ({pct}% {}) |",
        direction.as_str()
    )
    .unwrap();

    out
}

/// Artifact directory name for a run.
pub fn artifact_dir_name() -> &'static str {
    "autoresearch-results"
}

/// Get or create the results directory.
pub fn results_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(artifact_dir_name())
}

/// Create `autoresearch-results/` and protect it from git staging.
///
/// 1. Creates the directory
/// 2. Writes `autoresearch-results/.gitignore` containing `*\n!.gitignore\n`
/// 3. If `<workspace>/.gitignore` exists and doesn't contain `autoresearch-results/`, appends it
/// 4. If no `.gitignore` exists, creates one with `autoresearch-results/\n`
pub fn ensure_results_dir_protected(workspace: &Path) -> Result<PathBuf> {
    let results = workspace.join(artifact_dir_name());
    fs::create_dir_all(&results).context("Failed to create autoresearch-results directory")?;

    // Write inner .gitignore to ignore everything except itself
    let inner_gitignore = results.join(".gitignore");
    fs::write(&inner_gitignore, "*\n!.gitignore\n")
        .context("Failed to write autoresearch-results/.gitignore")?;

    // Protect from workspace-level git staging
    let ws_gitignore = workspace.join(".gitignore");
    let entries = ["autoresearch-results/", ".codex-autoresearch/"];
    if ws_gitignore.exists() {
        let content =
            fs::read_to_string(&ws_gitignore).context("Failed to read workspace .gitignore")?;
        let missing: Vec<&str> = entries
            .iter()
            .copied()
            .filter(|entry| !content.lines().any(|l| l.trim() == *entry))
            .collect();
        if !missing.is_empty() {
            use std::io::Write;
            let mut file = OpenOptions::new()
                .append(true)
                .open(&ws_gitignore)
                .context("Failed to open workspace .gitignore for append")?;
            // Ensure we start on a new line
            if !content.ends_with('\n') && !content.is_empty() {
                writeln!(file)?;
            }
            for entry in missing {
                writeln!(file, "{entry}")?;
            }
        }
    } else {
        fs::write(&ws_gitignore, entries.join("\n") + "\n")
            .context("Failed to create workspace .gitignore")?;
    }

    Ok(results)
}

/// Generate a timestamped run tag.
pub fn generate_run_tag() -> String {
    Utc::now().format("%y%m%d-%H%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_row_to_tsv() {
        let row = ResultRow {
            iteration: 1,
            commit: Some("abc1234".to_string()),
            metric: Decimal::from(87),
            delta: Decimal::from(2),
            guard: GuardResult::Pass,
            status: IterationStatus::Keep,
            description: "add auth tests".to_string(),
        };
        assert_eq!(
            row.to_tsv(),
            "1\tabc1234\t87\t+2\tpass\tkeep\tadd auth tests"
        );
    }

    #[test]
    fn test_result_row_discard() {
        let row = ResultRow {
            iteration: 2,
            commit: None,
            metric: Decimal::from(84),
            delta: Decimal::from(-1),
            guard: GuardResult::Skip,
            status: IterationStatus::Discard,
            description: "refactor broke tests".to_string(),
        };
        assert_eq!(
            row.to_tsv(),
            "2\t-\t84\t-1\t-\tdiscard\trefactor broke tests"
        );
    }

    #[test]
    fn test_result_row_parallel_worker_label() {
        let row = ResultRow {
            iteration: 5,
            commit: Some("abc1234".to_string()),
            metric: Decimal::from(38),
            delta: Decimal::from(-3),
            guard: GuardResult::Pass,
            status: IterationStatus::Keep,
            description: "[PARALLEL worker-a] narrowed auth types".to_string(),
        };
        assert_eq!(
            row.to_tsv_with_iteration_label("5a"),
            "5a\tabc1234\t38\t-3\tpass\tkeep\t[PARALLEL worker-a] narrowed auth types"
        );
    }

    #[test]
    fn test_result_row_sanitizes_multiline_description() {
        let row = ResultRow {
            iteration: 3,
            commit: Some("abc1234".to_string()),
            metric: Decimal::from(10),
            delta: Decimal::from(1),
            guard: GuardResult::Pass,
            status: IterationStatus::Keep,
            description: "line one\tline two\nline three\rline four".to_string(),
        };
        assert_eq!(
            row.to_tsv(),
            "3\tabc1234\t10\t+1\tpass\tkeep\tline one line two line three line four"
        );
    }

    #[test]
    fn test_count_ignores_parallel_worker_rows() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Lower).unwrap();
        log.append(&ResultRow {
            iteration: 0,
            commit: Some("base".to_string()),
            metric: Decimal::from(41),
            delta: Decimal::ZERO,
            guard: GuardResult::Skip,
            status: IterationStatus::Baseline,
            description: "baseline".to_string(),
        })
        .unwrap();
        let worker = ResultRow {
            iteration: 1,
            commit: Some("abc1234".to_string()),
            metric: Decimal::from(38),
            delta: Decimal::from(-3),
            guard: GuardResult::Pass,
            status: IterationStatus::Keep,
            description: "[PARALLEL worker-a] narrowed auth types".to_string(),
        };
        log.append_labeled("1a", &worker).unwrap();
        log.append(&ResultRow {
            iteration: 1,
            description: "[PARALLEL batch] selected worker-a".to_string(),
            ..worker
        })
        .unwrap();

        assert_eq!(log.count().unwrap(), 2);
        log.validate().unwrap();
    }

    #[test]
    fn test_validate_requires_contiguous_main_iterations() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tbaseline\tbaseline\n2\tabc1234\t11\t+1\tpass\tkeep\tskipped one\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("main iteration 2; expected 1"));
    }

    #[test]
    fn test_validate_requires_baseline_row() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("missing the baseline row"));
    }

    #[test]
    fn test_validate_rejects_data_before_header() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            "0\tbase\t10\t0\t-\tbaseline\tbaseline\niteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n",
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("data before the column header"));
    }

    #[test]
    fn test_validate_rejects_duplicate_header() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tbaseline\tbaseline\niteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("duplicate column header"));
    }

    #[test]
    fn test_validate_requires_metric_direction_header() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            "iteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n0\tbase\t10\t0\t-\tbaseline\tbaseline\n",
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("missing the metric_direction header"));
    }

    #[test]
    fn test_validate_rejects_invalid_metric_direction_header() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            "# metric_direction: sideways\niteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n0\tbase\t10\t0\t-\tbaseline\tbaseline\n",
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("invalid metric_direction \"sideways\""));
    }

    #[test]
    fn test_validate_requires_baseline_status_for_iteration_zero() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tkeep\twrong first status\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("baseline iteration with status"));
    }

    #[test]
    fn test_validate_rejects_late_baseline_status() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tbaseline\tbaseline\n1\tabc1234\t11\t+1\tpass\tbaseline\tlate baseline\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("baseline status after iteration 0"));
    }

    #[test]
    fn test_validate_accepts_drift_status() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tbaseline\tbaseline\n1\t-\t9\t-1\t-\tdrift\trecalibrated\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        log.validate().unwrap();
    }

    #[test]
    fn test_validate_accepts_legacy_result_statuses() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tbaseline\tbaseline\n1\tabc1234\t11\t+1\tpass\tkeep (reworked)\tadjusted fix\n2\t-\t10\t0\t-\thook-blocked\tcommit hook blocked\n3\t-\t10\t0\t-\tmetric-error\tverify output invalid\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        log.validate().unwrap();
    }

    #[test]
    fn test_validate_accepts_skip_guard_alias() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tbaseline\tbaseline\n1\tabc1234\t11\t+1\tskip\tkeep\tguard skipped\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        log.validate().unwrap();
    }

    #[test]
    fn test_validate_accepts_timestamp_and_guard_metric_columns() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            "# metric_direction: higher\niteration\ttimestamp\tcommit\tmetric\tdelta\tguard\tguard-metric\tstatus\tdescription\n0\t2026-05-30T00:00:00Z\tbase\t10\t0\t-\t-\tbaseline\tbaseline\n1\t2026-05-30T00:01:00Z\tabc1234\t11\t+1\tpass\tok\tkeep\timproved\n",
        )
        .unwrap();

        log.validate().unwrap();
    }

    #[test]
    fn test_validate_rejects_invalid_worker_iteration_label() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tbaseline\tbaseline\nworker\tabc1234\t11\t+1\tpass\tkeep\tbad label\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("invalid iteration label"));
    }

    #[test]
    fn test_validate_rejects_worker_label_for_wrong_main_iteration() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tbaseline\tbaseline\n2a\tabc1234\t11\t+1\tpass\tkeep\twrong batch\n1\tabc1234\t11\t+1\tpass\tkeep\tmain row\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("worker iteration 2; expected pending main iteration 1"));
    }

    #[test]
    fn test_validate_rejects_worker_rows_without_main_row() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tbaseline\tbaseline\n1a\tabc1234\t11\t+1\tpass\tkeep\tworker only\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("worker rows for iteration 1 without an authoritative main row"));
    }

    #[test]
    fn test_validate_rejects_worker_baseline_status() {
        let dir = tempfile::tempdir().unwrap();
        let log = ResultsLog::create(dir.path(), Direction::Higher).unwrap();
        fs::write(
            log.path(),
            format!(
                "{}\n0\tbase\t10\t0\t-\tbaseline\tbaseline\n1a\tabc1234\t11\t+1\tpass\tbaseline\tbad worker\n1\tabc1234\t11\t+1\tpass\tkeep\tmain row\n",
                tsv_header(Direction::Higher)
            ),
        )
        .unwrap();

        let err = log.validate().unwrap_err().to_string();

        assert!(err.contains("worker row with baseline status"));
    }
}
