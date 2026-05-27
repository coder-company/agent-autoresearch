use assert_cmd::Command;
use predicates::prelude::*;

/// Helper to run a hook with stdin input.
fn run_hook(hook_name: &str, input: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("autoresearch")
        .unwrap()
        .args(["hook", hook_name])
        .write_stdin(input)
        .assert()
}

// ── Scout Block ──────────────────────────────────────────────────────

#[test]
fn test_scout_block_allows_normal_files() {
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "path": "src/main.rs",
            "content": "fn main() {}"
        }
    });

    run_hook("scout-block", &input.to_string())
        .success()
        // Should not contain a block decision
        .stdout(predicate::str::contains("\"decision\":\"block\"").not());
}

#[test]
fn test_scout_block_allows_read_tools() {
    let input = serde_json::json!({
        "tool_name": "Read",
        "tool_input": {
            "path": "node_modules/express/index.js"
        }
    });

    // Read tools should always pass through scout-block
    run_hook("scout-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\"").not());
}

// ── Privacy Block ────────────────────────────────────────────────────

#[test]
fn test_privacy_block_catches_api_keys() {
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "path": "config.js",
            "content": "const config = { api_key: 'sk-abc123def456ghi789jkl012mno345pqr678stu901vwx234' }"
        }
    });

    run_hook("privacy-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\""));
}

#[test]
fn test_privacy_block_catches_aws_keys() {
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "path": "deploy.sh",
            "content": "export AWS_SECRET_KEY=mysecret"
        }
    });

    run_hook("privacy-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\""));
}

#[test]
fn test_privacy_block_catches_private_keys() {
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "path": "key.pem",
            "content": "-----BEGIN RSA PRIVATE KEY-----\nMIIE..."
        }
    });

    run_hook("privacy-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\""));
}

#[test]
fn test_privacy_block_allows_normal_content() {
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "path": "src/lib.rs",
            "content": "pub fn hello() -> &'static str { \"hello\" }"
        }
    });

    run_hook("privacy-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\"").not());
}

// ── Dangerous Command Block ──────────────────────────────────────────

#[test]
fn test_dangerous_cmd_blocks_rm_rf_root() {
    let input = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "rm -rf /"
        }
    });

    run_hook("dangerous-cmd-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\""));
}

#[test]
fn test_dangerous_cmd_blocks_force_push() {
    let input = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "git push --force origin main"
        }
    });

    run_hook("dangerous-cmd-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\""));
}

#[test]
fn test_dangerous_cmd_blocks_drop_table() {
    let input = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "psql -c 'DROP TABLE users'"
        }
    });

    run_hook("dangerous-cmd-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\""));
}

#[test]
fn test_dangerous_cmd_allows_safe_commands() {
    let input = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "cargo test"
        }
    });

    run_hook("dangerous-cmd-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\"").not());
}

#[test]
fn test_dangerous_cmd_ignores_non_bash_tools() {
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "command": "rm -rf /"
        }
    });

    // dangerous-cmd-block only checks Bash tool calls
    run_hook("dangerous-cmd-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\"").not());
}

// ── Iteration Context ────────────────────────────────────────────────

#[test]
fn test_iteration_context_injects_state() {
    // Without an active run directory, iteration-context should just allow
    let input = serde_json::json!({
        "prompt": "What should I do next?"
    });

    run_hook("iteration-context", &input.to_string())
        .success();
    // It either injects context (if TSV exists) or allows (if not)
    // Both are valid responses — the hook should not crash
}

#[test]
fn test_iteration_context_handles_empty_input() {
    run_hook("iteration-context", "{}")
        .success();
}
