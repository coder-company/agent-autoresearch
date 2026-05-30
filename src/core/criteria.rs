use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::BTreeMap;

use super::config::MetricCriterion;

const VALID_OPERATORS: &[&str] = &["<", "<=", ">", ">=", "=="];

#[derive(Debug, Clone, Serialize)]
pub struct CriteriaEvaluation {
    pub satisfied: bool,
    pub failures: Vec<String>,
}

pub fn parse_criteria_json(raw: Option<&str>, field_name: &str) -> Result<Vec<MetricCriterion>> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let criteria: Vec<MetricCriterion> =
        serde_json::from_str(trimmed).with_context(|| format!("Invalid {field_name} JSON"))?;
    validate_criteria(&criteria, field_name)?;
    Ok(criteria)
}

pub fn validate_criteria(criteria: &[MetricCriterion], field_name: &str) -> Result<()> {
    for (index, item) in criteria.iter().enumerate() {
        if item.metric_key.trim().is_empty() {
            anyhow::bail!("{field_name}[{index}].metric_key must be non-empty");
        }
        if !VALID_OPERATORS.contains(&item.operator.as_str()) {
            anyhow::bail!(
                "{field_name}[{index}].operator must be one of {}",
                VALID_OPERATORS.join(", ")
            );
        }
    }
    Ok(())
}

pub fn evaluate_criteria(
    criteria: &[MetricCriterion],
    metrics: &BTreeMap<String, Decimal>,
) -> CriteriaEvaluation {
    let mut failures = Vec::new();

    for item in criteria {
        let Some(actual) = metrics.get(&item.metric_key) else {
            failures.push(format!("{} missing", item.metric_key));
            continue;
        };

        if !criterion_matches(*actual, item.target, &item.operator) {
            failures.push(format!(
                "{} {} {} (actual {})",
                item.metric_key, item.operator, item.target, actual
            ));
        }
    }

    CriteriaEvaluation {
        satisfied: failures.is_empty(),
        failures,
    }
}

fn criterion_matches(actual: Decimal, target: Decimal, operator: &str) -> bool {
    match operator {
        "<" => actual < target,
        "<=" => actual <= target,
        ">" => actual > target,
        ">=" => actual >= target,
        "==" => actual == target,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_parse_criteria_json() {
        let criteria = parse_criteria_json(
            Some(r#"[{"metric_key":"coverage","operator":">=","target":"90"}]"#),
            "acceptance_criteria",
        )
        .unwrap();

        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].metric_key, "coverage");
        assert_eq!(criteria[0].operator, ">=");
        assert_eq!(criteria[0].target, Decimal::from(90));
    }

    #[test]
    fn test_rejects_invalid_operator() {
        let err = parse_criteria_json(
            Some(r#"[{"metric_key":"coverage","operator":"!=","target":"90"}]"#),
            "acceptance_criteria",
        )
        .unwrap_err();

        assert!(err.to_string().contains("operator"));
    }

    #[test]
    fn test_evaluate_criteria() {
        let criteria = vec![
            MetricCriterion {
                metric_key: "coverage".to_string(),
                operator: ">=".to_string(),
                target: Decimal::from(90),
            },
            MetricCriterion {
                metric_key: "failures".to_string(),
                operator: "==".to_string(),
                target: Decimal::ZERO,
            },
        ];
        let metrics = BTreeMap::from([
            ("coverage".to_string(), Decimal::from_str("91.5").unwrap()),
            ("failures".to_string(), Decimal::ZERO),
        ]);

        let evaluation = evaluate_criteria(&criteria, &metrics);
        assert!(evaluation.satisfied);
        assert!(evaluation.failures.is_empty());
    }

    #[test]
    fn test_evaluate_reports_failures() {
        let criteria = vec![MetricCriterion {
            metric_key: "failures".to_string(),
            operator: "==".to_string(),
            target: Decimal::ZERO,
        }];
        let metrics = BTreeMap::from([("failures".to_string(), Decimal::from(3))]);

        let evaluation = evaluate_criteria(&criteria, &metrics);
        assert!(!evaluation.satisfied);
        assert_eq!(evaluation.failures, vec!["failures == 0 (actual 3)"]);
    }
}
