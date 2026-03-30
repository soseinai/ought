use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use crate::parser::Parser;
use crate::types::{ParseError, Spec};

/// A directed acyclic graph of spec files, built from `requires:` references.
///
/// Handles cross-file dependencies, detects circular references,
/// and provides topological ordering for execution.
pub struct SpecGraph {
    specs: Vec<Spec>,
}

/// Recursively walk a directory and collect all files matching `*.ought.md`.
fn collect_ought_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(collect_ought_files(&path));
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.ends_with(".ought.md") {
                    results.push(path);
                }
        }
    }
    results
}

impl SpecGraph {
    /// Discover and parse all spec files from the given root directories.
    /// Builds the dependency graph from `requires:` metadata.
    pub fn from_roots(roots: &[PathBuf]) -> Result<Self, Vec<ParseError>> {
        let mut all_files = Vec::new();
        for root in roots {
            let files = collect_ought_files(root);
            all_files.extend(files);
        }

        // Deduplicate by canonical path
        all_files.sort();
        all_files.dedup();

        let mut specs = Vec::new();
        let mut all_errors = Vec::new();

        for file in &all_files {
            match Parser::parse_file(file) {
                Ok(spec) => specs.push(spec),
                Err(errors) => all_errors.extend(errors),
            }
        }

        // Check for cycles in the dependency graph
        let cycle_errors = detect_cycles(&specs);
        all_errors.extend(cycle_errors);

        if !all_errors.is_empty() {
            return Err(all_errors);
        }

        Ok(Self { specs })
    }

    /// All parsed specs.
    pub fn specs(&self) -> &[Spec] {
        &self.specs
    }

    /// Specs in topological order (dependencies before dependents).
    /// Uses Kahn's algorithm.
    pub fn topological_order(&self) -> Vec<&Spec> {
        if self.specs.is_empty() {
            return Vec::new();
        }

        // Build index by source_path
        let path_to_idx: HashMap<&PathBuf, usize> = self
            .specs
            .iter()
            .enumerate()
            .map(|(i, s)| (&s.source_path, i))
            .collect();

        let n = self.specs.len();
        let mut in_degree = vec![0usize; n];
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (i, spec) in self.specs.iter().enumerate() {
            for req in &spec.metadata.requires {
                // Try to resolve the requires path relative to the spec's directory
                let base_dir = spec.source_path.parent().unwrap_or(std::path::Path::new(""));
                let resolved = base_dir.join(&req.path);

                // Find matching spec by path (try both resolved and raw)
                let target_idx = path_to_idx.get(&resolved).or_else(|| path_to_idx.get(&req.path));

                if let Some(&j) = target_idx {
                    // Edge: j -> i (dependency j must come before dependent i)
                    adjacency[j].push(i);
                    in_degree[i] += 1;
                }
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<usize> = VecDeque::new();
        for (i, &deg) in in_degree.iter().enumerate() {
            if deg == 0 {
                queue.push_back(i);
            }
        }

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

        // If order.len() < n, there's a cycle. We still return what we have.
        order
    }

    /// Look up a spec by its source file path.
    pub fn get_by_path(&self, path: &std::path::Path) -> Option<&Spec> {
        self.specs.iter().find(|s| s.source_path == path)
    }
}

/// Detect cycles in the dependency graph formed by `requires:` references.
fn detect_cycles(specs: &[Spec]) -> Vec<ParseError> {
    let path_to_idx: HashMap<&PathBuf, usize> = specs
        .iter()
        .enumerate()
        .map(|(i, s)| (&s.source_path, i))
        .collect();

    let n = specs.len();
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n];

    for (i, spec) in specs.iter().enumerate() {
        for req in &spec.metadata.requires {
            let base_dir = spec
                .source_path
                .parent()
                .unwrap_or(std::path::Path::new(""));
            let resolved = base_dir.join(&req.path);

            let target_idx = path_to_idx.get(&resolved).or_else(|| path_to_idx.get(&req.path));

            if let Some(&j) = target_idx {
                adjacency[i].push(j);
            }
        }
    }

    // DFS-based cycle detection
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
