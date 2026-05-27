/// These tests verify the state machine and escalation ladder by testing
/// via the CLI init → decide → status flow.

#[test]
fn test_state_from_baseline() {
    // We can't use the library types directly in integration tests without
    // re-exporting them, so we test via the CLI init → decide → status flow.
    // This test verifies the init command creates proper state.

    use assert_cmd::Command;
    use predicates::prelude::*;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let dir_path = dir.path();

    // Initialize a git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir_path)
        .output()
        .unwrap();

    // Create a metric file and commit
    std::fs::write(dir_path.join("metric.txt"), "50\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir_path)
        .output()
        .unwrap();

    // Run init
    Command::cargo_bin("autoresearch")
        .unwrap()
        .args([
            "init",
            "--verify",
            "cat metric.txt",
            "--direction",
            "higher",
            "--cwd",
            dir_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"baseline_metric\": \"50\""));

    // Verify state.json was created
    let state_path = dir_path.join("autoresearch-results/state.json");
    assert!(state_path.exists());

    let state_content = std::fs::read_to_string(&state_path).unwrap();
    assert!(state_content.contains("\"iteration\": 0"));
    assert!(state_content.contains("\"baseline_metric\": \"50\""));
}

#[test]
fn test_state_record_keep_then_discard() {
    use assert_cmd::Command;
    use predicates::prelude::*;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let dir_path = dir.path();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir_path)
        .output()
        .unwrap();

    // Add .gitignore for autoresearch-results (just like real projects)
    std::fs::write(dir_path.join(".gitignore"), "autoresearch-results/\n").unwrap();
    std::fs::write(dir_path.join("metric.txt"), "50\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir_path)
        .output()
        .unwrap();

    // Init
    Command::cargo_bin("autoresearch")
        .unwrap()
        .args([
            "init",
            "--verify",
            "cat metric.txt",
            "--direction",
            "higher",
            "--cwd",
            dir_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Simulate a keep: bump metric, commit, then decide keep
    std::fs::write(dir_path.join("metric.txt"), "60\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "experiment: bump"])
        .current_dir(dir_path)
        .output()
        .unwrap();

    let head = String::from_utf8(
        std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(dir_path)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    Command::cargo_bin("autoresearch")
        .unwrap()
        .args([
            "decide",
            "--decision",
            "keep",
            "--metric",
            "60",
            "--commit",
            &head,
            "--description",
            "bump metric",
            "--cwd",
            dir_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"decision\": \"keep\""));

    // Verify state shows 1 keep
    let state_content =
        std::fs::read_to_string(dir_path.join("autoresearch-results/state.json")).unwrap();
    assert!(state_content.contains("\"keeps\": 1"));
    assert!(state_content.contains("\"current_metric\": \"60\""));

    // Now simulate a discard
    Command::cargo_bin("autoresearch")
        .unwrap()
        .args([
            "decide",
            "--decision",
            "discard",
            "--metric",
            "45",
            "--description",
            "regression",
            "--cwd",
            dir_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"decision\": \"discard\""));

    // Verify state shows 1 discard
    let state_content =
        std::fs::read_to_string(dir_path.join("autoresearch-results/state.json")).unwrap();
    assert!(state_content.contains("\"discards\": 1"));
    assert!(state_content.contains("\"consecutive_discards\": 1"));
    // Current metric should still be 60 (discard doesn't update it)
    assert!(state_content.contains("\"current_metric\": \"60\""));
}

#[test]
fn test_state_status_shows_active_run() {
    use assert_cmd::Command;
    use predicates::prelude::*;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let dir_path = dir.path();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir_path)
        .output()
        .unwrap();

    std::fs::write(dir_path.join("metric.txt"), "75\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir_path)
        .output()
        .unwrap();

    // Init
    Command::cargo_bin("autoresearch")
        .unwrap()
        .args([
            "init",
            "--verify",
            "cat metric.txt",
            "--direction",
            "higher",
            "--cwd",
            dir_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Status should show active run
    Command::cargo_bin("autoresearch")
        .unwrap()
        .args(["status", "--cwd", dir_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"active\": true"));
}

#[test]
fn test_escalation_thresholds_via_consecutive_discards() {
    use assert_cmd::Command;
    use predicates::prelude::*;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let dir_path = dir.path();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir_path)
        .output()
        .unwrap();

    std::fs::write(dir_path.join("metric.txt"), "50\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir_path)
        .output()
        .unwrap();

    // Init
    Command::cargo_bin("autoresearch")
        .unwrap()
        .args([
            "init",
            "--verify",
            "cat metric.txt",
            "--direction",
            "higher",
            "--cwd",
            dir_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Issue 2 discards — no escalation yet
    for i in 0..2 {
        Command::cargo_bin("autoresearch")
            .unwrap()
            .args([
                "decide",
                "--decision",
                "discard",
                "--metric",
                "40",
                "--description",
                &format!("discard {}", i + 1),
                "--cwd",
                dir_path.to_str().unwrap(),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("\"action\": \"None\""));
    }

    // 3rd discard triggers REFINE
    Command::cargo_bin("autoresearch")
        .unwrap()
        .args([
            "decide",
            "--decision",
            "discard",
            "--metric",
            "40",
            "--description",
            "discard 3",
            "--cwd",
            dir_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"action\": \"Refine\""));

    // Check state shows 3 consecutive discards
    let state_content =
        std::fs::read_to_string(dir_path.join("autoresearch-results/state.json")).unwrap();
    assert!(state_content.contains("\"consecutive_discards\": 3"));

    // Issue 2 more discards (total 5) — should trigger PIVOT
    for _ in 3..4 {
        Command::cargo_bin("autoresearch")
            .unwrap()
            .args([
                "decide",
                "--decision",
                "discard",
                "--metric",
                "38",
                "--description",
                "discard 4",
                "--cwd",
                dir_path.to_str().unwrap(),
            ])
            .assert()
            .success();
    }

    // 5th discard triggers PIVOT
    Command::cargo_bin("autoresearch")
        .unwrap()
        .args([
            "decide",
            "--decision",
            "discard",
            "--metric",
            "38",
            "--description",
            "discard 5",
            "--cwd",
            dir_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"action\": \"Pivot\""));

    let state_content =
        std::fs::read_to_string(dir_path.join("autoresearch-results/state.json")).unwrap();
    assert!(state_content.contains("\"consecutive_discards\": 5"));
}
