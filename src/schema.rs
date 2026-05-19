use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::link::LinkGraph;
use crate::metadata_renderer::markdown_path;
use crate::model::{Node, PageKind, iter_pages};
use crate::notes::headings;
use crate::output_plan::{OutputPlan, insert_bytes, insert_text};

#[derive(Debug, Clone, Deserialize)]
pub struct SchemaPack {
    pub id: String,
    pub version: u32,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldDef>,
    #[serde(default)]
    pub contexts: BTreeMap<String, ContextRecipe>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldDef {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub sources: Vec<FieldSource>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldSource {
    #[serde(default)]
    pub frontmatter: Option<String>,
    #[serde(default)]
    pub heading: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextRecipe {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub default_budget_chars: Option<usize>,
    #[serde(default)]
    pub sections: Vec<ContextSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextSection {
    pub title: String,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalCatalog {
    pub schema_version: u32,
    pub schema: CatalogSchema,
    pub pages: Vec<CatalogPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSchema {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogPage {
    pub generated_path: String,
    pub source_path: String,
    pub source_range: SourceRange,
    pub title: String,
    pub doc_type: String,
    pub entities: Vec<String>,
    pub tags: Vec<String>,
    pub headings: Vec<String>,
    pub fields: BTreeMap<String, Vec<FieldEvidence>>,
    pub outgoing_links: Vec<String>,
    pub backlinks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRange {
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldEvidence {
    pub text: String,
    pub source_path: String,
    pub line_start: usize,
    pub line_end: usize,
}

pub fn load_schema(path: &Path) -> Result<SchemaPack> {
    let body =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let schema: SchemaPack = serde_yaml::from_str(&body)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    schema.validate()?;
    Ok(schema)
}

impl SchemaPack {
    fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            anyhow::bail!("schema id is required");
        }
        if self.version != 1 {
            anyhow::bail!("unsupported schema version: {}", self.version);
        }
        if self.fields.is_empty() {
            anyhow::bail!("schema fields are required");
        }
        if self.contexts.is_empty() {
            anyhow::bail!("schema contexts are required");
        }
        for (name, field) in &self.fields {
            if field.sources.is_empty() {
                anyhow::bail!("field `{name}` must define at least one source");
            }
            for source in &field.sources {
                if source.frontmatter.is_none() && source.heading.is_none() {
                    anyhow::bail!("field `{name}` source must define frontmatter or heading");
                }
            }
        }
        for (name, context) in &self.contexts {
            if context.sections.is_empty() {
                anyhow::bail!("context `{name}` must define at least one section");
            }
            for section in &context.sections {
                let is_sources = section.kind.as_deref() == Some("sources");
                if section.fields.is_empty() && !is_sources {
                    anyhow::bail!(
                        "context `{name}` section `{}` must define fields or kind: sources",
                        section.title
                    );
                }
                for field in &section.fields {
                    if !self.fields.contains_key(field) {
                        anyhow::bail!(
                            "context `{name}` section `{}` references undefined field `{field}`",
                            section.title
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn field_label(&self, field: &str) -> String {
        self.fields
            .get(field)
            .and_then(|f| f.label.clone())
            .unwrap_or_else(|| field.to_string())
    }
}

pub fn add_schema_outputs(
    plan: &mut OutputPlan,
    nodes: &[Node],
    graph: &LinkGraph,
    input_root: &Path,
    schema: &SchemaPack,
) -> Result<()> {
    let catalog = build_catalog(nodes, graph, input_root, schema, plan);
    insert_bytes(
        plan,
        ".md-wiki/catalog.json",
        serde_json::to_vec_pretty(&catalog)?,
    );
    render_field_catalogs(plan, schema, &catalog);
    Ok(())
}

pub fn build_catalog(
    nodes: &[Node],
    graph: &LinkGraph,
    input_root: &Path,
    schema: &SchemaPack,
    plan: &OutputPlan,
) -> InternalCatalog {
    let mut pages = Vec::new();
    let mut seen_paths = BTreeSet::new();
    for node in nodes {
        let source_text =
            fs::read_to_string(input_root.join(&node.note.source_file)).unwrap_or_default();
        let frontmatter_yaml = frontmatter_yaml_value(&source_text);
        let fields = extract_fields(schema, node, &source_text, frontmatter_yaml.as_ref());
        let entities = entities_for(node);
        let tags = node.note.frontmatter.tags.clone();
        let headings: Vec<String> = node.note.headings.iter().map(|h| h.text.clone()).collect();
        let line_end = source_text.lines().count().max(1);
        let page_ranges = page_source_ranges(node, &source_text);

        for page in iter_pages(node) {
            let generated_path = markdown_path(&page.output_path);
            let source_range = page_ranges
                .get(&page.output_path)
                .cloned()
                .unwrap_or(SourceRange {
                    line_start: 1,
                    line_end,
                });
            let page_fields = fields_for_page(&fields, &source_range);
            seen_paths.insert(generated_path.clone());
            pages.push(CatalogPage {
                generated_path,
                source_path: markdown_path(&node.note.source_file),
                source_range,
                title: page.title,
                doc_type: format!("{:?}", page.kind).to_lowercase(),
                entities: entities.clone(),
                tags: tags.clone(),
                headings: headings.clone(),
                fields: page_fields,
                outgoing_links: graph
                    .forward_of(&page.output_path)
                    .iter()
                    .map(|path| markdown_path(path))
                    .collect(),
                backlinks: graph
                    .backward_of(&page.output_path)
                    .iter()
                    .map(|path| markdown_path(path))
                    .collect(),
            });
        }
    }

    add_plan_metadata_pages(&mut pages, plan, &seen_paths);
    pages.sort_by(|a, b| a.generated_path.cmp(&b.generated_path));
    InternalCatalog {
        schema_version: 1,
        schema: CatalogSchema {
            id: schema.id.clone(),
            version: schema.version,
        },
        pages,
    }
}

fn page_source_ranges(node: &Node, source_text: &str) -> BTreeMap<PathBuf, SourceRange> {
    let source_line_end = source_text.lines().count().max(1);
    let body_line_offset = source_body_line_offset(source_text);
    let body_lines: Vec<&str> = node.note.body.split('\n').collect();
    let mut search_start = 0usize;
    let mut out = BTreeMap::new();

    for page in iter_pages(node) {
        let body_span = raw_body_line_span(&page.raw_body, &body_lines, &mut search_start);
        let range = match (page.kind, body_span) {
            (PageKind::Entry, Some((_, body_end))) => SourceRange {
                line_start: 1,
                line_end: (body_end + body_line_offset).max(1),
            },
            (PageKind::Entry, None) => SourceRange {
                line_start: 1,
                line_end: body_line_offset.max(1),
            },
            (_, Some((body_start, body_end))) => SourceRange {
                line_start: body_start + body_line_offset,
                line_end: body_end + body_line_offset,
            },
            (_, None) => SourceRange {
                line_start: 1,
                line_end: source_line_end,
            },
        };
        out.insert(page.output_path, clamp_source_range(range, source_line_end));
    }

    out
}

fn raw_body_line_span(
    raw_body: &str,
    body_lines: &[&str],
    search_start: &mut usize,
) -> Option<(usize, usize)> {
    if raw_body.is_empty() {
        return None;
    }
    let raw_lines: Vec<&str> = raw_body.split('\n').collect();
    if raw_lines.is_empty() || raw_lines.len() > body_lines.len() {
        return None;
    }
    if let Some(span) = find_line_span(&raw_lines, body_lines, *search_start) {
        *search_start = span.1;
        return Some((span.0 + 1, span.1));
    }
    find_line_span(&raw_lines, body_lines, 0).map(|span| {
        *search_start = span.1;
        (span.0 + 1, span.1)
    })
}

fn find_line_span(needle: &[&str], haystack: &[&str], start_at: usize) -> Option<(usize, usize)> {
    if needle.len() > haystack.len() {
        return None;
    }
    let max_start = haystack.len() - needle.len();
    for start in start_at.min(max_start)..=max_start {
        if haystack[start..start + needle.len()] == *needle {
            return Some((start, start + needle.len()));
        }
    }
    None
}

fn clamp_source_range(mut range: SourceRange, line_end: usize) -> SourceRange {
    range.line_start = range.line_start.clamp(1, line_end);
    range.line_end = range.line_end.clamp(range.line_start, line_end);
    range
}

fn fields_for_page(
    fields: &BTreeMap<String, Vec<FieldEvidence>>,
    source_range: &SourceRange,
) -> BTreeMap<String, Vec<FieldEvidence>> {
    fields
        .iter()
        .filter_map(|(name, items)| {
            let page_items: Vec<_> = items
                .iter()
                .filter(|item| {
                    item.line_start >= source_range.line_start
                        && item.line_start <= source_range.line_end
                })
                .cloned()
                .collect();
            (!page_items.is_empty()).then(|| (name.clone(), page_items))
        })
        .collect()
}

fn add_plan_metadata_pages(
    pages: &mut Vec<CatalogPage>,
    plan: &OutputPlan,
    seen_paths: &BTreeSet<String>,
) {
    for (rel, body) in plan {
        if rel.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let rel_path = markdown_path(rel);
        if seen_paths.contains(&rel_path) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(body) else {
            continue;
        };
        let Some(meta) = md_wiki_metadata(text) else {
            continue;
        };
        let Some(source_path) = value_at_path(&meta, "source").and_then(|value| value.as_str())
        else {
            continue;
        };
        let generated_path = value_at_path(&meta, "output_path")
            .and_then(|value| value.as_str())
            .unwrap_or(&rel_path);
        if !generated_path.starts_with("fragments/") || seen_paths.contains(generated_path) {
            continue;
        }
        let title = value_at_path(&meta, "title")
            .and_then(|value| value.as_str())
            .unwrap_or(generated_path)
            .to_string();
        let tags = string_sequence_at(&meta, "tags");
        let body_without_frontmatter = strip_leading_frontmatter(text);
        let headings: Vec<String> = headings::extract(body_without_frontmatter)
            .into_iter()
            .map(|heading| heading.text)
            .collect();
        let source_range = first_line_range(&meta).unwrap_or(SourceRange {
            line_start: 1,
            line_end: 1,
        });
        let entities = metadata_page_entities(&title, source_path, &tags, &headings);

        pages.push(CatalogPage {
            generated_path: generated_path.to_string(),
            source_path: source_path.to_string(),
            source_range,
            title,
            doc_type: value_at_path(&meta, "page_kind")
                .and_then(|value| value.as_str())
                .unwrap_or("page")
                .to_string(),
            entities,
            tags,
            headings,
            fields: BTreeMap::new(),
            outgoing_links: string_sequence_at(&meta, "outgoing_links"),
            backlinks: Vec::new(),
        });
    }
}

fn md_wiki_metadata(text: &str) -> Option<serde_yaml::Value> {
    frontmatter_yaml_value(text).and_then(|value| value_at_path(&value, "md_wiki").cloned())
}

fn strip_leading_frontmatter(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("---\n") else {
        return text;
    };
    let mut offset = 0usize;
    for line in rest.lines() {
        if line.trim() == "---" {
            return &rest[offset + line.len() + 1..];
        }
        offset += line.len() + 1;
    }
    text
}

fn string_sequence_at(value: &serde_yaml::Value, path: &str) -> Vec<String> {
    value_at_path(value, path)
        .and_then(|value| value.as_sequence())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
        .collect()
}

fn first_line_range(meta: &serde_yaml::Value) -> Option<SourceRange> {
    let first = value_at_path(meta, "line_ranges")?.as_sequence()?.first()?;
    let items = first.as_sequence()?;
    let line_start = items.first()?.as_u64()? as usize;
    let line_end = items.get(1)?.as_u64()? as usize;
    Some(SourceRange {
        line_start,
        line_end,
    })
}

fn metadata_page_entities(
    title: &str,
    source_path: &str,
    tags: &[String],
    headings: &[String],
) -> Vec<String> {
    let mut entities = BTreeSet::new();
    entities.insert(title.to_string());
    entities.insert(source_path.to_string());
    if let Some(stem) = Path::new(source_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
    {
        entities.insert(stem.to_string());
    }
    for tag in tags {
        entities.insert(tag.clone());
    }
    for heading in headings {
        entities.insert(heading.clone());
    }
    entities.into_iter().collect()
}

fn entities_for(node: &Node) -> Vec<String> {
    let mut entities = BTreeSet::new();
    entities.insert(node.title.clone());
    if let Some(title) = &node.note.frontmatter.title {
        entities.insert(title.clone());
    }
    for alias in &node.note.frontmatter.aliases {
        entities.insert(alias.clone());
    }
    for tag in &node.note.frontmatter.tags {
        entities.insert(tag.clone());
    }
    entities.into_iter().collect()
}

fn frontmatter_yaml_value(source: &str) -> Option<serde_yaml::Value> {
    let rest = source.strip_prefix("---\n")?;
    let end = rest
        .lines()
        .scan(0usize, |offset, line| {
            let start = *offset;
            *offset += line.len() + 1;
            Some((start, line))
        })
        .find_map(|(start, line)| (line.trim() == "---").then_some(start))?;
    serde_yaml::from_str(&rest[..end]).ok()
}

fn extract_fields(
    schema: &SchemaPack,
    node: &Node,
    source_text: &str,
    frontmatter: Option<&serde_yaml::Value>,
) -> BTreeMap<String, Vec<FieldEvidence>> {
    let mut out = BTreeMap::new();
    let body_line_offset = source_body_line_offset(source_text);
    for (field_name, field) in &schema.fields {
        let mut items = Vec::new();
        for source in &field.sources {
            if let Some(path) = &source.frontmatter
                && let Some(value) = frontmatter.and_then(|fm| value_at_path(fm, path))
                && let Some(text) = value_to_text(value)
            {
                items.push(FieldEvidence {
                    text,
                    source_path: markdown_path(&node.note.source_file),
                    line_start: 1,
                    line_end: frontmatter_line_end(source_text),
                });
            }
            if let Some(heading) = &source.heading
                && let Some((text, line_start, line_end)) =
                    section_under_heading(&node.note.body, heading)
            {
                items.push(FieldEvidence {
                    text,
                    source_path: markdown_path(&node.note.source_file),
                    line_start: line_start + body_line_offset,
                    line_end: line_end + body_line_offset,
                });
            }
        }
        items.sort_by(|left, right| {
            left.line_start
                .cmp(&right.line_start)
                .then_with(|| left.line_end.cmp(&right.line_end))
                .then_with(|| left.source_path.cmp(&right.source_path))
                .then_with(|| left.text.cmp(&right.text))
        });
        if !items.is_empty() {
            out.insert(field_name.clone(), items);
        }
    }
    out
}

fn value_at_path<'a>(value: &'a serde_yaml::Value, path: &str) -> Option<&'a serde_yaml::Value> {
    let mut current = value;
    for segment in path.split('.') {
        let key = serde_yaml::Value::String(segment.to_string());
        current = current.as_mapping()?.get(&key)?;
    }
    Some(current)
}

fn value_to_text(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::Null => None,
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        serde_yaml::Value::Sequence(items) => {
            let text = items
                .iter()
                .filter_map(value_to_text)
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty()).then_some(text)
        }
        serde_yaml::Value::Mapping(_) | serde_yaml::Value::Tagged(_) => {
            serde_yaml::to_string(value)
                .ok()
                .map(|s| s.trim().to_string())
        }
    }
}

fn frontmatter_line_end(source: &str) -> usize {
    if !source.starts_with("---\n") {
        return 1;
    }
    for (idx, line) in source.lines().enumerate().skip(1) {
        if line.trim() == "---" {
            return idx + 1;
        }
    }
    1
}

fn source_body_line_offset(source: &str) -> usize {
    if source.starts_with("---\n") {
        frontmatter_line_end(source)
    } else {
        0
    }
}

fn section_under_heading(body: &str, target: &str) -> Option<(String, usize, usize)> {
    let lines: Vec<&str> = body.lines().collect();
    let mut found: Option<(usize, usize)> = None;
    for (idx, line) in lines.iter().enumerate() {
        let Some((level, text)) = parse_heading(line) else {
            continue;
        };
        if found.is_none() && text == target {
            found = Some((idx, level));
            continue;
        }
        if let Some((start, start_level)) = found
            && idx > start
            && level <= start_level
        {
            return heading_body(&lines, start, idx);
        }
    }
    found.and_then(|(start, _)| heading_body(&lines, start, lines.len()))
}

fn heading_body(
    lines: &[&str],
    heading_idx: usize,
    end_idx: usize,
) -> Option<(String, usize, usize)> {
    let mut start = heading_idx + 1;
    while start < end_idx && lines[start].trim().is_empty() {
        start += 1;
    }
    let mut end = end_idx;
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let text = lines[start..end].join("\n").trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some((text, start + 1, end))
    }
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.starts_with(' ') {
        return None;
    }
    Some((level, rest.trim().trim_end_matches('#').trim()))
}

fn render_field_catalogs(plan: &mut OutputPlan, schema: &SchemaPack, catalog: &InternalCatalog) {
    let mut index = String::from("# Fields\n\n");
    for field in schema.fields.keys() {
        let label = schema.field_label(field);
        index.push_str(&format!("- [{}]({}.md)\n", label, field));
        let mut body = format!("# {}\n\n", label);
        for page in &catalog.pages {
            if let Some(items) = page.fields.get(field) {
                for item in items {
                    body.push_str(&format!(
                        "- `{}` lines {}-{}: {}\n",
                        page.generated_path,
                        item.line_start,
                        item.line_end,
                        compact_line(&item.text)
                    ));
                }
            }
        }
        insert_text(
            plan,
            PathBuf::from("agent/fields").join(format!("{field}.md")),
            body,
        );
    }
    insert_text(plan, "agent/fields/index.md", index);
}

pub fn render_context_pack(
    wiki: &Path,
    schema_path: &Path,
    task: &str,
    entities: &[String],
    query: Option<&str>,
    time: Option<&str>,
    budget: Option<usize>,
) -> Result<String> {
    let schema = load_schema(schema_path)?;
    let recipe = schema
        .contexts
        .get(task)
        .with_context(|| format!("schema context `{task}` is not defined"))?;
    let catalog_path = wiki.join(".md-wiki/catalog.json");
    let catalog_body = fs::read_to_string(&catalog_path)
        .with_context(|| format!("failed to read {}", catalog_path.display()))?;
    let catalog: InternalCatalog = serde_json::from_str(&catalog_body)
        .with_context(|| format!("failed to parse {}", catalog_path.display()))?;
    if catalog.schema.id != schema.id || catalog.schema.version != schema.version {
        anyhow::bail!("catalog schema does not match schema pack; rerun init or add with --schema");
    }

    let selection = select_pages(&catalog, recipe, entities, query, time);
    let limit = budget.or(recipe.default_budget_chars).unwrap_or(20_000);
    Ok(pack_with_budget(ContextPackInput {
        schema: &schema,
        recipe,
        task,
        pages: &selection.pages,
        required_scope_paths: &selection.required_scope_paths,
        entities,
        query,
        time,
        budget: limit,
    }))
}

struct PageSelection<'a> {
    pages: Vec<&'a CatalogPage>,
    required_scope_paths: BTreeSet<String>,
}

fn select_pages<'a>(
    catalog: &'a InternalCatalog,
    recipe: &ContextRecipe,
    entities: &[String],
    query: Option<&str>,
    time: Option<&str>,
) -> PageSelection<'a> {
    let recipe_fields: BTreeSet<&str> = recipe
        .sections
        .iter()
        .flat_map(|section| section.fields.iter().map(String::as_str))
        .collect();
    let query = query.map(str::to_lowercase);
    let time = time.map(str::to_lowercase);
    let by_path: BTreeMap<&str, &CatalogPage> = catalog
        .pages
        .iter()
        .map(|page| (page.generated_path.as_str(), page))
        .collect();
    let mut candidates: BTreeMap<String, (u8, &CatalogPage)> = BTreeMap::new();
    let mut target_paths = BTreeSet::new();
    let has_target = !entities.is_empty() || query.is_some() || time.is_some();

    for page in &catalog.pages {
        if entity_matches(page, entities) {
            add_candidate(&mut candidates, 0, page);
            target_paths.insert(page.generated_path.clone());
            for neighbor in page.outgoing_links.iter().chain(page.backlinks.iter()) {
                if let Some(linked_page) = by_path.get(neighbor.as_str()) {
                    add_candidate(&mut candidates, 1, linked_page);
                    target_paths.insert(linked_page.generated_path.clone());
                }
            }
        }
    }

    for page in &catalog.pages {
        if page
            .fields
            .keys()
            .any(|field| recipe_fields.contains(field.as_str()))
        {
            add_candidate(&mut candidates, 2, page);
        }
    }

    for page in &catalog.pages {
        if query.as_ref().is_some_and(|q| page_matches(page, q)) {
            add_candidate(&mut candidates, 3, page);
            target_paths.insert(page.generated_path.clone());
        }
        if time.as_ref().is_some_and(|q| page_matches(page, q)) {
            add_candidate(&mut candidates, 4, page);
            target_paths.insert(page.generated_path.clone());
        }
    }

    let mut pages: Vec<_> = candidates.into_values().collect();
    pages.sort_by(|(left_priority, left), (right_priority, right)| {
        left_priority
            .cmp(right_priority)
            .then_with(|| left.generated_path.cmp(&right.generated_path))
    });
    let pages: Vec<_> = pages.into_iter().map(|(_, page)| page).collect();
    let required_scope_paths = if has_target {
        target_paths
    } else {
        pages
            .iter()
            .map(|page| page.generated_path.clone())
            .collect::<BTreeSet<_>>()
    };

    PageSelection {
        pages,
        required_scope_paths,
    }
}

fn entity_matches(page: &CatalogPage, entities: &[String]) -> bool {
    !entities.is_empty()
        && entities.iter().any(|entity| {
            page.entities.iter().any(|candidate| candidate == entity)
                || page.tags.iter().any(|tag| tag == entity)
        })
}

fn add_candidate<'a>(
    candidates: &mut BTreeMap<String, (u8, &'a CatalogPage)>,
    priority: u8,
    page: &'a CatalogPage,
) {
    candidates
        .entry(page.generated_path.clone())
        .and_modify(|existing| {
            if priority < existing.0 {
                *existing = (priority, page);
            }
        })
        .or_insert((priority, page));
}

fn page_matches(page: &CatalogPage, needle: &str) -> bool {
    let haystacks = std::iter::once(&page.title)
        .chain(page.entities.iter())
        .chain(page.tags.iter())
        .chain(page.headings.iter());
    if haystacks
        .into_iter()
        .any(|text| text.to_lowercase().contains(needle))
    {
        return true;
    }
    page.fields
        .values()
        .flatten()
        .any(|item| item.text.to_lowercase().contains(needle))
}

struct ContextPackInput<'a> {
    schema: &'a SchemaPack,
    recipe: &'a ContextRecipe,
    task: &'a str,
    pages: &'a [&'a CatalogPage],
    required_scope_paths: &'a BTreeSet<String>,
    entities: &'a [String],
    query: Option<&'a str>,
    time: Option<&'a str>,
    budget: usize,
}

fn pack_with_budget(input: ContextPackInput<'_>) -> String {
    let title = input.recipe.title.as_deref().unwrap_or(input.task);
    let mut pack = context_header(
        input.schema,
        input.task,
        title,
        input.entities,
        input.query,
        input.time,
        input.budget,
    );
    let mut source_trail = Vec::new();
    let mut cited_pages = BTreeSet::new();
    let mut missing = Vec::new();

    let required_section_titles: BTreeSet<String> = input
        .recipe
        .sections
        .iter()
        .filter(|section| section.required)
        .map(|section| section.title.clone())
        .collect();

    for section in input
        .recipe
        .sections
        .iter()
        .filter(|s| s.kind.as_deref() != Some("sources"))
    {
        let mut section_body = String::new();
        for field in &section.fields {
            let mut found = false;
            for page in input.pages {
                if let Some(items) = page.fields.get(field) {
                    if !section.required
                        || input.required_scope_paths.contains(&page.generated_path)
                    {
                        found = true;
                    }
                    for item in items {
                        source_trail.push(format!(
                            "- `{}` from `{}` lines {}-{} ({})",
                            page.generated_path,
                            item.source_path,
                            item.line_start,
                            item.line_end,
                            field
                        ));
                        cited_pages.insert(page.generated_path.clone());
                        section_body.push_str(&format!(
                            "- `{}` lines {}-{}: {}\n",
                            page.generated_path,
                            item.line_start,
                            item.line_end,
                            compact_line(&item.text)
                        ));
                    }
                }
            }
            if section.required && !found {
                missing.push(format!("- `{field}` for section `{}`", section.title));
            }
        }
        if !section_body.is_empty() {
            pack.push_str(&format!("\n## {}\n\n{}", section.title, section_body));
        }
    }

    for page in input.pages {
        if cited_pages.contains(&page.generated_path) {
            continue;
        }
        source_trail.push(format!(
            "- `{}` from `{}` lines {}-{}",
            page.generated_path,
            page.source_path,
            page.source_range.line_start,
            page.source_range.line_end
        ));
    }

    if !missing.is_empty() {
        pack.push_str("\n## Missing Required Evidence\n\n");
        for item in &missing {
            pack.push_str(item);
            pack.push('\n');
        }
    }
    pack.push_str("\n## Source Trail\n\n");
    if source_trail.is_empty() {
        pack.push_str("- No sources matched.\n");
    } else {
        source_trail.sort();
        source_trail.dedup();
        for item in &source_trail {
            pack.push_str(item);
            pack.push('\n');
        }
    }

    enforce_budget(pack, input.budget, &required_section_titles)
}

fn context_header(
    schema: &SchemaPack,
    task: &str,
    title: &str,
    entities: &[String],
    query: Option<&str>,
    time: Option<&str>,
    budget: usize,
) -> String {
    let doc = ContextFrontmatter {
        md_wiki_context: ContextFrontmatterData {
            schema_id: &schema.id,
            schema_version: schema.version,
            task,
            budget_chars: budget,
            entities,
            query,
            time,
        },
    };
    let yaml = serde_yaml::to_string(&doc).expect("context frontmatter should serialize");
    let mut out = String::from("---\n");
    out.push_str(&yaml);
    out.push_str("---\n");
    out.push_str(&format!("# {}\n", title));
    out
}

#[derive(Serialize)]
struct ContextFrontmatter<'a> {
    md_wiki_context: ContextFrontmatterData<'a>,
}

#[derive(Serialize)]
struct ContextFrontmatterData<'a> {
    schema_id: &'a str,
    schema_version: u32,
    task: &'a str,
    budget_chars: usize,
    #[serde(skip_serializing_if = "slice_is_empty")]
    entities: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time: Option<&'a str>,
}

fn slice_is_empty<T>(items: &[T]) -> bool {
    items.is_empty()
}

fn enforce_budget(
    pack: String,
    budget: usize,
    required_section_titles: &BTreeSet<String>,
) -> String {
    if pack.chars().count() <= budget {
        return pack;
    }
    let source_heading = "\n## Source Trail\n\n";
    let source_start = pack.find(source_heading).unwrap_or(pack.len());
    let prefix_end = pack[..source_start].find("\n## ").unwrap_or(source_start);
    let mandatory_prefix = &pack[..prefix_end];
    let pre_source_body = &pack[prefix_end..source_start];
    let pre_source_sections = markdown_sections(pre_source_body);
    let source_body = pack
        .find(source_heading)
        .map(|idx| &pack[idx + source_heading.len()..])
        .unwrap_or("");
    let omitted = "\n\nOmitted due to budget.\n";
    let trail_omitted = "- Omitted due to budget.\n";
    let mut out = String::new();
    out.push_str(mandatory_prefix);

    let reserved_len =
        omitted.chars().count() + source_heading.chars().count() + trail_omitted.chars().count();
    let mut omitted_any_section = false;
    for section in &pre_source_sections {
        let must_keep = section.title == "Missing Required Evidence"
            || required_section_titles.contains(section.title);
        if !must_keep {
            continue;
        }
        if push_section_with_budget(&mut out, section.body, reserved_len, budget) {
            continue;
        }
        if !push_pruned_section_with_budget(&mut out, section.body, reserved_len, budget) {
            omitted_any_section = true;
        }
    }

    for section in &pre_source_sections {
        let already_handled = section.title == "Missing Required Evidence"
            || required_section_titles.contains(section.title);
        if already_handled {
            continue;
        }
        if push_section_with_budget(&mut out, section.body, reserved_len, budget) {
            continue;
        }
        if !push_pruned_section_with_budget(&mut out, section.body, reserved_len, budget) {
            omitted_any_section = true;
        }
    }

    if omitted_any_section
        && out.chars().count()
            + omitted.chars().count()
            + source_heading.chars().count()
            + trail_omitted.chars().count()
            <= budget
    {
        out.push_str(omitted);
    }
    out.push_str(source_heading);

    let base_len = out.chars().count();
    let trail_omitted_len = trail_omitted.chars().count();
    if base_len + trail_omitted_len > budget {
        return compact_budget_floor(mandatory_prefix, budget);
    }

    let source_line_count = source_body.lines().count();
    let mut used = base_len;
    let mut written_source_lines = 0usize;
    let mut wrote_source = false;
    for line in source_body.lines() {
        let line_len = line.chars().count() + 1;
        if used + line_len + trail_omitted_len > budget {
            break;
        }
        out.push_str(line);
        out.push('\n');
        used += line_len;
        written_source_lines += 1;
        wrote_source = true;
    }
    if !wrote_source || written_source_lines < source_line_count {
        out.push_str(trail_omitted);
    }
    out
}

struct MarkdownSection<'a> {
    title: &'a str,
    body: &'a str,
}

fn markdown_sections(body: &str) -> Vec<MarkdownSection<'_>> {
    let mut sections = Vec::new();
    let mut starts = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative) = body[cursor..].find("\n## ") {
        let start = cursor + relative;
        starts.push(start);
        cursor = start + 1;
    }
    for (idx, start) in starts.iter().enumerate() {
        let end = starts.get(idx + 1).copied().unwrap_or(body.len());
        let section = &body[*start..end];
        let title = section
            .lines()
            .find(|line| line.starts_with("## "))
            .and_then(|line| line.strip_prefix("## "))
            .unwrap_or("")
            .trim();
        sections.push(MarkdownSection {
            title,
            body: section,
        });
    }
    sections
}

fn push_section_with_budget(
    out: &mut String,
    section: &str,
    reserved_len: usize,
    budget: usize,
) -> bool {
    if out.chars().count() + section.chars().count() + reserved_len <= budget {
        out.push_str(section);
        true
    } else {
        false
    }
}

fn push_pruned_section_with_budget(
    out: &mut String,
    section: &str,
    reserved_len: usize,
    budget: usize,
) -> bool {
    let omitted_note = "- Omitted due to budget; see Source Trail.\n";
    let mut candidate = String::new();
    let mut omitted_evidence = false;
    let mut saw_evidence = false;

    for line in section.lines() {
        let line_with_newline = format!("{line}\n");
        if !line.starts_with("- ") {
            candidate.push_str(&line_with_newline);
            continue;
        }
        saw_evidence = true;
        if out.chars().count()
            + candidate.chars().count()
            + line_with_newline.chars().count()
            + omitted_note.chars().count()
            + reserved_len
            <= budget
        {
            candidate.push_str(&line_with_newline);
        } else {
            omitted_evidence = true;
        }
    }

    if !saw_evidence {
        return false;
    }
    if omitted_evidence {
        candidate.push_str(omitted_note);
    }
    push_section_with_budget(out, &candidate, reserved_len, budget)
}

fn compact_budget_floor(mandatory_prefix: &str, budget: usize) -> String {
    let title_line = mandatory_prefix
        .lines()
        .find(|line| line.starts_with("# "))
        .unwrap_or("# Context");
    let compact_prefix = format!("---\nmd_wiki_context:\n  budget_chars: {budget}\n---\n");
    let with_title = format!("{compact_prefix}{title_line}\n\n## Source Trail\n");
    if with_title.chars().count() <= budget {
        return with_title;
    }
    let without_title = format!("{compact_prefix}## Source Trail\n");
    if without_title.chars().count() <= budget {
        return without_title;
    }
    let bare = "---\nmd_wiki_context: {}\n---\n## Source Trail\n";
    if bare.chars().count() <= budget {
        return bare.to_string();
    }
    truncate_chars(bare, budget)
}

fn truncate_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn compact_line(text: &str) -> String {
    let mut out = text.split_whitespace().collect::<Vec<_>>().join(" ");
    const LIMIT: usize = 220;
    if out.chars().count() > LIMIT {
        out = out.chars().take(LIMIT - 3).collect::<String>();
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_undefined_context_field() {
        let schema = SchemaPack {
            id: "s".into(),
            version: 1,
            fields: BTreeMap::from([(
                "known".into(),
                FieldDef {
                    label: None,
                    sources: vec![FieldSource {
                        frontmatter: Some("known".into()),
                        heading: None,
                    }],
                },
            )]),
            contexts: BTreeMap::from([(
                "task".into(),
                ContextRecipe {
                    title: None,
                    default_budget_chars: None,
                    sections: vec![ContextSection {
                        title: "Missing".into(),
                        fields: vec!["unknown".into()],
                        kind: None,
                        required: false,
                    }],
                },
            )]),
        };

        let err = schema.validate().unwrap_err().to_string();
        assert!(err.contains("undefined field"));
    }

    #[test]
    fn extracts_nested_frontmatter_value() {
        let value: serde_yaml::Value = serde_yaml::from_str("narrative:\n  setup: key\n").unwrap();
        assert_eq!(
            value_at_path(&value, "narrative.setup").and_then(value_to_text),
            Some("key".into())
        );
    }

    #[test]
    fn extracts_heading_section_body() {
        let body = "# A\n\n## Setup\n\nalpha\n\n### Detail\n\nbeta\n\n## Next\n\nno";
        let (text, line_start, line_end) = section_under_heading(body, "Setup").unwrap();
        assert!(text.contains("alpha"));
        assert!(text.contains("### Detail"));
        assert_eq!(line_start, 5);
        assert_eq!(line_end, 9);
    }
}
