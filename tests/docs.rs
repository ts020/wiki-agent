#[test]
fn readme_uses_current_fragments_output_structure() {
    let readme = include_str!("../README.md");
    assert!(readme.contains("├── fragments/"));
    assert!(!readme.contains("├── notes/"));
    assert!(!readme.contains("`notes/`"));
}

#[test]
fn readme_documents_qwen3_reading_order() {
    let readme = include_str!("../README.md");
    assert!(readme.contains("## Qwen3 推奨読み順"));
    assert!(readme.contains("agent/pages/index.md"));
    assert!(readme.contains("agent/terms/index.md"));
}
