use std::path::{Path, PathBuf};

use ought_spec::{Clause, Config, Spec};

use super::generator::Language;

/// A source file read into memory for inclusion in the LLM prompt.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub content: String,
}

/// The assembled context sent to the LLM alongside a clause.
#[derive(Debug, Clone)]
pub struct GenerationContext {
    pub spec_context: Option<String>,
    pub source_files: Vec<SourceFile>,
    pub schema_files: Vec<SourceFile>,
    pub target_language: Language,
}

/// Assembles context for LLM generation by reading source files,
/// schemas, and free-text context from spec metadata.
pub struct ContextAssembler<'a> {
    config: &'a Config,
}

impl<'a> ContextAssembler<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    /// Assemble context for a clause, reading files from `source:` and `schema:`
    /// metadata, plus auto-discovering relevant source when no hints are given.
    pub fn assemble(&self, clause: &Clause, spec: &Spec) -> anyhow::Result<GenerationContext> {
        let spec_dir = spec
            .source_path
            .parent()
            .unwrap_or_else(|| Path::new("."));

        // Read source files from spec metadata
        let mut source_files = Vec::new();
        for source_path_str in &spec.metadata.sources {
            let resolved = spec_dir.join(source_path_str);
            if resolved.is_file() {
                match self.read_source(&resolved) {
                    Ok(sf) => source_files.push(sf),
                    Err(_) => continue,
                }
            } else if resolved.is_dir() {
                // If it's a directory, read all files in it (non-recursive for simplicity)
                if let Ok(entries) = std::fs::read_dir(&resolved) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() && source_files.len() < self.config.context.max_files
                            && let Ok(sf) = self.read_source(&p) {
                                source_files.push(sf);
                            }
                    }
                }
            }
        }

        // If no explicit sources, auto-discover
        if source_files.is_empty()
            && let Ok(discovered) = self.discover_sources(clause) {
                source_files = discovered;
            }

        // Read schema files from spec metadata
        let mut schema_files = Vec::new();
        for schema_path_str in &spec.metadata.schemas {
            let resolved = spec_dir.join(schema_path_str);
            if resolved.is_file()
                && let Ok(sf) = self.read_source(&resolved) {
                    schema_files.push(sf);
                }
        }

        // Determine target language from generator config or default to Rust
        let target_language = detect_language(self.config);

        Ok(GenerationContext {
            spec_context: spec.metadata.context.clone(),
            source_files,
            schema_files,
            target_language,
        })
    }

    /// Auto-discover source files relevant to a clause by keyword matching
    /// against files in the configured search paths.
    pub fn discover_sources(&self, clause: &Clause) -> anyhow::Result<Vec<SourceFile>> {
        let search_paths = &self.config.context.search_paths;
        if search_paths.is_empty() {
            return Ok(Vec::new());
        }

        // Extract keywords from clause text for matching
        let clause_words: Vec<String> = clause
            .text
            .split_whitespace()
            .map(|w| w.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 3) // skip short words
            .collect();

        let mut candidates: Vec<(PathBuf, usize)> = Vec::new();

        for search_path in search_paths {
            collect_files_recursive(search_path, &self.config.context.exclude, &mut |path| {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let lower_content = content.to_lowercase();
                    let score: usize = clause_words
                        .iter()
                        .filter(|w| lower_content.contains(w.as_str()))
                        .count();
                    if score > 0 {
                        candidates.push((path.to_path_buf(), score));
                    }
                }
            });
        }

        // Sort by relevance (highest score first)
        candidates.sort_by(|a, b| b.1.cmp(&a.1));

        // Take top N files up to max_files
        let max = self.config.context.max_files;
        let mut results = Vec::new();
        for (path, _score) in candidates.into_iter().take(max) {
            if let Ok(sf) = self.read_source(&path) {
                results.push(sf);
            }
        }

        Ok(results)
    }

    /// Read a source file, respecting the max_files limit.
    pub fn read_source(&self, path: &Path) -> anyhow::Result<SourceFile> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("failed to read source file {}: {}", path.display(), e))?;
        Ok(SourceFile {
            path: path.to_path_buf(),
            content,
        })
    }
}

/// Recursively collect files from a directory, skipping excluded patterns.
fn collect_files_recursive(
    dir: &Path,
    exclude: &[String],
    callback: &mut dyn FnMut(&Path),
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Skip excluded patterns
        if exclude.iter().any(|pat| file_name.contains(pat.as_str())) {
            continue;
        }

        // Skip hidden files/dirs
        if file_name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_files_recursive(&path, exclude, callback);
        } else if path.is_file() {
            callback(&path);
        }
    }
}

/// Detect target language from config. Defaults to Rust.
fn detect_language(config: &Config) -> Language {
    // Try to infer from project name or default
    let name = config.project.name.to_lowercase();
    if name.contains("python") || name.contains("py") {
        Language::Python
    } else if name.contains("typescript") || name.contains("ts") {
        Language::TypeScript
    } else if name.contains("javascript") || name.contains("js") {
        Language::JavaScript
    } else if name.contains("go") || name.contains("golang") {
        Language::Go
    } else {
        Language::Rust
    }
}
