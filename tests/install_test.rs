#[test]
fn codex_installer_replaces_skill_dir() {
    let script = include_str!("../install.sh");
    let guard = script
        .find("ensure_safe_codex_skill_dir \"$skill_dir\"")
        .unwrap();
    let remove = script.find("rm -rf \"$skill_dir\"").unwrap();

    assert!(guard < remove);
    assert!(script.contains("rm -rf \"$skill_dir\"\n            mkdir -p \"$skill_dir\""));
    assert!(!script.contains("rm -rf \"$skill_dir/autoresearch\""));
}

#[test]
fn codex_installer_rejects_unsafe_skill_targets() {
    let script = include_str!("../install.sh");

    assert!(script.contains("ensure_safe_codex_skill_dir()"));
    assert!(script.contains("Refusing empty Codex skill path."));
    assert!(script.contains("\"/\"|\"$HOME\"|\"$HOME/.codex\"|\"$HOME/.codex/skills\""));
    assert!(script.contains("${dir##*/}"));
    assert!(script.contains("!= \"autoresearch\""));
}

#[test]
fn codex_installer_uses_maintained_agents_package() {
    let script = include_str!("../install.sh");

    assert!(script.contains("$REPO_DIR/.agents/skills/autoresearch"));
    assert!(script.contains("cp -R \"$REPO_DIR/.agents/skills/autoresearch/.\" \"$skill_dir/\""));
}
