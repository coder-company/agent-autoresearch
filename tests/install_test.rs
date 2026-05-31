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

#[test]
fn codex_plugin_installer_uses_local_marketplace_package() {
    let script = include_str!("../install.sh");

    assert!(script.contains("--codex-plugin"));
    assert!(script.contains("INSTALL_CODEX_PLUGIN=1"));
    assert!(script.contains("$REPO_DIR/.agents/plugins/marketplace.json"));
    assert!(script.contains("$REPO_DIR/plugins/autoresearch/.codex-plugin/plugin.json"));
    assert!(script
        .contains("codex plugin marketplace add \"$REPO_DIR/.agents/plugins/marketplace.json\""));
    assert!(script.contains("codex plugin install autoresearch@autoresearch-local"));
}

#[test]
fn vscode_installer_copies_extension_package() {
    let script = include_str!("../install.sh");

    assert!(script.contains("--vscode"));
    assert!(script.contains("INSTALL_VSCODE=1"));
    assert!(script.contains("--vscode-dir"));
    assert!(script.contains("ensure_safe_vscode_extension_dir()"));
    assert!(script.contains("$REPO_DIR/integrations/vscode/package.json"));
    assert!(script.contains("cp -R \"$REPO_DIR/integrations/vscode/.\" \"$extension_dir/\""));
    assert!(script.contains("Reload VS Code, then run: Autoresearch: Show Status"));
    assert!(script.contains("install_vscode_extension"));
}

#[test]
fn vscode_installer_guards_before_replacing_extension_dir() {
    let script = include_str!("../install.sh");
    let guard = script
        .find("ensure_safe_vscode_extension_dir \"$extension_root\"")
        .unwrap();
    let remove = script.find("rm -rf \"$extension_dir\"").unwrap();

    assert!(guard < remove);
    assert!(script.contains("\"/\"|\"$HOME\"|\"$HOME/.vscode\""));
}

#[test]
fn installer_supports_local_and_global_copy_targets() {
    let script = include_str!("../install.sh");

    assert!(script.contains("-g|--global"));
    assert!(script.contains("-l|--local"));
    assert!(script.contains("INSTALL_SCOPE=\"global\""));
    assert!(script.contains("Choose either --global or --local, not both."));
    assert!(script.contains("LAUNCH_DIR=\"$(pwd)\""));
    assert!(script.contains("target_root=\"$LAUNCH_DIR/.opencode\""));
    assert!(script.contains(
        "mkdir -p \"$opencode_dir/skills\" \"$opencode_dir/commands\" \"$opencode_dir/agents\""
    ));
    assert!(script.contains("cp \"$REPO_DIR\"/.opencode/agents/*.md \"$opencode_dir/agents/\""));
    assert!(script.contains("target_dir=\"$LAUNCH_DIR/.codex/skills/autoresearch\""));
    assert!(script.contains("cargo build --manifest-path \"$REPO_DIR/Cargo.toml\" --release"));
    assert!(!script.contains("\n    cd \"$REPO_DIR\"\n\n    cargo build --release"));
    assert!(script.contains("INSTALL_VSCODE=1"));
}

#[test]
fn opencode_installer_rejects_unsafe_config_roots() {
    let script = include_str!("../install.sh");

    assert!(script.contains("ensure_safe_opencode_dir()"));
    assert!(script.contains("Refusing empty OpenCode config path."));
    assert!(script.contains("\"/\"|\"$HOME\"|\"$HOME/.config\""));
    assert!(script.contains("ensure_safe_opencode_dir \"$opencode_dir\""));
    let guard = script
        .find("ensure_safe_opencode_dir \"$opencode_dir\"")
        .unwrap();
    let remove = script
        .find("rm -rf \"$opencode_dir/skills/autoresearch\"")
        .unwrap();
    assert!(guard < remove);
}
