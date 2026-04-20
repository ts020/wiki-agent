use std::fmt::Write;

use crate::extract::{EntryPoint, TechStack, TestLayout};

pub fn render_tech_stack(stack: &TechStack) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Tech stack");
    s.push('\n');

    let _ = writeln!(&mut s, "## Languages");
    s.push('\n');
    if stack.languages.is_empty() {
        let _ = writeln!(&mut s, "_(none detected)_");
    } else {
        for l in &stack.languages {
            let _ = writeln!(&mut s, "- {l}");
        }
    }
    s.push('\n');

    let _ = writeln!(&mut s, "## Manifests");
    s.push('\n');
    if stack.manifests.is_empty() {
        let _ = writeln!(&mut s, "_(none detected)_");
    } else {
        for m in &stack.manifests {
            let header = match &m.project_name {
                Some(n) => format!("`{}` — {} ({})", m.file.display(), n, m.kind.label()),
                None => format!("`{}` ({})", m.file.display(), m.kind.label()),
            };
            let _ = writeln!(&mut s, "- {header}");
            if !m.dependencies.is_empty() {
                let deps = m.dependencies.join(", ");
                let _ = writeln!(&mut s, "    - dependencies: {deps}");
            }
        }
    }
    s.push('\n');
    s
}

pub fn render_entry_points(entries: &[EntryPoint]) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Entry points");
    s.push('\n');
    if entries.is_empty() {
        let _ = writeln!(&mut s, "_(none detected)_");
    } else {
        for e in entries {
            let _ = writeln!(
                &mut s,
                "- `{}` — {} ({})",
                e.file.display(),
                e.language,
                e.description
            );
        }
    }
    s.push('\n');
    s
}

pub fn render_tests(layout: &TestLayout) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Tests");
    s.push('\n');

    let _ = writeln!(&mut s, "## Directories");
    s.push('\n');
    if layout.test_dirs.is_empty() {
        let _ = writeln!(&mut s, "_(none detected)_");
    } else {
        for d in &layout.test_dirs {
            let _ = writeln!(&mut s, "- `{}`", d.display());
        }
    }
    s.push('\n');

    let _ = writeln!(&mut s, "## Files");
    s.push('\n');
    if layout.test_files.is_empty() {
        let _ = writeln!(&mut s, "_(none detected)_");
    } else {
        for f in &layout.test_files {
            let _ = writeln!(&mut s, "- `{}`", f.display());
        }
    }
    s.push('\n');
    s
}
