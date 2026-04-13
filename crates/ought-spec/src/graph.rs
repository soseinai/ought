use std::collections::{HashMap, VecDeque};
use std::path::{Component, Path, PathBuf};

use crate::parser::{OughtMdParser, Parser};
use crate::types::{ParseError, Spec};

/// A directed graph of spec files, built from `requires:` references.
///
/// Resolves cross-file dependencies at construction time, rejects cycles,
/// and provides topological ordering for downstream execution.
#[derive(Debug)]
pub struct SpecGraph {
    specs: Vec<Spec>,
    /// Directed edges `(dependent_idx, dependency_idx)` into `specs`. Resolved
    /// once at construction and reused by `topological_order` and anything
    /// else that needs to walk the `requires:` graph.
    edges: Vec<(usize, usize)>,
}

impl SpecGraph {
    /// Discover and parse every `.ought.md` file under the given roots using
    /// the default [`OughtMdParser`], then build the graph.
    pub fn from_roots(roots: &[PathBuf]) -> Result<Self, Vec<ParseError>> {
        Self::from_roots_with(&OughtMdParser, roots)
    }

    /// Same as [`SpecGraph::from_roots`] but parses with a caller-supplied
    /// [`Parser`]. Useful for tests and for plugging in alternative spec
    /// formats.
    pub fn from_roots_with(
        parser: &dyn Parser,
        roots: &[PathBuf],
    ) -> Result<Self, Vec<ParseError>> {
        let files = collect_files(roots);
        let (specs, mut errors) = parse_all(parser, &files);

        match Self::from_specs(specs) {
            Ok(graph) => {
                if errors.is_empty() {
                    Ok(graph)
                } else {
                    Err(errors)
                }
            }
            Err(graph_errors) => {
                errors.extend(graph_errors);
                Err(errors)
            }
        }
    }

    /// Build a graph from already-parsed specs. Validates that every
    /// `requires:` entry resolves to a spec in the set and rejects cycles.
    ///
    /// Prefer this when callers have their own parsing pipeline (tests, the
    /// viewer, any consumer that wants to inspect or transform specs before
    /// graph construction).
    pub fn from_specs(specs: Vec<Spec>) -> Result<Self, Vec<ParseError>> {
        let (edges, mut errors) = resolve_references(&specs);
        errors.extend(detect_cycles(&specs, &edges));

        if errors.is_empty() {
            Ok(Self { specs, edges })
        } else {
            Err(errors)
        }
    }

    /// All parsed specs.
    pub fn specs(&self) -> &[Spec] {
        &self.specs
    }

    /// Specs in topological order (dependencies before dependents). Uses
    /// Kahn's algorithm over the pre-resolved edges.
    pub fn topological_order(&self) -> Vec<&Spec> {
        let n = self.specs.len();
        if n == 0 {
            return Vec::new();
        }

        let mut in_degree = vec![0usize; n];
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n];

        for &(dependent, dependency) in &self.edges {
            // Edge direction for ordering: dependency must come before
            // dependent, so walk from dependency → dependent.
            adjacency[dependency].push(dependent);
            in_degree[dependent] += 1;
        }

        let mut queue: VecDeque<usize> = in_degree
            .iter()
            .enumerate()
            .filter_map(|(i, &d)| if d == 0 { Some(i) } else { None })
            .collect();

        let mut order = Vec::with_capacity(n);
        while let Some(node) = queue.pop_front() {
            order.push(&self.specs[node]);
            for &neighbor in &adjacency[node] {
                in_degree[neighbor] -= 1;
                if in_degree[neighbor] == 0 {
                    queue.push_back(neighbor);
                }
            }
        }

        // If order.len() < n there's a cycle, but construction already
        // rejects those — so in practice order always covers every spec.
        order
    }

    /// Look up a spec by its source file path.
    pub fn get_by_path(&self, path: &Path) -> Option<&Spec> {
        self.specs.iter().find(|s| s.source_path == path)
    }
}

/// Walk each root directory and gather every `*.ought.md` file, deduplicated
/// and sorted for determinism.
fn collect_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut all_files = Vec::new();
    for root in roots {
        all_files.extend(collect_ought_files(root));
    }
    all_files.sort();
    all_files.dedup();
    all_files
}

/// Parse every file with the given parser, accumulating specs and any
/// per-file parse errors separately.
fn parse_all(parser: &dyn Parser, files: &[PathBuf]) -> (Vec<Spec>, Vec<ParseError>) {
    let mut specs = Vec::new();
    let mut errors = Vec::new();
    for file in files {
        match parser.parse_file(file) {
            Ok(spec) => specs.push(spec),
            Err(errs) => errors.extend(errs),
        }
    }
    (specs, errors)
}

/// Recursively walk a directory and collect all files matching `*.ought.md`.
fn collect_ought_files(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(collect_ought_files(&path));
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.ends_with(".ought.md")
            {
                results.push(path);
            }
        }
    }
    results
}

/// Collapse `.` and `..` components without touching the filesystem so a
/// path like `ought/analysis/../engine/parser.ought.md` compares equal to the
/// `ought/engine/parser.ought.md` produced by directory traversal.
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

/// Resolve every `requires:` reference in `specs` to a target index.
///
/// Returns `(edges, errors)` where each edge is `(dependent_idx, dependency_idx)`
/// and errors flag references that didn't match any spec in the set.
fn resolve_references(specs: &[Spec]) -> (Vec<(usize, usize)>, Vec<ParseError>) {
    let path_to_idx: HashMap<&PathBuf, usize> = specs
        .iter()
        .enumerate()
        .map(|(i, s)| (&s.source_path, i))
        .collect();

    let mut edges = Vec::new();
    let mut errors = Vec::new();

    for (i, spec) in specs.iter().enumerate() {
        for req in &spec.metadata.requires {
            // Resolve the requires path relative to the spec's directory, then
            // fall back to a raw lookup for refs written as absolute or
            // already-root-relative paths.
            let base_dir = spec.source_path.parent().unwrap_or(Path::new(""));
            let resolved = normalize_path(&base_dir.join(&req.path));

            match path_to_idx
                .get(&resolved)
                .or_else(|| path_to_idx.get(&req.path))
            {
                Some(&j) => edges.push((i, j)),
                None => errors.push(ParseError {
                    file: spec.source_path.clone(),
                    line: 0,
                    message: format!(
                        "unresolved cross-reference: '{}' (resolved to '{}')",
                        req.path.display(),
                        resolved.display()
                    ),
                }),
            }
        }
    }

    (edges, errors)
}

/// Detect cycles in the dependency graph formed by `edges`. Uses DFS colouring.
fn detect_cycles(specs: &[Spec], edges: &[(usize, usize)]) -> Vec<ParseError> {
    let n = specs.len();
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(dependent, dependency) in edges {
        // Walk from dependent → dependency so a cycle is detected the same
        // way it was historically (starts at a node, follows its requires).
        adjacency[dependent].push(dependency);
    }

    let mut errors = Vec::new();
    let mut visited = vec![0u8; n]; // 0=unvisited, 1=in-progress, 2=done

    fn dfs(
        node: usize,
        adjacency: &[Vec<usize>],
        visited: &mut [u8],
        path: &mut Vec<usize>,
        specs: &[Spec],
        errors: &mut Vec<ParseError>,
    ) {
        visited[node] = 1;
        path.push(node);

        for &neighbor in &adjacency[node] {
            if visited[neighbor] == 1 {
                // Found a cycle
                let cycle_start = path.iter().position(|&n| n == neighbor).unwrap();
                let cycle_names: Vec<String> = path[cycle_start..]
                    .iter()
                    .map(|&i| specs[i].source_path.display().to_string())
                    .collect();
                errors.push(ParseError {
                    file: specs[node].source_path.clone(),
                    line: 0,
                    message: format!(
                        "circular dependency detected: {}",
                        cycle_names.join(" -> ")
                    ),
                });
            } else if visited[neighbor] == 0 {
                dfs(neighbor, adjacency, visited, path, specs, errors);
            }
        }

        path.pop();
        visited[node] = 2;
    }

    let mut path = Vec::new();
    for i in 0..n {
        if visited[i] == 0 {
            dfs(i, &adjacency, &mut visited, &mut path, specs, &mut errors);
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::types::{Metadata, SpecRef};

    /// Build a minimal `Spec` at `path` that `requires:` each of `deps`.
    /// Sections are empty — we only care about `metadata.requires` here.
    fn spec(path: &str, deps: &[&str]) -> Spec {
        let requires = deps
            .iter()
            .map(|d| SpecRef {
                label: d.to_string(),
                path: PathBuf::from(d),
                anchor: None,
            })
            .collect();
        Spec {
            name: path.to_string(),
            metadata: Metadata {
                context: None,
                sources: Vec::new(),
                schemas: Vec::new(),
                requires,
            },
            sections: Vec::new(),
            source_path: PathBuf::from(path),
        }
    }

    fn names(specs: &[&Spec]) -> Vec<String> {
        specs.iter().map(|s| s.source_path.display().to_string()).collect()
    }

    #[test]
    fn empty_graph_is_valid() {
        let graph = SpecGraph::from_specs(vec![]).expect("empty graph must build");
        assert!(graph.specs().is_empty());
        assert!(graph.topological_order().is_empty());
    }

    #[test]
    fn single_spec_no_requires_orders_to_itself() {
        let graph = SpecGraph::from_specs(vec![spec("a.ought.md", &[])]).unwrap();
        let order = graph.topological_order();
        assert_eq!(names(&order), vec!["a.ought.md"]);
    }

    #[test]
    fn linear_dependency_orders_dependency_before_dependent() {
        // a requires b → b must come first
        let graph = SpecGraph::from_specs(vec![
            spec("a.ought.md", &["b.ought.md"]),
            spec("b.ought.md", &[]),
        ])
        .unwrap();
        let order = names(&graph.topological_order());
        let pos_a = order.iter().position(|p| p == "a.ought.md").unwrap();
        let pos_b = order.iter().position(|p| p == "b.ought.md").unwrap();
        assert!(pos_b < pos_a, "b must precede a; got {order:?}");
    }

    #[test]
    fn diamond_graph_orders_correctly() {
        // d → {b, c}, b → a, c → a. Expect a first, d last.
        let graph = SpecGraph::from_specs(vec![
            spec("a.ought.md", &[]),
            spec("b.ought.md", &["a.ought.md"]),
            spec("c.ought.md", &["a.ought.md"]),
            spec("d.ought.md", &["b.ought.md", "c.ought.md"]),
        ])
        .unwrap();
        let order = names(&graph.topological_order());
        let pos = |p: &str| order.iter().position(|x| x == p).unwrap();
        assert!(pos("a.ought.md") < pos("b.ought.md"));
        assert!(pos("a.ought.md") < pos("c.ought.md"));
        assert!(pos("b.ought.md") < pos("d.ought.md"));
        assert!(pos("c.ought.md") < pos("d.ought.md"));
    }

    #[test]
    fn direct_cycle_is_rejected() {
        // a → b, b → a
        let errors = SpecGraph::from_specs(vec![
            spec("a.ought.md", &["b.ought.md"]),
            spec("b.ought.md", &["a.ought.md"]),
        ])
        .expect_err("cycle must be rejected");
        assert!(
            errors.iter().any(|e| e.message.contains("circular dependency")),
            "expected circular-dependency error; got {errors:?}"
        );
    }

    #[test]
    fn self_loop_is_rejected() {
        let errors = SpecGraph::from_specs(vec![spec("a.ought.md", &["a.ought.md"])])
            .expect_err("self-loop must be rejected");
        assert!(
            errors.iter().any(|e| e.message.contains("circular dependency")),
            "expected circular-dependency error; got {errors:?}"
        );
    }

    #[test]
    fn unresolved_reference_is_rejected() {
        let errors = SpecGraph::from_specs(vec![spec("a.ought.md", &["missing.ought.md"])])
            .expect_err("unresolved ref must be rejected");
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("unresolved") && e.message.contains("missing.ought.md")),
            "expected unresolved-reference error; got {errors:?}"
        );
    }

    #[test]
    fn unrelated_specs_have_no_ordering_constraint() {
        // Two disconnected specs — both appear in the order, either first.
        let graph = SpecGraph::from_specs(vec![
            spec("a.ought.md", &[]),
            spec("b.ought.md", &[]),
        ])
        .unwrap();
        let order = names(&graph.topological_order());
        assert_eq!(order.len(), 2);
        assert!(order.contains(&"a.ought.md".to_string()));
        assert!(order.contains(&"b.ought.md".to_string()));
    }
}
