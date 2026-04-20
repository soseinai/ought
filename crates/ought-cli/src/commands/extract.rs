//! `ought extract` — reverse-engineer `.ought.md` specs from a codebase,
//! with a rule-based audit of existing specs run first.
//!
//! Combined command: audits whatever specs already exist, then dispatches
//! LLM agents to draft new specs for uncovered source areas. Cold-start
//! sibling of `survey`, but writes files.

use std::path::{Path, PathBuf};

use ought_gen::{ExtractAssignment, ExtractGroup, ExtractOrchestrator, ExtractRunStatus};
use ought_spec::SpecGraph;

use super::{load_config, load_specs};
use crate::{Cli, ExtractArgs};

pub fn run(cli: &Cli, args: &ExtractArgs) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;
    let config_dir = config_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    // 0. Audit phase — rule-based, runs on whatever specs already exist.
    if !args.no_audit {
        run_audit_phase(&config, &config_path)?;
    }

    // 1. Resolve source paths.
    let search_paths: Vec<PathBuf> = if !args.paths.is_empty() {
        args.paths.clone()
    } else {
        config
            .context
            .search_paths
            .iter()
            .map(|p| config_dir.join(p))
            .collect()
    };
    if search_paths.is_empty() {
        anyhow::bail!(
            "no source paths: pass paths as arguments or set [context].search_paths in ought.toml"
        );
    }
    for p in &search_paths {
        if !p.exists() {
            anyhow::bail!("source path does not exist: {}", p.display());
        }
    }

    // 2. Resolve output spec root.
    let specs_root = if let Some(ref out) = args.out {
        out.clone()
    } else {
        let first = config
            .specs
            .roots
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("ought/"));
        config_dir.join(first)
    };
    std::fs::create_dir_all(&specs_root)?;

    // 3. Walk + group.
    let groups = build_groups(&search_paths, config.context.max_files);
    if groups.is_empty() {
        eprintln!("No source files found to extract from.");
        return Ok(());
    }

    // 4. Pre-flight: skip groups whose target already exists unless force.
    // Under dry-run we always preview, regardless of what's on disk.
    let (live_groups, skipped_groups): (Vec<_>, Vec<_>) = groups.into_iter().partition(|g| {
        let target = specs_root.join(&g.target_spec_path);
        args.force || args.dry_run || !target.exists()
    });
    for skipped in &skipped_groups {
        eprintln!(
            "  skip: {} already exists (rerun with --force to overwrite)",
            specs_root.join(&skipped.target_spec_path).display()
        );
    }
    if live_groups.is_empty() {
        eprintln!("Nothing to extract: every target already exists. Use --force to regenerate.");
        return Ok(());
    }

    let total_files: usize = live_groups.iter().map(|g| g.source_files.len()).sum();
    eprintln!(
        "{} spec file(s) to draft from {} source file(s){}",
        live_groups.len(),
        total_files,
        if args.dry_run { " (dry run)" } else { "" }
    );

    // 5. Partition across agents round-robin.
    let parallelism = args
        .parallelism
        .unwrap_or(config.generator.parallelism)
        .max(1);
    let assignments = build_assignments(
        live_groups,
        &config_path,
        &specs_root,
        parallelism,
        args.dry_run,
        args.force,
    );

    // 6. Apply optional model override and run the agent loop.
    let mut gen_cfg = config.generator.clone();
    if let Some(ref m) = args.model {
        gen_cfg.model = m.clone();
    }

    let orchestrator = ExtractOrchestrator::new(gen_cfg, cli.verbose);
    let reports = tokio::runtime::Runtime::new()?.block_on(orchestrator.run(assignments))?;

    let mut written = 0usize;
    let mut errors = 0usize;
    for report in &reports {
        render_report(report);
        written += report.written.len();
        errors += report.write_errors.len() + report.errors.len();
    }

    eprintln!("\n{} written, {} errors", written, errors);
    Ok(())
}

// ── Audit phase ────────────────────────────────────────────────────────────

fn run_audit_phase(
    config: &ought_cli::config::Config,
    config_path: &Path,
) -> anyhow::Result<()> {
    let specs: SpecGraph = match load_specs(config, config_path) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    if specs.specs().is_empty() {
        return Ok(());
    }

    let result = ought_analysis::audit::audit(&specs)?;

    eprintln!("[audit]");
    if result.findings.is_empty() {
        eprintln!("  No issues found in existing specs.");
        eprintln!();
        return Ok(());
    }
    eprintln!("  Found {} issue(s):", result.findings.len());
    for finding in &result.findings {
        let kind_str = match finding.kind {
            ought_analysis::AuditFindingKind::Contradiction => "\x1b[31mCONTRADICTION\x1b[0m",
            ought_analysis::AuditFindingKind::Gap => "\x1b[33mGAP\x1b[0m",
            ought_analysis::AuditFindingKind::Ambiguity => "\x1b[34mAMBIGUITY\x1b[0m",
            ought_analysis::AuditFindingKind::Redundancy => "\x1b[36mREDUNDANCY\x1b[0m",
        };
        eprintln!("    [{}] {}", kind_str, finding.description);
        if !finding.clauses.is_empty() {
            eprintln!(
                "      Clauses: {}",
                finding
                    .clauses
                    .iter()
                    .map(|c| c.0.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if let Some(ref suggestion) = finding.suggestion {
            eprintln!("      Suggestion: {}", suggestion);
        }
    }
    eprintln!();
    Ok(())
}

// ── Reporting ──────────────────────────────────────────────────────────────

fn render_report(report: &ought_gen::ExtractReport) {
    let status_label = match report.status {
        ExtractRunStatus::Completed => "completed",
        ExtractRunStatus::MaxTurnsExceeded => "max-turns-exceeded",
        ExtractRunStatus::Truncated => "truncated",
        ExtractRunStatus::ContextExhausted => "context-exhausted",
        ExtractRunStatus::Errored => "errored",
        ExtractRunStatus::NotRun => "not-run",
    };
    eprintln!(
        "  [extract agent {}] {}: {} written, {} write errors, {} turns, {}/{} tokens (in/out)",
        report.assignment_id,
        status_label,
        report.written.len(),
        report.write_errors.len(),
        report.turns,
        report.usage_input_tokens,
        report.usage_output_tokens,
    );
    for err in &report.errors {
        eprintln!("    error: {}", err);
    }
    for (target, msg) in &report.write_errors {
        eprintln!("    write_error[{}]: {}", target, msg);
    }
}

// ── Grouping ───────────────────────────────────────────────────────────────

/// Walk the search paths and group files by their first path component
/// relative to the search path. Each group becomes one output `.ought.md`.
///
/// Uses `specs_root` only for the output filename shape; the walker
/// itself doesn't touch it.
pub(crate) fn build_groups(search_paths: &[PathBuf], max_files: usize) -> Vec<ExtractGroup> {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    let mut file_count = 0usize;

    for root in search_paths {
        let root_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root")
            .to_string();

        walk_source_dir(root, &mut |path| {
            if file_count >= max_files {
                return;
            }
            let rel = match path.strip_prefix(root) {
                Ok(r) => r,
                Err(_) => return,
            };
            let first = rel.components().next();
            let (key, title) = match first {
                Some(std::path::Component::Normal(os)) => {
                    let name = os.to_string_lossy().to_string();
                    if rel.components().count() == 1 {
                        (root_name.clone(), root_name.clone())
                    } else {
                        (name.clone(), name)
                    }
                }
                _ => (root_name.clone(), root_name.clone()),
            };
            groups
                .entry(key)
                .or_insert_with(|| (title, Vec::new()))
                .1
                .push(path.to_string_lossy().to_string());
            file_count += 1;
        });
    }

    let mut out = Vec::new();
    for (key, (title, sources)) in groups {
        let safe = key.replace(['/', '\\', '.'], "_");
        let target = format!("{}.ought.md", safe);
        out.push(ExtractGroup {
            title: pretty_title(&title),
            target_spec_path: target,
            source_files: sources,
        });
    }

    if file_count >= max_files {
        eprintln!(
            "  note: stopped at max_files={} (set [context].max_files higher to include more)",
            max_files
        );
    }

    out
}

fn pretty_title(name: &str) -> String {
    name.replace(['_', '-'], " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn walk_source_dir(dir: &Path, cb: &mut dyn FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.starts_with('.') {
            continue;
        }
        if matches!(
            name.as_str(),
            "target"
                | "node_modules"
                | "__pycache__"
                | "vendor"
                | ".venv"
                | "venv"
                | "dist"
                | "build"
        ) {
            continue;
        }
        if path.is_dir() {
            walk_source_dir(&path, cb);
        } else if path.is_file() && is_source_file(&path) {
            cb(&path);
        }
    }
}

fn is_source_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext,
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "go" | "java" | "rb" | "kt" | "swift"
    )
}

fn build_assignments(
    groups: Vec<ExtractGroup>,
    config_path: &Path,
    specs_root: &Path,
    parallelism: usize,
    dry_run: bool,
    force: bool,
) -> Vec<ExtractAssignment> {
    let project_root = config_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_string_lossy()
        .to_string();

    let n = parallelism.min(groups.len()).max(1);
    let mut buckets: Vec<Vec<ExtractGroup>> = (0..n).map(|_| Vec::new()).collect();
    for (i, g) in groups.into_iter().enumerate() {
        buckets[i % n].push(g);
    }

    buckets
        .into_iter()
        .enumerate()
        .filter(|(_, gs)| !gs.is_empty())
        .map(|(i, gs)| ExtractAssignment {
            id: format!("extract_{}", i),
            project_root: project_root.clone(),
            config_path: config_path.to_string_lossy().into_owned(),
            specs_root: specs_root.to_string_lossy().into_owned(),
            dry_run,
            force,
            groups: gs,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn touch(p: &Path) {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, "").unwrap();
    }

    #[test]
    fn groups_by_top_level_directory() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        touch(&src.join("auth/login.rs"));
        touch(&src.join("auth/token.rs"));
        touch(&src.join("cli/main.rs"));

        let groups = build_groups(&[src], 100);
        let keys: Vec<_> = groups.iter().map(|g| g.target_spec_path.as_str()).collect();
        assert!(keys.contains(&"auth.ought.md"));
        assert!(keys.contains(&"cli.ought.md"));

        let auth = groups
            .iter()
            .find(|g| g.target_spec_path == "auth.ought.md")
            .unwrap();
        assert_eq!(auth.source_files.len(), 2);
        assert_eq!(auth.title, "Auth");
    }

    #[test]
    fn root_level_files_land_in_search_path_group() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        touch(&src.join("main.rs"));
        touch(&src.join("lib.rs"));
        touch(&src.join("auth/login.rs"));

        let groups = build_groups(&[src], 100);
        let src_group = groups
            .iter()
            .find(|g| g.target_spec_path == "src.ought.md")
            .expect("expected src group for root-level files");
        assert_eq!(src_group.source_files.len(), 2);
    }

    #[test]
    fn skips_target_and_build_dirs() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        touch(&src.join("auth/login.rs"));
        touch(&src.join("target/debug/build.rs"));
        touch(&src.join("node_modules/foo/bar.js"));

        let groups = build_groups(&[src], 100);
        let keys: Vec<_> = groups.iter().map(|g| g.target_spec_path.as_str()).collect();
        assert!(keys.contains(&"auth.ought.md"));
        assert!(
            !keys
                .iter()
                .any(|k| k.contains("target") || k.contains("node_modules"))
        );
    }

    #[test]
    fn respects_max_files_cap() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        for i in 0..10 {
            touch(&src.join(format!("m{}/file.rs", i)));
        }
        let groups = build_groups(&[src], 3);
        let total: usize = groups.iter().map(|g| g.source_files.len()).sum();
        assert!(total <= 3, "expected cap at 3, got {}", total);
    }

    #[test]
    fn build_assignments_round_robins_groups() {
        let groups = vec![
            ExtractGroup {
                title: "A".into(),
                target_spec_path: "a.ought.md".into(),
                source_files: vec!["a".into()],
            },
            ExtractGroup {
                title: "B".into(),
                target_spec_path: "b.ought.md".into(),
                source_files: vec!["b".into()],
            },
            ExtractGroup {
                title: "C".into(),
                target_spec_path: "c.ought.md".into(),
                source_files: vec!["c".into()],
            },
        ];
        let asns = build_assignments(
            groups,
            Path::new("/tmp/ought.toml"),
            Path::new("/tmp/ought"),
            2,
            false,
            false,
        );
        assert_eq!(asns.len(), 2);
        assert_eq!(asns[0].groups.len(), 2);
        assert_eq!(asns[1].groups.len(), 1);
    }

    #[test]
    fn pretty_title_capitalizes_and_cleans() {
        assert_eq!(pretty_title("user-auth"), "User Auth");
        assert_eq!(pretty_title("my_module"), "My Module");
        assert_eq!(pretty_title("cli"), "Cli");
    }
}
