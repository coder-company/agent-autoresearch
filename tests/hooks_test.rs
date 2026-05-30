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

fn run_hook_in(dir: &std::path::Path, hook_name: &str, input: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("autoresearch")
        .unwrap()
        .current_dir(dir)
        .args(["hook", hook_name])
        .write_stdin(input)
        .assert()
}

fn write_scope_state(dir: &std::path::Path, scope: &[&str]) {
    let results = dir.join("autoresearch-results");
    std::fs::create_dir_all(&results).unwrap();
    let scope = scope
        .iter()
        .map(|item| serde_json::Value::String((*item).to_string()))
        .collect::<Vec<_>>();
    let state = serde_json::json!({
        "config": {
            "scope": scope
        }
    });
    std::fs::write(results.join("state.json"), state.to_string()).unwrap();
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

#[test]
fn test_scout_block_allows_in_scope_write() {
    let dir = tempfile::tempdir().unwrap();
    write_scope_state(dir.path(), &["src/**/*.rs"]);
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "file_path": "src/main.rs",
            "content": "fn main() {}"
        }
    });

    run_hook_in(dir.path(), "scout-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\"").not());
}

#[test]
fn test_scout_block_blocks_out_of_scope_write() {
    let dir = tempfile::tempdir().unwrap();
    write_scope_state(dir.path(), &["src/**/*.rs"]);
    let input = serde_json::json!({
        "tool_name": "Edit",
        "tool_input": {
            "file_path": "README.md",
            "old_string": "old",
            "new_string": "new"
        }
    });

    run_hook_in(dir.path(), "scout-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\""))
        .stdout(predicate::str::contains("outside autoresearch scope"));
}

#[test]
fn test_scout_block_blocks_workspace_escape() {
    let dir = tempfile::tempdir().unwrap();
    write_scope_state(dir.path(), &["src/**"]);
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "path": "../outside.rs",
            "content": "fn main() {}"
        }
    });

    run_hook_in(dir.path(), "scout-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\""))
        .stdout(predicate::str::contains("outside the workspace"));
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
fn test_privacy_block_catches_sensitive_paths() {
    for path in [
        ".env",
        ".env.local",
        "config/credentials.json",
        "config/.ssh/id_ed25519",
        ".aws/credentials",
        "secrets/api_key.txt",
    ] {
        let input = serde_json::json!({
            "tool_name": "Read",
            "tool_input": {
                "file_path": path
            }
        });

        run_hook("privacy-block", &input.to_string())
            .success()
            .stdout(predicate::str::contains("\"decision\":\"block\""));
    }
}

#[test]
fn test_privacy_block_allows_documented_exceptions_and_approved_paths() {
    for path in [".env.example", ".env.sample", "APPROVED:.env"] {
        let input = serde_json::json!({
            "tool_name": "Read",
            "tool_input": {
                "file_path": path
            }
        });

        run_hook("privacy-block", &input.to_string())
            .success()
            .stdout(predicate::str::contains("\"decision\":\"block\"").not());
    }
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
fn test_dangerous_cmd_blocks_destructive_git_cleanup() {
    for command in [
        "git push -f origin main",
        "push --force origin",
        "git clean -f",
        "git clean -fd",
        "git branch -D feature",
        "git checkout .",
        "git restore .",
    ] {
        let input = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": command
            }
        });

        run_hook("dangerous-cmd-block", &input.to_string())
            .success()
            .stdout(predicate::str::contains("\"decision\":\"block\""));
    }
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

    run_hook("iteration-context", &input.to_string()).success();
    // It either injects context (if TSV exists) or allows (if not)
    // Both are valid responses — the hook should not crash
}

#[test]
fn test_iteration_context_handles_empty_input() {
    run_hook("iteration-context", "{}").success();
}
