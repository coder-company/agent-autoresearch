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
