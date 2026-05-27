use anyhow::{Context, Result};
use rust_decimal::Decimal;
use std::str::FromStr;

use super::config::Direction;

/// Parse a metric value from verify command output.
/// Follows the scalar contract: the final non-empty line must be a numeric value.
pub fn parse_scalar_metric(output: &str) -> Result<Decimal> {
    let line = output
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .context("Verify command produced no output")?
        .trim();

    Decimal::from_str(line)
        .with_context(|| format!("Cannot parse metric from final line: {line:?}"))
}

/// Parse metrics from JSON output format.
/// The final non-empty line must be a valid JSON object.
pub fn parse_json_metrics(output: &str, primary_key: &str) -> Result<Decimal> {
    let line = output
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .context("Verify command produced no output")?
        .trim();

    let obj: serde_json::Value =
        serde_json::from_str(line).context("Final line is not valid JSON")?;

    let value = obj
        .get(primary_key)
        .with_context(|| format!("Key {primary_key:?} not found in metrics JSON"))?;

    match value {
        serde_json::Value::Number(n) => {
            let s = n.to_string();
            Decimal::from_str(&s)
                .with_context(|| format!("Cannot parse {primary_key:?} value: {s:?}"))
        }
        serde_json::Value::String(s) => {
            Decimal::from_str(s).with_context(|| format!("Cannot parse {primary_key:?}: {s:?}"))
        }
        _ => anyhow::bail!("Metric key {primary_key:?} is not a number or string"),
    }
}

/// Calculate delta between two metric values.
pub fn delta(current: Decimal, previous: Decimal) -> Decimal {
    current - previous
}

/// Format delta with sign prefix.
pub fn format_delta(d: Decimal) -> String {
    if d > Decimal::ZERO {
        format!("+{d}")
    } else {
        d.to_string()
    }
}

/// Determine if a metric change represents improvement.
pub fn is_improvement(current: Decimal, previous: Decimal, direction: Direction) -> bool {
    let d = delta(current, previous);
    direction.is_improvement(d)
}

/// Check if a gain is marginal (< 1% of baseline).
pub fn is_marginal(current: Decimal, baseline: Decimal) -> bool {
    if baseline.is_zero() {
        return current.is_zero();
    }
    let pct_change = ((current - baseline) / baseline).abs() * Decimal::from(100);
    pct_change < Decimal::ONE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scalar_metric() {
        assert_eq!(
            parse_scalar_metric("some banner\n\n85.2\n").unwrap(),
            Decimal::from_str("85.2").unwrap()
        );
    }

    #[test]
    fn test_parse_scalar_metric_integer() {
        assert_eq!(parse_scalar_metric("47\n").unwrap(), Decimal::from(47));
    }

    #[test]
    fn test_parse_scalar_metric_empty() {
        assert!(parse_scalar_metric("").is_err());
    }

    #[test]
    fn test_parse_scalar_metric_non_numeric() {
        assert!(parse_scalar_metric("all tests passed\n").is_err());
    }

    #[test]
    fn test_parse_json_metrics() {
        let output = r#"running tests...
{"coverage": 85.2, "passing": 47, "failing": 3}
"#;
        assert_eq!(
            parse_json_metrics(output, "coverage").unwrap(),
            Decimal::from_str("85.2").unwrap()
        );
    }

    #[test]
    fn test_is_improvement_higher() {
        assert!(is_improvement(
            Decimal::from(90),
            Decimal::from(85),
            Direction::Higher
        ));
        assert!(!is_improvement(
            Decimal::from(80),
            Decimal::from(85),
            Direction::Higher
        ));
    }

    #[test]
    fn test_is_improvement_lower() {
        assert!(is_improvement(
            Decimal::from(3),
            Decimal::from(5),
            Direction::Lower
        ));
        assert!(!is_improvement(
            Decimal::from(7),
            Decimal::from(5),
            Direction::Lower
        ));
    }

    #[test]
    fn test_is_marginal() {
        // 85 -> 85.5 is < 1% of 85
        assert!(is_marginal(
            Decimal::from_str("85.5").unwrap(),
            Decimal::from(85),
        ));
        // 85 -> 90 is > 1%
        assert!(!is_marginal(Decimal::from(90), Decimal::from(85)));
    }
}
