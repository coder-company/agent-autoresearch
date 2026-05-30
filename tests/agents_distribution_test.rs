use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn extract_reference_links(content: &str) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    let mut offset = 0;

    while let Some(relative_start) = content[offset..].find("references/") {
        let start = offset + relative_start;
        let after_prefix = start + "references/".len();
        let Some(relative_end) = content[after_prefix..].find(".md") else {
            break;
        };
        let end = after_prefix + relative_end + ".md".len();
        refs.insert(content[start..end].to_string());
        offset = end;
    }

    refs
}

#[test]
fn codex_skill_lists_packaged_references() {
    let root = repo_root();
    let skill_path = root.join(".agents/skills/autoresearch/SKILL.md");
    let skill = fs::read_to_string(&skill_path).unwrap();
    let refs = extract_reference_links(&skill);

    for required in [
        "references/core-principles.md",
        "references/runtime-protocol.md",
        "references/interaction-wizard.md",
        "references/session-resume.md",
        "references/escalation.md",
        "references/health-check-protocol.md",
        "references/results-logging.md",
        "references/environment-awareness.md",
        "references/parallel-experiments-protocol.md",
        "references/pivot-protocol.md",
        "references/web-search-protocol.md",
        "references/lessons-protocol.md",
    ] {
        assert!(
            refs.contains(required),
            "missing Codex skill reference: {required}"
        );
    }

    for reference in refs {
        let packaged = root.join(".agents/skills/autoresearch").join(&reference);
        assert!(
            packaged.is_file(),
            "Codex skill lists {reference}, but the packaged file is missing"
        );
    }
}

#[test]
fn codex_skill_reference_links_are_closed_and_synced() {
    let root = repo_root();
    let package_root = root.join(".agents/skills/autoresearch");
    let packaged_references = package_root.join("references");

    for entry in fs::read_dir(&packaged_references).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap();
        for reference in extract_reference_links(&content) {
            assert!(
                package_root.join(&reference).is_file(),
                "{} links to {reference}, but it is not packaged",
                path.display()
            );
        }

        let relative = path.strip_prefix(&package_root).unwrap();
        let canonical = root.join(relative);
        assert!(
            canonical.is_file(),
            "packaged reference {} has no canonical source",
            path.display()
        );
        assert_eq!(
            fs::read_to_string(&canonical).unwrap(),
            content,
            "packaged reference {} drifted from {}",
            path.display(),
            canonical.display()
        );
    }
}

#[test]
fn codex_skill_packages_openai_agent_metadata() {
    let root = repo_root();
    let canonical = root.join("agents/skill-openai.yaml");
    let packaged = root.join(".agents/skills/autoresearch/agents/openai.yaml");
    let packaged_content = fs::read_to_string(&packaged).unwrap();

    assert!(packaged.is_file(), "missing packaged OpenAI agent metadata");
    assert_eq!(
        fs::read_to_string(&canonical).unwrap(),
        packaged_content,
        "packaged OpenAI agent metadata drifted from agents/skill-openai.yaml"
    );
    for unsupported in ["name:", "description:", "model:", "tools:"] {
        assert!(
            !packaged_content
                .lines()
                .any(|line| line.trim_start().starts_with(unsupported)),
            "packaged skill agent metadata contains unsupported field {unsupported}"
        );
    }
}

#[test]
fn codex_plugin_packages_synced_skill() {
    let root = repo_root();
    let manifest_path = root.join("plugins/autoresearch/.codex-plugin/plugin.json");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();

    assert_eq!(manifest["name"], "autoresearch");
    assert_eq!(manifest["skills"], "./skills/");
    assert!(
        manifest["interface"]["defaultPrompt"]
            .as_array()
            .unwrap()
            .iter()
            .any(|prompt| prompt.as_str().unwrap().contains("$autoresearch")),
        "plugin manifest should expose $autoresearch prompts"
    );

    let packaged_skill = root.join("plugins/autoresearch/skills/autoresearch/SKILL.md");
    assert_eq!(
        fs::read_to_string(root.join(".agents/skills/autoresearch/SKILL.md")).unwrap(),
        fs::read_to_string(&packaged_skill).unwrap(),
        "Codex plugin skill drifted from .agents skill package"
    );
}
