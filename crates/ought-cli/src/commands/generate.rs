use std::process;
use std::sync::{Arc, Mutex};

use ought_gen::manifest::Manifest;
use ought_gen::{AgentRunStatus, AgentReport};

use super::{
    build_agent_assignments, collect_all_testable_ids, collect_section_groups, load_config,
    load_specs, primary_test_dir,
};
use crate::{Cli, GenerateArgs};

pub fn run(cli: &Cli, args: &GenerateArgs) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;
    let specs = load_specs(&config, &config_path)?;

    let test_dir = primary_test_dir(&config, &config_path);

    std::fs::create_dir_all(&test_dir)?;

    let manifest_path = test_dir.join("manifest.toml");
    let mut manifest = Manifest::load(&manifest_path).unwrap_or_default();

    let groups = collect_section_groups(&specs);

    let mut generated_count = 0usize;
    let mut error_count = 0usize;
    let mut stale_count = 0usize;

    if args.check {
        for group in &groups {
            for clause in &group.testable_clauses {
                if clause.pending {
                    continue;
                }
                if args.force || manifest.is_stale(&clause.id, &clause.content_hash, "") {
                    eprintln!("  stale: {}", clause.id);
                    stale_count += 1;
                }
            }
        }
    } else {
        let assignments = build_agent_assignments(
            &groups,
            &manifest,
            &config,
            &config_path,
            &test_dir,
            config.generator.parallelism.max(1),
            args.force,
        );

        if assignments.is_empty() {
            eprintln!("All tests up to date, nothing to generate.");
        } else {
            let total_clauses: usize = assignments
                .iter()
                .map(|a| a.groups.iter().map(|g| g.clauses.len()).sum::<usize>())
                .sum();
            eprintln!(
                "{} assignments, {} clauses to generate",
                assignments.len(),
                total_clauses
            );

            // Hand the manifest to the orchestrator behind a shared lock so
            // tool primitives can update it in place; we'll read it back
            // out at the end to persist orphan removal.
            let shared_manifest = Arc::new(Mutex::new(std::mem::take(&mut manifest)));

            let orchestrator = ought_gen::Orchestrator::new(
                config.generator.clone(),
                shared_manifest.clone(),
                manifest_path.clone(),
                cli.verbose,
            );

            let reports = tokio::runtime::Runtime::new()?.block_on(orchestrator.run(assignments))?;

            for report in &reports {
                render_report(report, cli.verbose);
                generated_count += report.generated.len();
                error_count += report.write_errors.len() + report.errors.len();
            }

            // Recover the manifest for orphan cleanup and final save.
            manifest = Arc::try_unwrap(shared_manifest)
                .map_err(|_| anyhow::anyhow!("manifest still has outstanding references"))?
                .into_inner()
                .map_err(|_| anyhow::anyhow!("manifest mutex poisoned"))?;
        }
    }

    let all_ids = collect_all_testable_ids(&specs);
    let id_refs: Vec<&ought_spec::ClauseId> = all_ids.iter().collect();
    manifest.remove_orphans(&id_refs);

    manifest.save(&manifest_path)?;

    eprintln!("\n{} generated, {} errors", generated_count, error_count);

    if args.check && stale_count > 0 {
        eprintln!("{} stale clauses", stale_count);
        process::exit(1);
    }

    Ok(())
}

fn render_report(report: &AgentReport, verbose: bool) {
    let status_label = match report.status {
        AgentRunStatus::Completed => "completed",
        AgentRunStatus::MaxTurnsExceeded => "max-turns-exceeded",
        AgentRunStatus::Truncated => "truncated",
        AgentRunStatus::ContextExhausted => "context-exhausted",
        AgentRunStatus::Errored => "errored",
        AgentRunStatus::NotRun => "not-run",
    };
    eprintln!(
        "  [agent {}] {}: {} written, {} write errors, {} turns, {}/{} tokens (in/out)",
        report.assignment_id,
        status_label,
        report.generated.len(),
        report.write_errors.len(),
        report.turns,
        report.usage_input_tokens,
        report.usage_output_tokens,
    );
    for err in &report.errors {
        eprintln!("    error: {}", err);
    }
    for (clause_id, msg) in &report.write_errors {
        eprintln!("    write_error[{}]: {}", clause_id, msg);
    }
    if verbose && (report.usage_cache_read_tokens + report.usage_cache_creation_tokens) > 0 {
        eprintln!(
            "    cache: {} read, {} created",
            report.usage_cache_read_tokens, report.usage_cache_creation_tokens
        );
    }
}
