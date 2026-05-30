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
