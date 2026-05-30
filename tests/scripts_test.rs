use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn distribution_validator_passes_current_tree() {
    let root = repo_root();
    let script = root.join("scripts/validate_distribution.sh");

    let syntax = Command::new("bash")
        .arg("-n")
        .arg(&script)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        syntax.status.success(),
        "bash -n failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&syntax.stdout),
        String::from_utf8_lossy(&syntax.stderr)
    );

    let validation = Command::new(&script).current_dir(&root).output().unwrap();
    assert!(
        validation.status.success(),
        "distribution validation failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&validation.stdout),
        String::from_utf8_lossy(&validation.stderr)
    );
    assert!(String::from_utf8_lossy(&validation.stdout).contains("Distribution validation passed"));
}

#[test]
fn binary_skill_e2e_harness_passes() {
    let root = repo_root();
    let script = root.join("scripts/run_skill_e2e.sh");
    let bin = assert_cmd::cargo::cargo_bin("autoresearch");

    let syntax = Command::new("bash")
        .arg("-n")
        .arg(&script)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        syntax.status.success(),
        "bash -n failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&syntax.stdout),
        String::from_utf8_lossy(&syntax.stderr)
    );

    let smoke = Command::new(&script)
        .args(["binary-smoke", "--clean"])
        .env("AUTORESEARCH_BIN", bin)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "binary smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );
    assert!(String::from_utf8_lossy(&smoke.stdout).contains("binary smoke: OK"));
}

#[test]
fn multi_repo_skill_e2e_harness_passes() {
    let root = repo_root();
    let script = root.join("scripts/run_skill_e2e.sh");
    let bin = assert_cmd::cargo::cargo_bin("autoresearch");

    let smoke = Command::new(&script)
        .args(["multi-repo-smoke", "--clean"])
        .env("AUTORESEARCH_BIN", bin)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "multi-repo smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );
    assert!(String::from_utf8_lossy(&smoke.stdout).contains("multi-repo smoke: OK"));
}

#[test]
fn release_script_updates_agent_package_versions() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("scripts/release.sh")).unwrap();

    assert!(script.contains("update_json_version"));
    assert!(script.contains(".claude-plugin/plugin.json"));
    assert!(script.contains(".claude-plugin/marketplace.json"));
    assert!(script.contains("plugins/autoresearch/.codex-plugin/plugin.json"));
    assert!(script.contains("$VERSION-codex.0"));
    assert!(script.contains("skills/autoresearch/SKILL.md"));
    assert!(script.contains(".agents/skills/autoresearch/SKILL.md"));
    assert!(script.contains("\"$ROOT/scripts/transform.sh\""));
    assert!(script.contains("plugins/autoresearch/skills/autoresearch"));
}

#[test]
fn release_script_blocks_untracked_dirty_worktrees() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("scripts/release.sh")).unwrap();

    assert!(script.contains("git -C \"$ROOT\" status --porcelain"));
    assert!(!script.contains("git -C \"$ROOT\" diff --quiet HEAD"));
}
