#[test]
fn readme_uses_current_fragments_output_structure() {
    let readme = include_str!("../README.md");
    assert!(readme.contains("├── fragments/"));
    assert!(!readme.contains("├── notes/"));
    assert!(!readme.contains("`notes/`"));
}
