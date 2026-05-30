use super::{HookInput, HookResponse};
use std::process::Command;

const SHIPPING_VERBS: &[&str] = &["ship", "merge", "deploy", "pr", "publish", "release"];
const NEGATION_PHRASES: &[&str] = &[
    "don't ship",
    "never deploy",
    "not ready to merge",
    "don't merge",
    "don't deploy",
    "don't publish",
    "don't release",
    "no ship",
    "no merge",
    "no deploy",
];
const WARN_THRESHOLD: u64 = 400;
const BLOCK_THRESHOLD: u64 = 800;

/// Simplify gate: remind the agent to prefer simpler solutions
/// when the iteration log shows marginal gains with high complexity.
pub fn run(input: Option<&HookInput>) -> HookResponse {
    let input = match input {
        Some(i) => i,
        None => return HookResponse::allow(),
    };

    if let Some(prompt) = input.prompt.as_deref() {
        if has_shipping_verb(prompt) {
            match current_diff_loc() {
                Some(loc) if loc > BLOCK_THRESHOLD => {
                    return HookResponse::block(format!(
                        "Blocked: {loc} changed lines exceeds {BLOCK_THRESHOLD} LOC shipping threshold. Simplify before shipping, or set AR_DISABLE_SIMPLIFY_GATE=1 to override."
                    ));
                }
                Some(loc) if loc >= WARN_THRESHOLD => {
                    return HookResponse::inject(format!(
                        "WARNING: {loc} changed lines. Consider simplifying before shipping."
                    ));
                }
                _ => {}
            }
        }
    }

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
            status == "keep" && delta.parse::<f64>().map(|d| d.abs() < 1.0).unwrap_or(false)
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

fn has_shipping_verb(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    if NEGATION_PHRASES.iter().any(|phrase| lower.contains(phrase)) {
        return false;
    }
    SHIPPING_VERBS.iter().any(|verb| {
        lower
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|word| word == *verb)
    })
}

fn current_diff_loc() -> Option<u64> {
    let output = Command::new("git").args(["diff", "--stat"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    parse_diff_loc(&stdout)
}

fn parse_diff_loc(diff_stat: &str) -> Option<u64> {
    let summary = diff_stat.lines().rev().find(|line| {
        line.contains("changed") || line.contains("insertion") || line.contains("deletion")
    })?;
    Some(
        extract_count_before(summary, "insertion").unwrap_or(0)
            + extract_count_before(summary, "deletion").unwrap_or(0),
    )
}

fn extract_count_before(text: &str, marker: &str) -> Option<u64> {
    let prefix = text.split(marker).next()?;
    prefix
        .split(|ch: char| !ch.is_ascii_digit())
        .rev()
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse().ok())
}
