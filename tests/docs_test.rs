use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn guide_index_links_every_top_level_guide() {
    let root = repo_root();
    let guide_dir = root.join("guide");
    let index = fs::read_to_string(guide_dir.join("README.md")).unwrap();

    for entry in fs::read_dir(&guide_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let filename = path.file_name().unwrap().to_str().unwrap();
        if filename == "README.md" {
            continue;
        }

        assert!(
            index.contains(filename),
            "guide/README.md does not link guide/{filename}"
        );
    }
}

#[test]
fn readme_links_required_docs_entrypoints() {
    let root = repo_root();
    let readme = fs::read_to_string(root.join("README.md")).unwrap();
    let required_docs = [
        "docs/README.md",
        "docs/INSTALL.md",
        "docs/GUIDE.md",
        "docs/EXAMPLES.md",
        "docs/system-architecture.md",
        "docs/project-changelog.md",
    ];

    for path in required_docs {
        assert!(
            root.join(path).is_file(),
            "required documentation entrypoint is missing: {path}"
        );
        assert!(
            readme.contains(path),
            "README.md does not link required documentation entrypoint: {path}"
        );
    }
}

#[test]
fn docs_site_summary_links_required_docs() {
    let root = repo_root();
    let book = fs::read_to_string(root.join("book.toml")).unwrap();
    let summary = fs::read_to_string(root.join("docs/SUMMARY.md")).unwrap();

    assert!(book.contains("src = \"docs\""));
    assert!(book.contains("build-dir = \"book\""));
    assert!(summary.contains("[Installation](INSTALL.md)"));
    assert!(summary.contains("[Guide](GUIDE.md)"));
    assert!(summary.contains("[Examples](EXAMPLES.md)"));
    assert!(summary.contains("[System Architecture](system-architecture.md)"));
    assert!(summary.contains("[Development Roadmap](development-roadmap.md)"));
    assert!(summary.contains("[中文](i18n/README_ZH.md)"));
}
