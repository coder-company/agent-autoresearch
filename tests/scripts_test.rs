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
fn skill_e2e_builds_current_binary_without_override() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("scripts/run_skill_e2e.sh")).unwrap();
    let helper = script
        .split_once("autoresearch_bin()")
        .unwrap()
        .1
        .split_once("init_fixture_repo()")
        .unwrap()
        .0;

    assert!(helper.contains("AUTORESEARCH_BIN"));
    assert!(helper.contains("cargo build --manifest-path \"$ROOT/Cargo.toml\" >/dev/null"));
    assert!(!helper.contains("if [[ ! -x \"$bin\" ]]"));
}

#[test]
fn runtime_skill_e2e_harness_passes() {
    let root = repo_root();
    let script = root.join("scripts/run_skill_e2e.sh");
    let bin = assert_cmd::cargo::cargo_bin("autoresearch");

    let smoke = Command::new(&script)
        .args(["runtime-smoke", "--clean"])
        .env("AUTORESEARCH_BIN", bin)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "runtime smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );
    assert!(String::from_utf8_lossy(&smoke.stdout).contains("runtime smoke: OK"));
}

#[test]
fn parallel_skill_e2e_harness_passes() {
    let root = repo_root();
    let script = root.join("scripts/run_skill_e2e.sh");
    let bin = assert_cmd::cargo::cargo_bin("autoresearch");

    let smoke = Command::new(&script)
        .args(["parallel-smoke", "--clean"])
        .env("AUTORESEARCH_BIN", bin)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        smoke.status.success(),
        "parallel smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );
    assert!(String::from_utf8_lossy(&smoke.stdout).contains("parallel smoke: OK"));
}

#[test]
fn release_script_updates_agent_package_versions() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("scripts/release.sh")).unwrap();

    assert!(script.contains("update_json_version"));
    assert!(script.contains("update_cargo_version"));
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
fn release_script_uses_portable_file_rewrites() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("scripts/release.sh")).unwrap();

    assert!(script.contains("awk -v version="));
    assert!(script.contains("tmp=\"$(mktemp)\""));
    assert!(script.contains("mv \"$tmp\" \"$path\""));
    assert!(script.contains("update_cargo_version \"$ROOT/Cargo.toml\" \"$VERSION\""));
    assert!(!script.contains("sed -i"));
    assert!(!script.contains("0,/^version:"));
}

#[test]
fn release_script_avoids_mapfile_for_macos_bash() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("scripts/release.sh")).unwrap();

    assert!(script.contains("collect_change_lines()"));
    assert!(script.contains("while IFS= read -r line"));
    assert!(script.contains("CHANGE_LINES+=(\"$line\")"));
    assert!(!script.contains("mapfile"));
}

#[test]
fn release_script_blocks_untracked_dirty_worktrees() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("scripts/release.sh")).unwrap();

    assert!(script.contains("git -C \"$ROOT\" status --porcelain"));
    assert!(!script.contains("git -C \"$ROOT\" diff --quiet HEAD"));
}

#[test]
fn release_script_enforces_release_binary_size() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("scripts/release.sh")).unwrap();

    assert!(script.contains("MAX_RELEASE_BINARY_BYTES=$((5 * 1024 * 1024))"));
    assert!(script.contains("wc -c < \"$RELEASE_BINARY\""));
    assert!(script.contains("release binary is too large"));
    assert!(script.contains("cargo fmt --manifest-path \"$ROOT/Cargo.toml\" -- --check"));
    assert!(script.contains("\"$ROOT/scripts/validate_distribution.sh\""));
    assert!(script.contains("[10/10] Committing and tagging"));
}

#[test]
fn release_script_generates_non_placeholder_changelog_notes() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("scripts/release.sh")).unwrap();

    assert!(script.contains("git -C \"$ROOT\" log --format='- %s'"));
    assert!(script.contains("CHANGE_LINES"));
    assert!(script.contains("Added changelog entry for v$VERSION from recent commit subjects."));
    assert!(!script.contains("TODO: Fill in changes for this release"));
}

#[test]
fn contributor_gate_enforces_release_binary_size() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("scripts/run_contributor_gate.sh")).unwrap();

    assert!(script.contains("MAX_RELEASE_BINARY_BYTES=$((5 * 1024 * 1024))"));
    assert!(script.contains("wc -c < \"$RELEASE_BINARY\""));
    assert!(script.contains("Release binary is too large"));
    assert!(script.contains("for script in install.sh scripts/*.sh tests/*.sh"));
    assert!(script.contains("bash -n \"$script\""));
}

#[test]
fn ci_workflow_runs_full_contributor_gate_with_operational_guards() {
    let root = repo_root();
    let workflow = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();

    assert!(workflow.contains("workflow_dispatch:"));
    assert!(workflow.contains("permissions:\n  contents: read"));
    assert!(workflow.contains("concurrency:"));
    assert!(workflow.contains("cancel-in-progress: true"));
    assert!(workflow.contains("timeout-minutes: 25"));
    assert!(workflow.contains("actions/cache@v4"));
    assert!(workflow.contains("tests/*.sh"));
    assert!(workflow.contains("bash -n \"$script\""));
    assert!(workflow.contains("./scripts/run_contributor_gate.sh"));
}

#[test]
fn release_workflow_builds_prebuilt_binary_matrix() {
    let root = repo_root();
    let workflow = std::fs::read_to_string(root.join(".github/workflows/release.yml")).unwrap();

    assert!(workflow.contains("tags:"));
    assert!(workflow.contains("\"v*\""));
    assert!(workflow.contains("workflow_dispatch:"));
    assert!(workflow.contains("linux-x86_64"));
    assert!(workflow.contains("linux-aarch64"));
    assert!(workflow.contains("macos-x86_64"));
    assert!(workflow.contains("macos-aarch64"));
    assert!(workflow.contains("windows-x86_64"));
    assert!(workflow.contains("ubuntu-24.04-arm"));
    assert!(workflow.contains("macos-15-intel"));
    assert!(workflow.contains("macos-14"));
    assert!(workflow.contains("cargo build --locked --release --target ${{ matrix.target }}"));
    assert!(workflow.contains("actions/upload-artifact@v4"));
    assert!(workflow.contains("actions/download-artifact@v4"));
    assert!(workflow.contains("gh release upload \"$TAG\" --clobber"));
    assert!(workflow.contains("sha256sum"));
    assert!(workflow.contains("shasum -a 256"));
}
