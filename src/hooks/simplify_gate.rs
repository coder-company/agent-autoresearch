use super::{HookInput, HookResponse};

/// Simplify gate: remind the agent to prefer simpler solutions
/// when the iteration log shows marginal gains with high complexity.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let _input = match input {
        Some(i) => i,
        None => return HookResponse::allow(),
    };

    // Check if we have recent results showing marginal improvements
    let cwd = std::env::current_dir().unwrap_or_default();
    let tsv_path = cwd.join("autoresearch-results/results.tsv");

    if !tsv_path.exists() {
        return HookResponse::allow();
    }

    let content = match std::fs::read_to_string(&tsv_path) {
        Ok(c) => c,
        Err(_) => return HookResponse::allow(),
    };

    // Count recent marginal keeps (delta < 1% of baseline)
    let data_lines: Vec<&str> = content
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("iteration\t") && !l.is_empty())
        .collect();

    let last_5: Vec<&str> = data_lines.iter().rev().take(5).copied().collect();

    let marginal_keeps = last_5
        .iter()
        .filter(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 6 {
                return false;
            }
            let status = parts[5];
            let delta = parts[3].trim_start_matches('+');
            status == "keep"
                && delta
                    .parse::<f64>()
                    .map(|d| d.abs() < 1.0)
                    .unwrap_or(false)
        })
        .count();

    if marginal_keeps >= 3 {
        return HookResponse::inject(
            "⚠️ **Simplicity gate:** Last 3+ keeps were marginal (<1% improvement). \
             Consider whether added complexity is justified. \
             Rule 6: Equal results + less code = KEEP. \
             Rule 11: Discard gains under 1% that add disproportionate complexity."
                .to_string(),
        );
    }

    HookResponse::allow()
}
