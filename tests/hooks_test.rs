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

fn run_hook_disabled(hook_name: &str, env_name: &str, input: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("autoresearch")
        .unwrap()
        .env(env_name, "1")
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

fn init_git_repo(dir: &std::path::Path) {
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::fs::write(dir.join("README.md"), "initial\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir)
        .output()
        .unwrap();
}

fn write_changed_lines(dir: &std::path::Path, count: usize) {
    let content = (0..count)
        .map(|index| format!("line {index}\n"))
        .collect::<String>();
    std::fs::write(dir.join("README.md"), content).unwrap();
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
            "path": "src/main.rs"
        }
    });

    run_hook("scout-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\"").not());
}

#[test]
fn test_scout_block_blocks_generated_vendor_and_sensitive_paths() {
    for path in [
        "node_modules/express/index.js",
        ".git/config",
        "dist/bundle.js",
        "coverage/index.html",
        ".venv/lib/python/site-packages/pkg.py",
        ".ssh/id_rsa",
        "debug.log",
    ] {
        let input = serde_json::json!({
            "tool_name": "Read",
            "tool_input": {
                "path": path
            }
        });

        run_hook("scout-block", &input.to_string())
            .success()
            .stdout(predicate::str::contains("\"decision\":\"block\""));
    }
}

#[test]
fn test_scout_block_blocks_bash_reads_of_ignored_paths() {
    for command in [
        "cat node_modules/foo/bar.js",
        "cat .git/HEAD",
        "grep TODO coverage/index.html",
        "RUST_LOG=debug rg token .aws/credentials",
    ] {
        let input = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": command
            }
        });

        run_hook("scout-block", &input.to_string())
            .success()
            .stdout(predicate::str::contains("\"decision\":\"block\""));
    }
}

#[test]
fn test_scout_block_allows_bash_build_commands_and_plain_text() {
    for command in [
        "npm test",
        "cargo test",
        "python script.py",
        "echo 'testing node_modules string'",
    ] {
        let input = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": command
            }
        });

        run_hook("scout-block", &input.to_string())
            .success()
            .stdout(predicate::str::contains("\"decision\":\"block\"").not());
    }
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

#[test]
fn test_hook_disable_env_allows_scout_block() {
    let dir = tempfile::tempdir().unwrap();
    write_scope_state(dir.path(), &["src/**"]);
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {
            "path": "../outside.rs",
            "content": "fn main() {}"
        }
    });

    Command::cargo_bin("autoresearch")
        .unwrap()
        .current_dir(dir.path())
        .env("AR_DISABLE_SCOUT_BLOCK", "1")
        .args(["hook", "scout-block"])
        .write_stdin(input.to_string())
        .assert()
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
fn test_privacy_block_warns_for_bash_sensitive_paths() {
    let input = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "cat .env"
        }
    });

    run_hook("privacy-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"additionalContext\""))
        .stdout(predicate::str::contains("\"decision\":\"block\"").not());
}

#[test]
fn test_privacy_block_allows_bash_documented_exceptions() {
    let input = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "cat .env.example"
        }
    });

    run_hook("privacy-block", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"additionalContext\"").not())
        .stdout(predicate::str::contains("\"decision\":\"block\"").not());
}

#[test]
fn test_hook_disable_env_allows_privacy_block() {
    let input = serde_json::json!({
        "tool_name": "Read",
        "tool_input": {
            "file_path": ".env"
        }
    });

    run_hook_disabled(
        "privacy-block",
        "AR_DISABLE_PRIVACY_BLOCK",
        &input.to_string(),
    )
    .success()
    .stdout(predicate::str::contains("\"decision\":\"block\"").not());
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

#[test]
fn test_hook_disable_env_allows_dangerous_cmd_block() {
    let input = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "git push --force origin main"
        }
    });

    run_hook_disabled(
        "dangerous-cmd-block",
        "AR_DISABLE_DANGEROUS_CMD_BLOCK",
        &input.to_string(),
    )
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

#[test]
fn test_iteration_context_throttles_by_session_id() {
    let dir = tempfile::tempdir().unwrap();
    let results = dir.path().join("autoresearch-results");
    std::fs::create_dir_all(&results).unwrap();
    std::fs::write(
        results.join("results.tsv"),
        "iteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n0\tabc\t10\t0\t-\tbaseline\tinitial\n1\tdef\t11\t+1\tpass\tkeep\timproved\n",
    )
    .unwrap();
    let input = serde_json::json!({
        "session_id": "throttle-test",
        "prompt": "autoresearch status"
    });

    for _ in 0..4 {
        run_hook_in(dir.path(), "iteration-context", &input.to_string())
            .success()
            .stdout(predicate::str::contains("\"additionalContext\"").not());
    }

    run_hook_in(dir.path(), "iteration-context", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"additionalContext\""))
        .stdout(predicate::str::contains("Active iteration state"));
}

// ── Dev Rules Reminder ───────────────────────────────────────────────

#[test]
fn test_dev_rules_reminder_throttles_by_session_id() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plans")).unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    let input = serde_json::json!({
        "session_id": "dev-rules-test"
    });

    for _ in 0..4 {
        run_hook_in(dir.path(), "dev-rules-reminder", &input.to_string())
            .success()
            .stdout(predicate::str::contains("\"additionalContext\"").not());
    }

    run_hook_in(dir.path(), "dev-rules-reminder", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"additionalContext\""))
        .stdout(predicate::str::contains("Dev context"))
        .stdout(predicate::str::contains("docs/code-standards.md"));
}

// ── Subagent Context ─────────────────────────────────────────────────

#[test]
fn test_subagent_context_injects_project_and_tsv_summary() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    let subdir = dir.path().join("src");
    std::fs::create_dir_all(&subdir).unwrap();
    let results = dir.path().join("autoresearch-results");
    std::fs::create_dir_all(&results).unwrap();
    std::fs::write(
        results.join("state.json"),
        serde_json::json!({
            "iteration": 2,
            "current_metric": "12",
            "last_status": "keep"
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        results.join("results.tsv"),
        "iteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n0\tabc\t10\t0\t-\tbaseline\tinitial\n2\tdef\t12\t+2\tpass\tkeep\timproved\n",
    )
    .unwrap();

    run_hook_in(&subdir, "subagent-context", "{}")
        .success()
        .stdout(predicate::str::contains(
            "Autoresearch context (for subagent)",
        ))
        .stdout(predicate::str::contains(
            "Active TSV: autoresearch-results/results.tsv",
        ))
        .stdout(predicate::str::contains("Iteration: 2"))
        .stdout(predicate::str::contains("Latest: iteration=2"))
        .stdout(predicate::str::contains("Metric: 12"));
}

#[test]
fn test_subagent_context_uses_tsv_without_state() {
    let dir = tempfile::tempdir().unwrap();
    let results = dir.path().join("autoresearch-results");
    std::fs::create_dir_all(&results).unwrap();
    std::fs::write(
        results.join("results.tsv"),
        "iteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n1\tabc\t7\t+1\tpass\tkeep\timproved\n",
    )
    .unwrap();

    run_hook_in(dir.path(), "subagent-context", "{}")
        .success()
        .stdout(predicate::str::contains(
            "Autoresearch context (for subagent)",
        ))
        .stdout(predicate::str::contains("Latest: iteration=1"))
        .stdout(predicate::str::contains("Metric: ?"));
}

// ── Session Init ─────────────────────────────────────────────────────

#[test]
fn test_session_init_injects_project_context() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());

    run_hook_in(dir.path(), "session-init", "{}")
        .success()
        .stdout(predicate::str::contains("\"additionalContext\""))
        .stdout(predicate::str::contains("Session initialized"))
        .stdout(predicate::str::contains(dir.path().to_str().unwrap()));
}

#[test]
fn test_session_init_includes_resumable_run_context() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    let subdir = dir.path().join("src");
    std::fs::create_dir_all(&subdir).unwrap();
    let results = dir.path().join("autoresearch-results");
    std::fs::create_dir_all(&results).unwrap();
    let state = serde_json::json!({
        "iteration": 3,
        "current_metric": "42",
        "phase": {
            "phase": "iterating",
            "iteration": 3,
            "current_metric": "42",
            "best_metric": "42",
            "best_iteration": 3
        }
    });
    std::fs::write(results.join("state.json"), state.to_string()).unwrap();

    run_hook_in(&subdir, "session-init", "{}")
        .success()
        .stdout(predicate::str::contains("Session initialized"))
        .stdout(predicate::str::contains(
            "Resumable autoresearch run detected",
        ))
        .stdout(predicate::str::contains("iteration 3"));
}

// ── Session End ──────────────────────────────────────────────────────

#[test]
fn test_session_end_emits_terminal_notification() {
    let dir = tempfile::tempdir().unwrap();
    let results = dir.path().join("autoresearch-results");
    std::fs::create_dir_all(&results).unwrap();
    std::fs::write(
        results.join("results.tsv"),
        "iteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n0\tabc\t10\t0\t-\tbaseline\tinitial\n",
    )
    .unwrap();

    run_hook_in(dir.path(), "session-end", "{}")
        .success()
        .stdout(predicate::str::contains("\"terminalSequence\""))
        .stdout(predicate::str::contains("Session completed"))
        .stdout(predicate::str::contains("1 iterations, metric: 10"));
}

// ── Simplify Gate ────────────────────────────────────────────────────

#[test]
fn test_simplify_gate_blocks_large_shipping_diff() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    write_changed_lines(dir.path(), 900);
    let input = serde_json::json!({
        "prompt": "ship this"
    });

    run_hook_in(dir.path(), "simplify-gate", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\""))
        .stdout(predicate::str::contains("shipping threshold"));
}

#[test]
fn test_simplify_gate_warns_medium_shipping_diff() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    write_changed_lines(dir.path(), 450);
    let input = serde_json::json!({
        "prompt": "merge this"
    });

    run_hook_in(dir.path(), "simplify-gate", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"additionalContext\""))
        .stdout(predicate::str::contains("\"decision\":\"block\"").not());
}

#[test]
fn test_simplify_gate_allows_negated_shipping_prompt() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    write_changed_lines(dir.path(), 900);
    let input = serde_json::json!({
        "prompt": "don't ship yet"
    });

    run_hook_in(dir.path(), "simplify-gate", &input.to_string())
        .success()
        .stdout(predicate::str::contains("\"decision\":\"block\"").not())
        .stdout(predicate::str::contains("\"additionalContext\"").not());
}
