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
