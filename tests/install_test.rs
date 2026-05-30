#[test]
fn codex_installer_replaces_skill_dir() {
    let script = include_str!("../install.sh");

    assert!(script.contains("rm -rf \"$skill_dir\"\n            mkdir -p \"$skill_dir\""));
    assert!(!script.contains("rm -rf \"$skill_dir/autoresearch\""));
}

#[test]
fn codex_installer_uses_maintained_agents_package() {
    let script = include_str!("../install.sh");

    assert!(script.contains("$REPO_DIR/.agents/skills/autoresearch"));
    assert!(script.contains("cp -R \"$REPO_DIR/.agents/skills/autoresearch/.\" \"$skill_dir/\""));
}
