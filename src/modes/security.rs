//! STRIDE + OWASP security audit mode.
//!
//! Scans code for security vulnerabilities using STRIDE threat categories
//! and OWASP Top 10. Produces findings with severity, evidence, and
//! recommendations. Read-only by default, --fix flag for auto-remediation.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::RunConfig;

use super::{ModeDescription, ModeRunner};

/// STRIDE threat categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrideCategory {
    Spoofing,
    Tampering,
    Repudiation,
    InformationDisclosure,
    DenialOfService,
    ElevationOfPrivilege,
}

impl StrideCategory {
    /// All STRIDE categories.
    pub fn all() -> &'static [StrideCategory] {
        &[
            Self::Spoofing,
            Self::Tampering,
            Self::Repudiation,
            Self::InformationDisclosure,
            Self::DenialOfService,
            Self::ElevationOfPrivilege,
        ]
    }

    /// Short label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Spoofing => "Spoofing",
            Self::Tampering => "Tampering",
            Self::Repudiation => "Repudiation",
            Self::InformationDisclosure => "Information Disclosure",
            Self::DenialOfService => "Denial of Service",
            Self::ElevationOfPrivilege => "Elevation of Privilege",
        }
    }
}

/// OWASP Top 10 categories (2021).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwaspCategory {
    BrokenAccessControl,
    CryptographicFailures,
    Injection,
    InsecureDesign,
    SecurityMisconfiguration,
    VulnerableComponents,
    AuthenticationFailures,
    DataIntegrityFailures,
    LoggingFailures,
    Ssrf,
}

impl OwaspCategory {
    /// All OWASP Top 10 categories.
    pub fn all() -> &'static [OwaspCategory] {
        &[
            Self::BrokenAccessControl,
            Self::CryptographicFailures,
            Self::Injection,
            Self::InsecureDesign,
            Self::SecurityMisconfiguration,
            Self::VulnerableComponents,
            Self::AuthenticationFailures,
            Self::DataIntegrityFailures,
            Self::LoggingFailures,
            Self::Ssrf,
        ]
    }

    /// Human label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::BrokenAccessControl => "A01: Broken Access Control",
            Self::CryptographicFailures => "A02: Cryptographic Failures",
            Self::Injection => "A03: Injection",
            Self::InsecureDesign => "A04: Insecure Design",
            Self::SecurityMisconfiguration => "A05: Security Misconfiguration",
            Self::VulnerableComponents => "A06: Vulnerable Components",
            Self::AuthenticationFailures => "A07: Authentication Failures",
            Self::DataIntegrityFailures => "A08: Data Integrity Failures",
            Self::LoggingFailures => "A09: Logging Failures",
            Self::Ssrf => "A10: SSRF",
        }
    }
}

/// Severity levels for security findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
    Info = 4,
}

impl Severity {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Critical => "CRITICAL",
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Low => "LOW",
            Self::Info => "INFO",
        }
    }
}

/// A single security finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    /// Severity level.
    pub severity: Severity,
    /// STRIDE category, if applicable.
    pub stride: Option<StrideCategory>,
    /// OWASP category, if applicable.
    pub owasp: Option<OwaspCategory>,
    /// Short title.
    pub title: String,
    /// File where the issue was found.
    pub file: Option<String>,
    /// Line number.
    pub line: Option<u32>,
    /// Evidence (code snippet or description).
    pub evidence: String,
    /// Remediation recommendation.
    pub recommendation: String,
    /// Whether auto-fix has been applied.
    pub fixed: bool,
}

/// Security audit configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Whether to attempt auto-remediation.
    pub auto_fix: bool,
    /// Minimum severity to report.
    pub min_severity: Severity,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            auto_fix: false,
            min_severity: Severity::Low,
        }
    }
}

/// Security audit session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySession {
    /// Configuration for this audit.
    pub config: SecurityConfig,
    /// All findings.
    pub findings: Vec<SecurityFinding>,
}

impl SecuritySession {
    /// Create a new audit session.
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            config,
            findings: Vec::new(),
        }
    }

    /// Add a finding.
    pub fn add_finding(&mut self, finding: SecurityFinding) {
        self.findings.push(finding);
    }

    /// Get findings filtered by minimum severity.
    pub fn filtered_findings(&self) -> Vec<&SecurityFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity <= self.config.min_severity)
            .collect()
    }

    /// Count findings by severity.
    pub fn count_by_severity(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// Total unfixed findings.
    pub fn unfixed_count(&self) -> usize {
        self.findings.iter().filter(|f| !f.fixed).count()
    }
}

/// The STRIDE + OWASP security audit mode.
#[derive(Debug, Clone, Default)]
pub struct SecurityMode;

impl ModeRunner for SecurityMode {
    fn name(&self) -> &'static str {
        "security"
    }

    fn validate_config(&self, config: &RunConfig) -> Result<()> {
        if config.scope.is_empty() {
            bail!("Security mode requires at least one scope pattern to audit");
        }
        Ok(())
    }

    fn describe(&self) -> ModeDescription {
        ModeDescription {
            name: "security",
            purpose: "STRIDE + OWASP security audit with severity-ranked findings",
            default_iterations: Some(15),
            required_fields: &["scope"],
            optional_fields: &["goal", "iterations", "guard"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Direction;

    fn make_config() -> RunConfig {
        RunConfig {
            goal: "Security audit".into(),
            scope: vec!["src/**/*.rs".into()],
            metric: String::new(),
            direction: Direction::Lower,
            verify: String::new(),
            guard: None,
            iterations: Some(15),
            run_tag: None,
            stop_condition: None,
            verify_format: Default::default(),
            primary_metric_key: None,
            acceptance_criteria: Vec::new(),
            required_keep_criteria: Vec::new(),
            required_keep_labels: Vec::new(),
            required_stop_labels: Vec::new(),
            rollback_strategy: Default::default(),
            run_mode: None,
            workspace_root: None,
            primary_repo: None,
        }
    }

    #[test]
    fn test_validate_valid() {
        let mode = SecurityMode;
        assert!(mode.validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_validate_missing_scope() {
        let mode = SecurityMode;
        let mut config = make_config();
        config.scope = vec![];
        assert!(mode.validate_config(&config).is_err());
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical < Severity::High);
        assert!(Severity::High < Severity::Medium);
        assert!(Severity::Medium < Severity::Low);
        assert!(Severity::Low < Severity::Info);
    }

    #[test]
    fn test_session_filtered_findings() {
        let mut session = SecuritySession::new(SecurityConfig {
            auto_fix: false,
            min_severity: Severity::Medium,
        });
        session.add_finding(SecurityFinding {
            severity: Severity::Critical,
            stride: Some(StrideCategory::Tampering),
            owasp: Some(OwaspCategory::Injection),
            title: "SQL injection".into(),
            file: Some("src/db.rs".into()),
            line: Some(42),
            evidence: "raw query".into(),
            recommendation: "Use parameterized queries".into(),
            fixed: false,
        });
        session.add_finding(SecurityFinding {
            severity: Severity::Info,
            stride: None,
            owasp: None,
            title: "TODO comment".into(),
            file: Some("src/lib.rs".into()),
            line: Some(10),
            evidence: "// TODO: add auth".into(),
            recommendation: "Add authentication".into(),
            fixed: false,
        });

        let filtered = session.filtered_findings();
        // Only Critical is <= Medium in our Ord (lower discriminant = higher severity)
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "SQL injection");
    }

    #[test]
    fn test_stride_all_categories() {
        assert_eq!(StrideCategory::all().len(), 6);
    }

    #[test]
    fn test_owasp_all_categories() {
        assert_eq!(OwaspCategory::all().len(), 10);
    }
}
