//! Shared helpers and per-command modules for the `ought` CLI.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use ought_cli::config::Config;
use ought_gen::manifest::Manifest;
use ought_run::runners;
use ought_spec::SpecGraph;

pub mod auth;
pub mod bisect;
pub mod blame;
pub mod check;
pub mod diff;
pub mod extract;
pub mod generate;
pub mod init;
pub mod inspect;
pub mod mcp;
pub mod run;
pub mod survey;
pub mod view;
pub mod watch;

// ─── Config + spec loading helpers ──────────────────────────────────────────

/// Load config, resolving from --config flag or auto-discovery.
pub fn load_config(config_path: &Option<PathBuf>) -> anyhow::Result<(PathBuf, Config)> {
    match config_path {
        Some(path) => {
            let config = Config::load(path)?;
            Ok((path.clone(), config))
        }
        None => Config::discover(),
    }
}

/// Resolve each configured spec root against the project root (the directory
/// containing `ought.toml`).
pub fn resolve_spec_roots(config: &Config, config_path: &Path) -> Vec<PathBuf> {
    let config_dir = config_path.parent().unwrap_or(Path::new("."));
    config
        .specs
        .roots
        .iter()
        .map(|r| config_dir.join(r))
        .collect()
}

/// Load and parse all specs from config roots.
pub fn load_specs(config: &Config, config_path: &Path) -> anyhow::Result<SpecGraph> {
    let roots = resolve_spec_roots(config, config_path);
    SpecGraph::from_roots(&roots).map_err(|errors| {
        let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        anyhow::anyhow!("spec parse errors:\n  {}", messages.join("\n  "))
    })
}

/// The first configured runner's name, defaulting to "rust".
pub fn primary_runner_name(config: &Config) -> String {
    config
        .runner
        .keys()
        .next()
        .cloned()
        .unwrap_or("rust".into())
}

/// The first configured runner's `test_dir`, resolved against the config
/// directory. Falls back to `ought/ought-gen` when neither user config nor
/// preset supplies one.
pub fn primary_test_dir(config: &Config, config_path: &Path) -> PathBuf {
    let config_dir = config_path.parent().unwrap_or(Path::new("."));
    config
        .runner
        .values()
        .next()
        .and_then(|r| r.test_dir.as_ref())
        .map(|td| config_dir.join(td))
        .unwrap_or_else(|| config_dir.join("ought/ought-gen"))
}

/// Resolve the primary runner for commands that execute tests.
///
/// Returns `(runner_name, runner_box, resolved_config, abs_test_dir)`.
pub fn resolve_primary_runner(
    config: &Config,
    config_path: &Path,
) -> anyhow::Result<(
    String,
    Box<dyn ought_run::Runner>,
    ought_run::ResolvedRunnerConfig,
    PathBuf,
)> {
    let runner_name = primary_runner_name(config);
    let runner_cfg = config.runner.get(&runner_name).cloned().unwrap_or_default();
    let resolved = runner_cfg.resolve(&runner_name)?;
    let config_dir = config_path.parent().unwrap_or(Path::new("."));
    let test_dir = config_dir.join(&resolved.test_dir);
    let runner = runners::from_config(&runner_name, &runner_cfg, config_dir)?;
    Ok((runner_name, runner, resolved, test_dir))
}

// ─── Spec traversal helpers ─────────────────────────────────────────────────

/// A section group ready for agent-based generation.
pub struct SectionGroup<'a> {
    pub spec_path: PathBuf,
    pub section_path: String,
    pub testable_clauses: Vec<&'a ought_spec::Clause>,
    pub conditions: Vec<String>,
    pub source_paths: Vec<String>,
}

/// Collect section groups from all specs. Each section becomes one batch.
/// GIVEN clauses become conditions (context), not testable clauses.
/// OTHERWISE clauses are included with their parent.
pub fn collect_section_groups(specs: &SpecGraph) -> Vec<SectionGroup<'_>> {
    let mut groups = Vec::new();
    for spec in specs.specs() {
        let source_paths = spec.metadata.sources.clone();
        collect_groups_from_sections(
            &spec.sections,
            &spec.name,
            &spec.source_path,
            &source_paths,
            &mut groups,
        );
    }
    groups
}

fn collect_groups_from_sections<'a>(
    sections: &'a [ought_spec::Section],
    parent_path: &str,
    spec_path: &Path,
    source_paths: &[String],
    groups: &mut Vec<SectionGroup<'a>>,
) {
    for section in sections {
        let section_path = format!("{} > {}", parent_path, section.title);
        let mut testable = Vec::new();
        let mut conditions = Vec::new();

        for clause in &section.clauses {
            match clause.keyword {
                ought_spec::Keyword::Given => {
                    if let Some(ref cond) = clause.condition {
                        conditions.push(cond.clone());
                    } else {
                        conditions.push(clause.text.clone());
                    }
                }
                _ => {
                    testable.push(clause);
                }
            }
        }

        if !testable.is_empty() {
            groups.push(SectionGroup {
                spec_path: spec_path.to_path_buf(),
                section_path: section_path.clone(),
                testable_clauses: testable,
                conditions,
                source_paths: source_paths.to_vec(),
            });
        }

        collect_groups_from_sections(
            &section.subsections,
            &section_path,
            spec_path,
            source_paths,
            groups,
        );
    }
}

/// Keep only groups from the requested spec path or glob-like pattern.
pub fn filter_section_groups_by_path<'a>(
    groups: Vec<SectionGroup<'a>>,
    config_path: &Path,
    pattern: &str,
) -> anyhow::Result<Vec<SectionGroup<'a>>> {
    let config_dir = config_path.parent().unwrap_or(Path::new("."));
    let filtered: Vec<SectionGroup<'a>> = groups
        .into_iter()
        .filter(|group| spec_path_matches(&group.spec_path, config_dir, pattern))
        .collect();

    if filtered.is_empty() {
        anyhow::bail!("no spec files matched '{}'", pattern);
    }

    Ok(filtered)
}

fn spec_path_matches(spec_path: &Path, config_dir: &Path, pattern: &str) -> bool {
    let config_dir = normalize_path(config_dir);
    let spec_path = normalize_path(spec_path);
    let raw_pattern = Path::new(pattern);
    let rooted_pattern = if raw_pattern.is_absolute() {
        raw_pattern.to_path_buf()
    } else {
        config_dir.join(raw_pattern)
    };
    let rooted_pattern = normalize_path(&rooted_pattern);

    let relative_spec = spec_path
        .strip_prefix(&config_dir)
        .map(normalize_path)
        .unwrap_or_else(|_| spec_path.clone());
    let raw_pattern = normalize_path(raw_pattern);

    if !pattern.contains('*') {
        return spec_path == rooted_pattern
            || spec_path.starts_with(&rooted_pattern)
            || relative_spec == raw_pattern
            || relative_spec.starts_with(&raw_pattern);
    }

    let spec_abs = path_to_slash(&spec_path);
    let spec_rel = path_to_slash(&relative_spec);
    let pattern_abs = path_to_slash(&rooted_pattern);
    let pattern_rel = path_to_slash(&raw_pattern);

    wildcard_match(&pattern_abs, &spec_abs) || wildcard_match(&pattern_rel, &spec_rel)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn path_to_slash(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');
    let mut parts = pattern.split('*').filter(|part| !part.is_empty());
    let Some(first) = parts.next() else {
        return true;
    };

    let mut rest = text;
    let mut last = first;
    if anchored_start {
        let Some(stripped) = rest.strip_prefix(first) else {
            return false;
        };
        rest = stripped;
    } else if let Some(index) = rest.find(first) {
        rest = &rest[index + first.len()..];
    } else {
        return false;
    }

    for part in parts {
        let Some(index) = rest.find(part) else {
            return false;
        };
        rest = &rest[index + part.len()..];
        last = part;
    }

    !anchored_end || text.ends_with(last)
}

/// Collect all testable clause IDs for manifest cleanup.
pub fn collect_all_testable_ids(specs: &SpecGraph) -> Vec<ought_spec::ClauseId> {
    let mut ids = Vec::new();
    for spec in specs.specs() {
        collect_ids_from_sections(&spec.sections, &mut ids);
    }
    ids
}

fn collect_ids_from_sections(
    sections: &[ought_spec::Section],
    ids: &mut Vec<ought_spec::ClauseId>,
) {
    for section in sections {
        for clause in &section.clauses {
            if clause.keyword != ought_spec::Keyword::Given {
                ids.push(clause.id.clone());
            }
        }
        collect_ids_from_sections(&section.subsections, ids);
    }
}

/// Info about a clause for exit-code decisions.
pub struct ClauseInfo {
    pub severity: ought_spec::Severity,
    pub otherwise_ids: Vec<String>,
}

/// Build a map from clause ID string to its info for exit-code decisions.
pub fn collect_all_clause_info(specs: &SpecGraph) -> HashMap<String, ClauseInfo> {
    let mut map = HashMap::new();
    for spec in specs.specs() {
        collect_clause_info_from_sections(&spec.sections, &mut map);
    }
    map
}

fn collect_clause_info_from_sections(
    sections: &[ought_spec::Section],
    map: &mut HashMap<String, ClauseInfo>,
) {
    for section in sections {
        for clause in &section.clauses {
            let otherwise_ids: Vec<String> =
                clause.otherwise.iter().map(|ow| ow.id.0.clone()).collect();
            map.insert(
                clause.id.0.clone(),
                ClauseInfo {
                    severity: clause.severity,
                    otherwise_ids,
                },
            );
            for ow in &clause.otherwise {
                map.insert(
                    ow.id.0.clone(),
                    ClauseInfo {
                        severity: ow.severity,
                        otherwise_ids: Vec::new(),
                    },
                );
            }
        }
        collect_clause_info_from_sections(&section.subsections, map);
    }
}

// ─── Agent assignment builder ───────────────────────────────────────────────

/// Convert SectionGroups into AgentAssignments, partitioning across N agents round-robin.
/// Only includes stale clauses (unless force is true).
pub fn build_agent_assignments(
    groups: &[SectionGroup<'_>],
    manifest: &Manifest,
    config: &Config,
    config_path: &Path,
    test_dir: &Path,
    parallelism: usize,
    force: bool,
) -> Vec<ought_gen::AgentAssignment> {
    let project_root = config_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_string_lossy()
        .to_string();

    let target_language = config
        .runner
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "rust".to_string());

    let mut assignment_groups: Vec<ought_gen::AssignmentGroup> = Vec::new();

    for group in groups {
        let stale_clauses: Vec<&ought_spec::Clause> = group
            .testable_clauses
            .iter()
            .filter(|c| !c.pending)
            .filter(|c| {
                if force {
                    true
                } else {
                    manifest.is_stale(&c.id, &c.content_hash, "")
                }
            })
            .copied()
            .collect();

        if stale_clauses.is_empty() {
            continue;
        }

        let clauses: Vec<ought_gen::AssignmentClause> = stale_clauses
            .iter()
            .map(|c| clause_to_assignment_clause(c))
            .collect();

        assignment_groups.push(ought_gen::AssignmentGroup {
            section_path: group.section_path.clone(),
            clauses,
            conditions: group.conditions.clone(),
        });
    }

    if assignment_groups.is_empty() {
        return vec![];
    }

    let n = parallelism.min(assignment_groups.len()).max(1);
    let mut buckets: Vec<Vec<ought_gen::AssignmentGroup>> = (0..n).map(|_| Vec::new()).collect();

    for (i, group) in assignment_groups.into_iter().enumerate() {
        buckets[i % n].push(group);
    }

    let all_source_paths: Vec<String> = groups
        .iter()
        .flat_map(|g| g.source_paths.iter().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    buckets
        .into_iter()
        .enumerate()
        .filter(|(_, groups)| !groups.is_empty())
        .map(|(i, groups)| ought_gen::AgentAssignment {
            id: format!("agent_{}", i),
            project_root: project_root.clone(),
            config_path: config_path.to_string_lossy().to_string(),
            test_dir: test_dir.to_string_lossy().to_string(),
            target_language: target_language.clone(),
            source_paths: all_source_paths.clone(),
            groups,
        })
        .collect()
}

/// Convert a spec Clause into an AssignmentClause (serializable).
pub fn clause_to_assignment_clause(clause: &ought_spec::Clause) -> ought_gen::AssignmentClause {
    let keyword = ought_gen::keyword_str(clause.keyword).to_string();
    let temporal = clause.temporal.as_ref().map(|t| match t {
        ought_spec::Temporal::Invariant => "MUST ALWAYS".to_string(),
        ought_spec::Temporal::Deadline(dur) => format!("MUST BY {:?}", dur),
    });

    let otherwise: Vec<ought_gen::AssignmentClause> = clause
        .otherwise
        .iter()
        .map(clause_to_assignment_clause)
        .collect();

    ought_gen::AssignmentClause {
        id: clause.id.0.clone(),
        keyword,
        text: clause.text.clone(),
        condition: clause.condition.clone(),
        temporal,
        content_hash: clause.content_hash.clone(),
        hints: clause.hints.clone(),
        otherwise,
    }
}

// ─── Clause lookup helpers ──────────────────────────────────────────────────

/// Find a clause by its ID string across all specs.
pub fn find_clause_by_id<'a>(
    specs: &'a SpecGraph,
    clause_id: &str,
) -> Option<&'a ought_spec::Clause> {
    for spec in specs.specs() {
        if let Some(c) = find_clause_in_sections(&spec.sections, clause_id) {
            return Some(c);
        }
    }
    None
}

/// Find a clause by partial ID match (clause ID contains the search string).
pub fn find_clause_by_partial_id<'a>(
    specs: &'a SpecGraph,
    partial_id: &str,
) -> Option<&'a ought_spec::Clause> {
    let search = partial_id.to_lowercase();
    for spec in specs.specs() {
        if let Some(c) = find_clause_partial_in_sections(&spec.sections, &search) {
            return Some(c);
        }
    }
    None
}

fn find_clause_partial_in_sections<'a>(
    sections: &'a [ought_spec::Section],
    search: &str,
) -> Option<&'a ought_spec::Clause> {
    let search_parts: Vec<&str> = search.split("::").collect();

    for section in sections {
        for clause in &section.clauses {
            if clause_id_matches(&clause.id.0, &search_parts) {
                return Some(clause);
            }
            for ow in &clause.otherwise {
                if clause_id_matches(&ow.id.0, &search_parts) {
                    return Some(ow);
                }
            }
        }
        if let Some(c) = find_clause_partial_in_sections(&section.subsections, search) {
            return Some(c);
        }
    }
    None
}

/// Check if a clause ID matches a search pattern.
/// Each segment of the search must fuzzy-match the corresponding segment of the ID.
fn clause_id_matches(clause_id: &str, search_parts: &[&str]) -> bool {
    let id_lower = clause_id.to_lowercase();
    let id_parts: Vec<&str> = id_lower.split("::").collect();

    if search_parts.len() > id_parts.len() {
        return false;
    }

    let offset = id_parts.len().saturating_sub(search_parts.len());
    for (i, search_part) in search_parts.iter().enumerate() {
        let id_part = id_parts.get(offset + i).unwrap_or(&"");
        let search_words: Vec<&str> = search_part.split('_').filter(|w| !w.is_empty()).collect();
        let matches = search_words.iter().all(|w| id_part.contains(w));
        if !matches {
            return false;
        }
    }
    true
}

fn find_clause_in_sections<'a>(
    sections: &'a [ought_spec::Section],
    clause_id: &str,
) -> Option<&'a ought_spec::Clause> {
    for section in sections {
        for clause in &section.clauses {
            if clause.id.0 == clause_id {
                return Some(clause);
            }
            for ow in &clause.otherwise {
                if ow.id.0 == clause_id {
                    return Some(ow);
                }
            }
        }
        if let Some(c) = find_clause_in_sections(&section.subsections, clause_id) {
            return Some(c);
        }
    }
    None
}

// ─── Test file collection ───────────────────────────────────────────────────

/// Collect GeneratedTest structs from files in the ought-gen directory.
pub fn collect_generated_tests(
    test_dir: &Path,
    extensions: &[String],
) -> anyhow::Result<Vec<ought_gen::GeneratedTest>> {
    let mut tests = Vec::new();

    if !test_dir.exists() {
        return Ok(tests);
    }

    fn walk(
        dir: &Path,
        root: &Path,
        extensions: &[String],
        tests: &mut Vec<ought_gen::GeneratedTest>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, root, extensions, tests);
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if !extensions.iter().any(|e| e == ext) {
                        continue;
                    }
                    let language = extension_to_language(ext);

                    let rel = path.strip_prefix(root).unwrap_or(&path);
                    let stem = rel.with_extension("").to_string_lossy().to_string();
                    let stem = stem.strip_suffix("_test").unwrap_or(&stem).to_string();
                    let clause_str = stem
                        .replace([std::path::MAIN_SEPARATOR, '/'], "::")
                        .replace("__", "::");
                    let clause_id = ought_spec::ClauseId(clause_str);

                    if let Ok(code) = std::fs::read_to_string(&path) {
                        tests.push(ought_gen::GeneratedTest {
                            clause_id,
                            code,
                            language,
                            file_path: path,
                        });
                    }
                }
            }
        }
    }

    walk(test_dir, test_dir, extensions, &mut tests);
    Ok(tests)
}

/// Map a known file extension to a Language.
pub fn extension_to_language(ext: &str) -> ought_gen::generator::Language {
    match ext {
        "rs" => ought_gen::generator::Language::Rust,
        "py" => ought_gen::generator::Language::Python,
        "ts" => ought_gen::generator::Language::TypeScript,
        "go" => ought_gen::generator::Language::Go,
        _ => ought_gen::generator::Language::JavaScript,
    }
}

// ─── Counting helpers ───────────────────────────────────────────────────────

pub fn count_clauses_in_sections(sections: &[ought_spec::Section]) -> usize {
    sections
        .iter()
        .map(|s| s.clauses.len() + count_clauses_in_sections(&s.subsections))
        .sum()
}

pub fn count_pending_in_sections(sections: &[ought_spec::Section]) -> usize {
    sections
        .iter()
        .map(|s| {
            s.clauses.iter().filter(|c| c.pending).count()
                + count_pending_in_sections(&s.subsections)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    use ought_spec::{
        Clause, ClauseId, Keyword, Metadata, Section, SourceLocation, Spec, SpecGraph,
    };

    fn spec(root: &Path, rel_path: &str, name: &str, clause_id: &str) -> Spec {
        let source_path = root.join(rel_path);
        Spec {
            name: name.to_string(),
            metadata: Metadata::default(),
            sections: vec![Section {
                title: "Behavior".to_string(),
                depth: 2,
                prose: String::new(),
                clauses: vec![Clause {
                    id: ClauseId(clause_id.to_string()),
                    keyword: Keyword::Must,
                    severity: Keyword::Must.severity(),
                    text: "do the thing".to_string(),
                    condition: None,
                    otherwise: Vec::new(),
                    temporal: None,
                    hints: Vec::new(),
                    source_location: SourceLocation {
                        file: source_path.clone(),
                        line: 3,
                    },
                    content_hash: clause_id.to_string(),
                    pending: false,
                }],
                subsections: Vec::new(),
            }],
            source_path,
        }
    }

    fn group_clause_ids(groups: &[SectionGroup<'_>]) -> Vec<String> {
        groups
            .iter()
            .flat_map(|group| group.testable_clauses.iter())
            .map(|clause| clause.id.0.clone())
            .collect()
    }

    #[test]
    fn filter_section_groups_by_relative_spec_path() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("ought.toml");
        let graph = SpecGraph::from_specs(vec![
            spec(
                tmp.path(),
                "ought/engine/generator.ought.md",
                "Generator",
                "generator::must_generate",
            ),
            spec(
                tmp.path(),
                "ought/engine/parser.ought.md",
                "Parser",
                "parser::must_parse",
            ),
        ])
        .unwrap();

        let groups = collect_section_groups(&graph);
        let filtered =
            filter_section_groups_by_path(groups, &config_path, "ought/engine/generator.ought.md")
                .unwrap();

        assert_eq!(
            group_clause_ids(&filtered),
            vec!["generator::must_generate"]
        );
    }

    #[test]
    fn filter_section_groups_by_glob_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("ought.toml");
        let graph = SpecGraph::from_specs(vec![
            spec(
                tmp.path(),
                "ought/engine/generator.ought.md",
                "Generator",
                "generator::must_generate",
            ),
            spec(
                tmp.path(),
                "ought/engine/parser.ought.md",
                "Parser",
                "parser::must_parse",
            ),
            spec(tmp.path(), "ought/cli/cli.ought.md", "CLI", "cli::must_run"),
        ])
        .unwrap();

        let groups = collect_section_groups(&graph);
        let filtered =
            filter_section_groups_by_path(groups, &config_path, "ought/engine/*.ought.md").unwrap();

        assert_eq!(
            group_clause_ids(&filtered),
            vec!["generator::must_generate", "parser::must_parse"]
        );
    }

    #[test]
    fn filter_section_groups_errors_when_no_specs_match() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("ought.toml");
        let graph = SpecGraph::from_specs(vec![spec(
            tmp.path(),
            "ought/engine/generator.ought.md",
            "Generator",
            "generator::must_generate",
        )])
        .unwrap();

        let groups = collect_section_groups(&graph);
        let result = filter_section_groups_by_path(groups, &config_path, "ought/missing.ought.md");
        assert!(result.is_err());
        let err = result.err().unwrap();

        assert!(
            err.to_string().contains("no spec files matched"),
            "unexpected error: {err}"
        );
    }
}
