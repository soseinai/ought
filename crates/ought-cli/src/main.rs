use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

use ought_gen::manifest::Manifest;
use ought_report::types::{ColorChoice as ReportColor, ReportOptions};
use ought_run::runners;
use ought_spec::{Config, SpecGraph};


#[derive(Parser)]
#[command(name = "ought", about = "Behavioral test framework powered by LLMs")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Path to ought.toml config file.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Suppress all output except errors and the final summary.
    #[arg(long, global = true)]
    quiet: bool,

    /// Output structured JSON instead of human-readable text.
    #[arg(long, global = true)]
    json: bool,

    /// Write JUnit XML results to the given file.
    #[arg(long, global = true)]
    junit: Option<PathBuf>,

    /// Control terminal color output.
    #[arg(long, global = true, default_value = "auto")]
    color: ColorChoice,

    /// Enable debug-level output.
    #[arg(long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold ought.toml and an example spec in a new project.
    Init,

    /// Execute generated tests and report results.
    Run(RunArgs),

    /// Regenerate test code from specs using the LLM.
    Generate(GenerateArgs),

    /// Validate spec file syntax without generating or running.
    Check,

    /// Show generated test code for a clause.
    Inspect(InspectArgs),

    /// Show diff between current and pending generated tests.
    Diff,

    /// Discover source behaviors not covered by any spec.
    Survey(SurveyArgs),

    /// Analyze specs for contradictions, gaps, and coherence issues.
    Audit,

    /// Explain why a clause is failing using git history.
    Blame(BlameArgs),

    /// Binary search git history to find the breaking commit.
    Bisect(BisectArgs),

    /// Watch for file changes and re-run affected specs.
    Watch,

    /// Launch a visual spec viewer in the browser.
    View {
        /// Port to serve on.
        #[arg(long, default_value = "3333")]
        port: u16,

        /// Don't auto-open the browser.
        #[arg(long)]
        no_open: bool,
    },

    /// MCP server commands.
    Mcp(McpArgs),
}

#[derive(clap::Args)]
struct RunArgs {
    /// Spec file or glob pattern to run (default: all specs).
    path: Option<String>,

    /// Enable LLM-powered failure diagnosis.
    #[arg(long)]
    diagnose: bool,

    /// Enable LLM-powered test quality grading.
    #[arg(long)]
    grade: bool,

    /// Exit with code 1 on SHOULD failures too.
    #[arg(long)]
    fail_on_should: bool,
}

#[derive(clap::Args)]
struct GenerateArgs {
    /// Spec file or glob pattern to generate for (default: all specs).
    path: Option<String>,

    /// Regenerate all clauses regardless of hash.
    #[arg(long)]
    force: bool,

    /// Exit with code 1 if any generated tests are stale (for CI).
    #[arg(long)]
    check: bool,
}

#[derive(clap::Args)]
struct InspectArgs {
    /// Clause identifier (e.g. `auth::login::must_return_jwt`).
    clause: String,
}

#[derive(clap::Args)]
struct SurveyArgs {
    /// Source path to survey (default: project source roots).
    path: Option<PathBuf>,
}

#[derive(clap::Args)]
struct BlameArgs {
    /// Clause identifier to investigate.
    clause: String,
}

#[derive(clap::Args)]
struct BisectArgs {
    /// Clause identifier to bisect.
    clause: String,

    /// Limit search to a git revision range (e.g. `abc123..def456`).
    #[arg(long)]
    range: Option<String>,

    /// Regenerate tests at each commit instead of using current manifest.
    #[arg(long)]
    regenerate: bool,
}

#[derive(clap::Args)]
struct McpArgs {
    #[command(subcommand)]
    command: McpCommand,
}

#[derive(Subcommand)]
enum McpCommand {
    /// Start the MCP server.
    Serve {
        /// Transport protocol.
        #[arg(long, default_value = "stdio")]
        transport: String,

        /// Port for SSE transport.
        #[arg(long)]
        port: Option<u16>,

        /// Server mode: "standard" (default) or "generation" for agent-driven generation.
        #[arg(long, default_value = "standard")]
        mode: String,

        /// Path to assignment JSON file (required for generation mode).
        #[arg(long)]
        assignment: Option<PathBuf>,
    },

    /// Register with MCP-compatible coding agents.
    Install,
}

#[derive(Clone, clap::ValueEnum)]
enum ColorChoice {
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    fn to_report_color(&self) -> ReportColor {
        match self {
            ColorChoice::Auto => ReportColor::Auto,
            ColorChoice::Always => ReportColor::Always,
            ColorChoice::Never => ReportColor::Never,
        }
    }
}

// ─── Config + spec loading helpers ──────────────────────────────────────────

/// Load config, resolving from --config flag or auto-discovery.
fn load_config(config_path: &Option<PathBuf>) -> anyhow::Result<(PathBuf, Config)> {
    match config_path {
        Some(path) => {
            let config = Config::load(path)?;
            Ok((path.clone(), config))
        }
        None => Config::discover(),
    }
}

/// Load and parse all specs from config roots.
fn load_specs(config: &Config, config_path: &std::path::Path) -> anyhow::Result<SpecGraph> {
    let config_dir = config_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();

    let roots: Vec<PathBuf> = config
        .specs
        .roots
        .iter()
        .map(|r| config_dir.join(r))
        .collect();

    SpecGraph::from_roots(&roots).map_err(|errors| {
        let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        anyhow::anyhow!("spec parse errors:\n  {}", messages.join("\n  "))
    })
}

/// A section group ready for agent-based generation.
struct SectionGroup<'a> {
    section_path: String,
    testable_clauses: Vec<&'a ought_spec::Clause>,
    conditions: Vec<String>,
    /// Source paths from the spec's metadata (for the agent to read).
    source_paths: Vec<String>,
}

/// Collect section groups from all specs. Each section becomes one batch.
/// GIVEN clauses become conditions (context), not testable clauses.
/// OTHERWISE clauses are included with their parent.
fn collect_section_groups(specs: &SpecGraph) -> Vec<SectionGroup<'_>> {
    let mut groups = Vec::new();
    for spec in specs.specs() {
        let source_paths = spec.metadata.sources.clone();
        collect_groups_from_sections(&spec.sections, &spec.name, &source_paths, &mut groups);
    }
    groups
}

fn collect_groups_from_sections<'a>(
    sections: &'a [ought_spec::Section],
    parent_path: &str,
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
                    // GIVEN is context, not testable. Collect the condition text.
                    if let Some(ref cond) = clause.condition {
                        conditions.push(cond.clone());
                    } else {
                        conditions.push(clause.text.clone());
                    }
                    // But clauses *nested under* a GIVEN (which already have
                    // the condition attached) ARE testable — they'll appear
                    // as separate clauses with `condition` set by the parser.
                }
                _ => {
                    testable.push(clause);
                    // OTHERWISE children are part of the parent's test —
                    // the prompt builder handles them via clause.otherwise.
                    // Don't add them as separate testable clauses.
                }
            }
        }

        if !testable.is_empty() {
            groups.push(SectionGroup {
                section_path: section_path.clone(),
                testable_clauses: testable,
                conditions,
                source_paths: source_paths.to_vec(),
            });
        }

        // Recurse into subsections
        collect_groups_from_sections(&section.subsections, &section_path, source_paths, groups);
    }
}

/// Collect all testable clause IDs for manifest cleanup.
fn collect_all_testable_ids(specs: &SpecGraph) -> Vec<ought_spec::ClauseId> {
    let mut ids = Vec::new();
    for spec in specs.specs() {
        collect_ids_from_sections(&spec.sections, &mut ids);
    }
    ids
}

fn collect_ids_from_sections(sections: &[ought_spec::Section], ids: &mut Vec<ought_spec::ClauseId>) {
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
struct ClauseInfo {
    severity: ought_spec::Severity,
    otherwise_ids: Vec<String>,
}

/// Build a map from clause ID string to its info for exit-code decisions.
fn collect_all_clause_info(specs: &SpecGraph) -> std::collections::HashMap<String, ClauseInfo> {
    let mut map = std::collections::HashMap::new();
    for spec in specs.specs() {
        collect_clause_info_from_sections(&spec.sections, &mut map);
    }
    map
}

fn collect_clause_info_from_sections(
    sections: &[ought_spec::Section],
    map: &mut std::collections::HashMap<String, ClauseInfo>,
) {
    for section in sections {
        for clause in &section.clauses {
            let otherwise_ids: Vec<String> = clause.otherwise.iter().map(|ow| ow.id.0.clone()).collect();
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

// ─── Agent assignment builder ──────────────────────────────────────────────

/// Convert SectionGroups into AgentAssignments, partitioning across N agents round-robin.
/// Only includes stale clauses (unless force is true).
fn build_agent_assignments(
    groups: &[SectionGroup<'_>],
    manifest: &Manifest,
    config: &Config,
    config_path: &std::path::Path,
    test_dir: &std::path::Path,
    parallelism: usize,
    force: bool,
) -> Vec<ought_gen::AgentAssignment> {
    let project_root = config_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_string_lossy()
        .to_string();

    // Determine target language string.
    let target_language = config
        .runner
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "rust".to_string());

    // Collect stale groups with their assignment groups.
    let mut assignment_groups: Vec<ought_gen::AssignmentGroup> = Vec::new();

    for group in groups {
        let stale_clauses: Vec<&ought_spec::Clause> = group
            .testable_clauses
            .iter()
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

    // Partition groups round-robin across N agents.
    let n = parallelism.min(assignment_groups.len()).max(1);
    let mut buckets: Vec<Vec<ought_gen::AssignmentGroup>> = (0..n).map(|_| Vec::new()).collect();

    for (i, group) in assignment_groups.into_iter().enumerate() {
        buckets[i % n].push(group);
    }

    // Collect unique source paths from groups assigned to each agent.
    let source_paths_per_bucket: Vec<Vec<String>> = buckets
        .iter()
        .map(|_| {
            // We'll populate per-bucket below
            Vec::new()
        })
        .collect();

    // Actually, collect from the original groups before they were partitioned.
    // Simpler: collect all source paths from all groups into each assignment
    // (they're reading source for context, not writing to it).
    let all_source_paths: Vec<String> = groups
        .iter()
        .flat_map(|g| g.source_paths.iter().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let _ = source_paths_per_bucket;

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
fn clause_to_assignment_clause(clause: &ought_spec::Clause) -> ought_gen::AssignmentClause {
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

// ─── Command implementations ────────────────────────────────────────────────

fn cmd_init() -> anyhow::Result<()> {
    // Check if ought.toml already exists
    if std::path::Path::new("ought.toml").exists() {
        anyhow::bail!("ought.toml already exists in this directory");
    }

    // Detect project language
    let language = if std::path::Path::new("Cargo.toml").exists() {
        "rust"
    } else if std::path::Path::new("package.json").exists() {
        "typescript"
    } else if std::path::Path::new("pyproject.toml").exists()
        || std::path::Path::new("setup.py").exists()
    {
        "python"
    } else if std::path::Path::new("go.mod").exists() {
        "go"
    } else {
        "rust" // default
    };

    // Create ought/ directory
    std::fs::create_dir_all("ought")?;

    // Write ought.toml
    let config_content = format!(
        r#"[project]
name = "{name}"
version = "0.1.0"

[specs]
roots = ["ought/"]

[context]
search_paths = ["src/"]
exclude = ["target/", "ought/ought-gen/"]

[generator]
provider = "anthropic"

[runner.{lang}]
command = "{cmd}"
test_dir = "ought/ought-gen/"
"#,
        name = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "myproject".into()),
        lang = language,
        cmd = match language {
            "rust" => "cargo test",
            "python" => "pytest",
            "typescript" => "npx jest",
            "go" => "go test ./...",
            _ => "cargo test",
        },
    );
    std::fs::write("ought.toml", config_content)?;

    // Write example spec
    let example_spec = r#"# Example

context: Replace this with a description of what you're specifying.
source: src/

## Basic Behavior

- **MUST** do the most important thing correctly
- **MUST NOT** do the thing that would be bad
- **SHOULD** handle edge cases gracefully
- **MAY** support optional features
"#;
    std::fs::write("ought/example.ought.md", example_spec)?;

    eprintln!("Created ought.toml and ought/example.ought.md");
    eprintln!("Detected language: {}", language);
    Ok(())
}

fn cmd_run(cli: &Cli, args: &RunArgs) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;
    let specs = load_specs(&config, &config_path)?;

    // Find the first available runner
    let runner_name = config.runner.keys().next().cloned().unwrap_or("rust".into());
    let runner = runners::from_name(&runner_name)?;

    if !runner.is_available() {
        anyhow::bail!(
            "test runner '{}' is not available — is the toolchain installed?",
            runner.name()
        );
    }

    // Resolve test directory
    let config_dir = config_path.parent().unwrap_or(std::path::Path::new("."));
    let test_dir = config
        .runner
        .get(&runner_name)
        .map(|r| config_dir.join(&r.test_dir))
        .unwrap_or_else(|| config_dir.join("ought/ought-gen"));

    // Collect generated test files from the manifest
    let manifest_path = test_dir.join("manifest.toml");
    let _manifest = Manifest::load(&manifest_path).unwrap_or_default();

    // Build list of GeneratedTest from files in ought-gen
    let generated_tests = collect_generated_tests(&test_dir, &runner_name)?;

    if generated_tests.is_empty() {
        eprintln!("No generated tests found. Run `ought generate` first.");
        // Still write empty JUnit/JSON if requested
        let empty_results = ought_run::RunResult {
            results: vec![],
            total_duration: std::time::Duration::ZERO,
        };
        if let Some(junit_path) = &cli.junit {
            ought_report::junit::report(&empty_results, specs.specs(), junit_path)?;
        }
        if cli.json {
            let json = ought_report::json::report(&empty_results, specs.specs())?;
            println!("{}", json);
        }
        return Ok(());
    }

    // Execute tests
    let results = runner.run(&generated_tests, &test_dir)?;

    // Report
    let report_opts = ReportOptions {
        diagnose: args.diagnose,
        grade: args.grade,
        quiet: cli.quiet,
        color: cli.color.to_report_color(),
    };

    if cli.json {
        let json = ought_report::json::report(&results, specs.specs())?;
        println!("{}", json);
    } else {
        ought_report::terminal::report(&results, specs.specs(), &report_opts)?;
    }

    if let Some(junit_path) = &cli.junit {
        ought_report::junit::report(&results, specs.specs(), junit_path)?;
    }

    // Exit code logic: exit 1 if any Required-severity (MUST/MUST NOT) test
    // failed or errored. Also exit 1 if --fail-on-should and any SHOULD test failed.
    // A failed MUST clause is forgiven if an OTHERWISE clause in its chain passed
    // (graceful degradation).
    let clause_info = collect_all_clause_info(&specs);
    let result_map: std::collections::HashMap<&str, &ought_run::TestResult> = results
        .results
        .iter()
        .map(|r| (r.clause_id.0.as_str(), r))
        .collect();

    let has_hard_failure = results.results.iter().any(|r| {
        let is_failure =
            r.status == ought_run::TestStatus::Failed || r.status == ought_run::TestStatus::Errored;
        if !is_failure {
            return false;
        }
        // Look up clause info by ID; try exact match, then fuzzy suffix match.
        let info = clause_info
            .get(r.clause_id.0.as_str())
            .or_else(|| {
                let needle = r.clause_id.0.as_str();
                clause_info
                    .iter()
                    .find(|(k, _)| needle.ends_with(k.as_str()) || k.ends_with(needle))
                    .map(|(_, v)| v)
            });

        let severity = info
            .map(|i| i.severity)
            .unwrap_or(ought_spec::Severity::Required);

        // Check graceful degradation: if this clause has OTHERWISE children and
        // any of them passed, the failure is forgiven.
        if let Some(info) = info
            && !info.otherwise_ids.is_empty() {
                let otherwise_passed = info.otherwise_ids.iter().any(|ow_id| {
                    // Extract the last segment of the otherwise ID for fuzzy matching.
                    // Spec IDs may be flat (spec::section::otherwise_x) while test
                    // file paths nest under the parent (spec::section::parent::otherwise_x).
                    let ow_suffix = ow_id.rsplit("::").next().unwrap_or(ow_id.as_str());
                    result_map
                        .get(ow_id.as_str())
                        .or_else(|| {
                            result_map
                                .iter()
                                .find(|(k, _)| {
                                    // Match if the result key ends with the otherwise suffix
                                    k.ends_with(ow_suffix)
                                })
                                .map(|(_, v)| v)
                        })
                        .map(|tr| tr.status == ought_run::TestStatus::Passed)
                        .unwrap_or(false)
                });
                if otherwise_passed {
                    return false; // graceful degradation — not a hard failure
                }
            }

        match severity {
            ought_spec::Severity::Required => true,
            ought_spec::Severity::Recommended | ought_spec::Severity::Optional => {
                args.fail_on_should
            }
            ought_spec::Severity::NegativeConfirmation => false,
        }
    });

    if has_hard_failure {
        process::exit(1);
    }

    Ok(())
}

fn cmd_generate(cli: &Cli, args: &GenerateArgs) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;
    let specs = load_specs(&config, &config_path)?;

    let config_dir = config_path.parent().unwrap_or(std::path::Path::new("."));
    let test_dir = config
        .runner
        .values()
        .next()
        .map(|r| config_dir.join(&r.test_dir))
        .unwrap_or_else(|| config_dir.join("ought/ought-gen"));

    std::fs::create_dir_all(&test_dir)?;

    let manifest_path = test_dir.join("manifest.toml");
    let mut manifest = Manifest::load(&manifest_path).unwrap_or_default();

    let groups = collect_section_groups(&specs);

    let mut generated_count = 0;
    let mut error_count = 0;
    let mut stale_count = 0;

    if args.check {
        // In check mode, just count stale clauses.
        for group in &groups {
            for clause in &group.testable_clauses {
                if args.force || manifest.is_stale(&clause.id, &clause.content_hash, "") {
                    eprintln!("  stale: {}", clause.id);
                    stale_count += 1;
                }
            }
        }
    } else {
        // Agent mode: spawn LLM agents with MCP server connections.
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
            let total_clauses: usize =
                assignments.iter().map(|a| a.groups.iter().map(|g| g.clauses.len()).sum::<usize>()).sum();
            eprintln!(
                "{} assignments, {} clauses to generate",
                assignments.len(),
                total_clauses
            );

            let orchestrator = ought_gen::Orchestrator::new(&config, cli.verbose);
            let reports = orchestrator.run(assignments)?;

            for report in &reports {
                generated_count += report.generated;
                for err in &report.errors {
                    eprintln!("  error: {}", err);
                    error_count += 1;
                }
            }
        }
    }

    // Remove orphaned entries and save final manifest
    let all_ids = collect_all_testable_ids(&specs);
    let id_refs: Vec<&ought_spec::ClauseId> = all_ids.iter().collect();
    manifest.remove_orphans(&id_refs);

    // Save manifest
    manifest.save(&manifest_path)?;

    eprintln!(
        "\n{} generated, {} errors",
        generated_count, error_count
    );

    if args.check && stale_count > 0 {
        eprintln!("{} stale clauses", stale_count);
        process::exit(1);
    }

    Ok(())
}

fn cmd_check(cli: &Cli) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;

    match load_specs(&config, &config_path) {
        Ok(specs) => {
            let clause_count: usize = specs
                .specs()
                .iter()
                .map(|s| count_clauses_in_sections(&s.sections))
                .sum();
            eprintln!(
                "All specs valid: {} files, {} clauses",
                specs.specs().len(),
                clause_count
            );
            Ok(())
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}

fn count_clauses_in_sections(sections: &[ought_spec::Section]) -> usize {
    sections
        .iter()
        .map(|s| s.clauses.len() + count_clauses_in_sections(&s.subsections))
        .sum()
}

fn cmd_inspect(cli: &Cli, args: &InspectArgs) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;
    let config_dir = config_path.parent().unwrap_or(std::path::Path::new("."));
    let test_dir = config
        .runner
        .values()
        .next()
        .map(|r| config_dir.join(&r.test_dir))
        .unwrap_or_else(|| config_dir.join("ought/ought-gen"));

    // Try to find the clause in the specs to show its text
    if let Ok(specs) = load_specs(&config, &config_path) {
        // Try exact match first, then partial match on the clause ID
        let clause = find_clause_by_id(&specs, &args.clause)
            .or_else(|| find_clause_by_partial_id(&specs, &args.clause));
        if let Some(clause) = clause {
            println!("// Clause: {} {}", ought_gen::keyword_str(clause.keyword), clause.text);
            if let Some(ref cond) = clause.condition {
                println!("//   GIVEN: {}", cond);
            }
            println!();
        }
    }

    // Find a file matching the clause ID (try multiple extensions and naming conventions)
    let clause_path = args.clause.replace("::", "/");
    let candidates = [
        test_dir.join(format!("{}_test.rs", clause_path)),
        test_dir.join(format!("{}.rs", clause_path)),
        test_dir.join(format!("{}_test.py", clause_path)),
        test_dir.join(format!("{}.py", clause_path)),
        test_dir.join(format!("{}.test.ts", clause_path)),
        test_dir.join(format!("{}.ts", clause_path)),
        test_dir.join(format!("{}_test.go", clause_path)),
        test_dir.join(format!("{}.go", clause_path)),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            let content = std::fs::read_to_string(candidate)?;
            println!("{}", content);
            return Ok(());
        }
    }

    anyhow::bail!(
        "no generated test found for clause '{}'\nLooked in: {}",
        args.clause,
        test_dir.display()
    );
}

/// Find a clause by its ID string across all specs.
fn find_clause_by_id<'a>(
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
fn find_clause_by_partial_id<'a>(
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
    // Split search into path segments for fuzzy matching
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
/// Fuzzy match: all underscore-separated words in the search segment must appear in the ID segment.
fn clause_id_matches(clause_id: &str, search_parts: &[&str]) -> bool {
    let id_lower = clause_id.to_lowercase();
    let id_parts: Vec<&str> = id_lower.split("::").collect();

    if search_parts.len() > id_parts.len() {
        return false;
    }

    let offset = id_parts.len().saturating_sub(search_parts.len());
    for (i, search_part) in search_parts.iter().enumerate() {
        let id_part = id_parts.get(offset + i).unwrap_or(&"");
        // All words in the search segment must appear in the ID segment
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

/// Collect GeneratedTest structs from files in the ought-gen directory.
fn collect_generated_tests(
    test_dir: &std::path::Path,
    _runner_name: &str,
) -> anyhow::Result<Vec<ought_gen::GeneratedTest>> {
    let mut tests = Vec::new();

    if !test_dir.exists() {
        return Ok(tests);
    }

    fn walk(
        dir: &std::path::Path,
        root: &std::path::Path,
        tests: &mut Vec<ought_gen::GeneratedTest>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, root, tests);
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let language = match ext {
                        "rs" => ought_gen::generator::Language::Rust,
                        "py" => ought_gen::generator::Language::Python,
                        "ts" => ought_gen::generator::Language::TypeScript,
                        "js" => ought_gen::generator::Language::JavaScript,
                        "go" => ought_gen::generator::Language::Go,
                        _ => continue,
                    };

                    // Derive clause ID from the relative path within the test dir.
                    // e.g. for test_dir/spec/section/must_do_something_test.rs
                    // we get clause ID "spec::section::must_do_something".
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(&path);
                    let stem = rel
                        .with_extension("")
                        .to_string_lossy()
                        .to_string();
                    // Strip trailing _test suffix if present
                    let stem = stem
                        .strip_suffix("_test")
                        .unwrap_or(&stem)
                        .to_string();
                    // Convert path separators and double-underscores to ::
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

    walk(test_dir, test_dir, &mut tests);
    Ok(tests)
}

fn cmd_diff(cli: &Cli) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;
    let specs = load_specs(&config, &config_path)?;

    let config_dir = config_path.parent().unwrap_or(std::path::Path::new("."));
    let test_dir = config
        .runner
        .values()
        .next()
        .map(|r| config_dir.join(&r.test_dir))
        .unwrap_or_else(|| config_dir.join("ought/ought-gen"));

    let manifest_path = test_dir.join("manifest.toml");
    let manifest = Manifest::load(&manifest_path).unwrap_or_default();

    // Collect stale clauses grouped by spec file.
    struct StaleClause {
        id: String,
        keyword: ought_spec::Keyword,
        text: String,
        reason: String,
    }

    struct SpecDiff {
        spec_file: String,
        stale: Vec<StaleClause>,
        total: usize,
    }

    let mut diffs: Vec<SpecDiff> = Vec::new();

    for spec in specs.specs() {
        let spec_file = spec
            .source_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| spec.name.clone());

        let mut stale_clauses = Vec::new();
        let mut total = 0;

        fn collect_stale(
            sections: &[ought_spec::Section],
            manifest: &Manifest,
            stale_clauses: &mut Vec<StaleClause>,
            total: &mut usize,
        ) {
            for section in sections {
                for clause in &section.clauses {
                    if clause.keyword == ought_spec::Keyword::Given {
                        continue;
                    }
                    *total += 1;
                    if manifest.is_stale(&clause.id, &clause.content_hash, "") {
                        let reason = match manifest.entries.get(&clause.id.0) {
                            Some(entry) => {
                                if entry.clause_hash != clause.content_hash {
                                    "clause changed".to_string()
                                } else {
                                    "source changed".to_string()
                                }
                            }
                            None => "new clause".to_string(),
                        };
                        stale_clauses.push(StaleClause {
                            id: clause.id.0.clone(),
                            keyword: clause.keyword,
                            text: clause.text.clone(),
                            reason,
                        });
                    }
                }
                collect_stale(&section.subsections, manifest, stale_clauses, total);
            }
        }

        collect_stale(&spec.sections, &manifest, &mut stale_clauses, &mut total);
        diffs.push(SpecDiff {
            spec_file,
            stale: stale_clauses,
            total,
        });
    }

    // Output in unified-diff-like format, grouped by spec file.
    let mut any_stale = false;
    for diff in &diffs {
        if diff.stale.is_empty() {
            continue;
        }
        any_stale = true;
        println!("--- {}", diff.spec_file);
        println!("+++ {} (pending)", diff.spec_file);
        println!("@@ {}/{} clauses stale @@", diff.stale.len(), diff.total);
        for sc in &diff.stale {
            let kw = ought_gen::keyword_str(sc.keyword);
            println!("  M {}  ({}, {} {})", sc.id, sc.reason, kw, sc.text);
        }
        println!();
    }

    if !any_stale {
        println!("All generated tests are up to date.");
    }

    Ok(())
}

fn cmd_watch(cli: &Cli) -> anyhow::Result<()> {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::Duration;

    let (config_path, config) = load_config(&cli.config)?;
    let config_dir = config_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();

    // Resolve directories to watch.
    let spec_roots: Vec<PathBuf> = config
        .specs
        .roots
        .iter()
        .map(|r| config_dir.join(r))
        .collect();

    let source_paths: Vec<PathBuf> = config
        .context
        .search_paths
        .iter()
        .map(|p| config_dir.join(p))
        .collect();

    // Run an initial cycle.
    fn run_cycle(cli: &Cli, config_path: &std::path::Path, config: &Config) {
        // Clear screen before printing.
        eprint!("\x1b[2J\x1b[H");

        let specs = match load_specs(config, config_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error loading specs: {}", e);
                return;
            }
        };

        // Print spec files being checked (before test execution, for early output).
        eprintln!(" ought watch: checking {} spec(s)...", specs.specs().len());
        for spec in specs.specs() {
            if let Some(name) = spec.source_path.file_name() {
                eprintln!("  {}", name.to_string_lossy());
            }
        }

        let runner_name = config.runner.keys().next().cloned().unwrap_or("rust".into());
        let runner = match runners::from_name(&runner_name) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error creating runner: {}", e);
                return;
            }
        };

        if !runner.is_available() {
            eprintln!("runner '{}' is not available", runner.name());
            return;
        }

        let config_dir = config_path
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let test_dir = config
            .runner
            .get(&runner_name)
            .map(|r| config_dir.join(&r.test_dir))
            .unwrap_or_else(|| config_dir.join("ought/ought-gen"));

        let generated_tests = match collect_generated_tests(&test_dir, &runner_name) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("error collecting tests: {}", e);
                return;
            }
        };

        if generated_tests.is_empty() {
            eprintln!("No generated tests found. Run `ought generate` first.");
            return;
        }

        let results = match runner.run(&generated_tests, &test_dir) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error running tests: {}", e);
                return;
            }
        };

        let report_opts = ReportOptions {
            diagnose: false,
            grade: false,
            quiet: cli.quiet,
            color: cli.color.to_report_color(),
        };

        if cli.json {
            if let Ok(json) = ought_report::json::report(&results, specs.specs()) {
                println!("{}", json);
            }
        } else {
            let _ = ought_report::terminal::report(&results, specs.specs(), &report_opts);
        }
    }

    eprintln!("ought watch: running initial cycle...");
    run_cycle(cli, &config_path, &config);

    // Set up the file watcher.
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            let dominated = matches!(
                event.kind,
                notify::EventKind::Modify(_) | notify::EventKind::Create(_) | notify::EventKind::Remove(_)
            );
            if dominated {
                let _ = tx.send(());
            }
        }
    })?;

    // Watch spec roots and source paths.
    for root in &spec_roots {
        if root.exists() {
            watcher.watch(root, RecursiveMode::Recursive)?;
        }
    }
    for path in &source_paths {
        let p: &std::path::Path = path.as_ref();
        if p.exists() {
            watcher.watch(p, RecursiveMode::Recursive)?;
        }
    }

    eprintln!("ought watch: watching for changes...");

    // Debounce loop: wait for events, debounce at 500ms (sliding window), then re-run.
    let debounce = Duration::from_millis(500);

    while let Ok(()) = rx.recv() {

        // Debounce with sliding window: each new event resets the timer.
        // This ensures rapid bursts are collapsed into a single cycle.
        loop {
            match rx.recv_timeout(debounce) {
                Ok(()) => {} // new event, reset the debounce timer
                Err(mpsc::RecvTimeoutError::Timeout) => break, // no events for 500ms, fire
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }

        // Drain any additional buffered events.
        while rx.try_recv().is_ok() {}

        // Reload config in case it changed.
        let config = match Config::load(&config_path) {
            Ok(c) => c,
            Err(e) => {
                eprint!("\x1b[2J\x1b[H");
                eprintln!("error reloading config: {}", e);
                continue;
            }
        };

        run_cycle(cli, &config_path, &config);
    }

    Ok(())
}

fn cmd_survey(cli: &Cli, args: &SurveyArgs) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;
    let specs = load_specs(&config, &config_path)?;

    let paths: Vec<PathBuf> = if let Some(ref path) = args.path {
        vec![path.clone()]
    } else {
        // Use source search paths from config.
        let config_dir = config_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        config
            .context
            .search_paths
            .iter()
            .map(|p| config_dir.join(p))
            .collect()
    };

    let result = ought_analysis::survey::survey(&specs, &paths)?;

    if cli.json {
        // Simple JSON output.
        println!("{{");
        println!("  \"uncovered\": [");
        for (i, item) in result.uncovered.iter().enumerate() {
            let comma = if i + 1 < result.uncovered.len() { "," } else { "" };
            println!(
                "    {{\"file\": {:?}, \"line\": {}, \"description\": {:?}, \"suggested_clause\": {:?}, \"suggested_spec\": {:?}}}{}",
                item.file.display().to_string(),
                item.line,
                item.description,
                item.suggested_clause,
                item.suggested_spec.display().to_string(),
                comma
            );
        }
        println!("  ]");
        println!("}}");
    } else {
        if result.uncovered.is_empty() {
            eprintln!("No uncovered behaviors found.");
        } else {
            eprintln!("Found {} uncovered behaviors:\n", result.uncovered.len());
            let mut current_spec: Option<&std::path::Path> = None;
            for item in &result.uncovered {
                if current_spec != Some(&item.suggested_spec) {
                    eprintln!("  \x1b[1m{}\x1b[0m", item.suggested_spec.display());
                    current_spec = Some(&item.suggested_spec);
                }
                eprintln!(
                    "    {}:{} - {}",
                    item.file.display(),
                    item.line,
                    item.description
                );
                eprintln!(
                    "      Suggested: \x1b[33m{}\x1b[0m",
                    item.suggested_clause
                );
            }
        }
    }

    Ok(())
}

fn cmd_audit(cli: &Cli) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;
    let specs = load_specs(&config, &config_path)?;

    let result = ought_analysis::audit::audit(&specs)?;

    if cli.json {
        println!("{{");
        println!("  \"findings\": [");
        for (i, finding) in result.findings.iter().enumerate() {
            let comma = if i + 1 < result.findings.len() { "," } else { "" };
            let kind = match finding.kind {
                ought_analysis::AuditFindingKind::Contradiction => "contradiction",
                ought_analysis::AuditFindingKind::Gap => "gap",
                ought_analysis::AuditFindingKind::Ambiguity => "ambiguity",
                ought_analysis::AuditFindingKind::Redundancy => "redundancy",
            };
            let clauses_json: Vec<String> = finding
                .clauses
                .iter()
                .map(|c| format!("{:?}", c.0))
                .collect();
            println!(
                "    {{\"kind\": {:?}, \"description\": {:?}, \"clauses\": [{}], \"suggestion\": {:?}, \"confidence\": {:?}}}{}",
                kind,
                finding.description,
                clauses_json.join(", "),
                finding.suggestion,
                finding.confidence,
                comma
            );
        }
        println!("  ]");
        println!("}}");
    } else {
        if result.findings.is_empty() {
            eprintln!("No issues found. Specs are coherent.");
        } else {
            eprintln!("Found {} issues:\n", result.findings.len());
            for finding in &result.findings {
                let kind_str = match finding.kind {
                    ought_analysis::AuditFindingKind::Contradiction => "\x1b[31mCONTRADICTION\x1b[0m",
                    ought_analysis::AuditFindingKind::Gap => "\x1b[33mGAP\x1b[0m",
                    ought_analysis::AuditFindingKind::Ambiguity => "\x1b[34mAMBIGUITY\x1b[0m",
                    ought_analysis::AuditFindingKind::Redundancy => "\x1b[36mREDUNDANCY\x1b[0m",
                };
                eprintln!("  [{}] {}", kind_str, finding.description);
                if !finding.clauses.is_empty() {
                    eprintln!(
                        "    Clauses: {}",
                        finding
                            .clauses
                            .iter()
                            .map(|c| c.0.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                if let Some(ref suggestion) = finding.suggestion {
                    eprintln!("    Suggestion: {}", suggestion);
                }
                if let Some(confidence) = finding.confidence {
                    eprintln!("    Confidence: {:.0}%", confidence * 100.0);
                }
                eprintln!();
            }
        }
    }

    Ok(())
}

fn cmd_blame(cli: &Cli, args: &BlameArgs) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;
    let specs = load_specs(&config, &config_path)?;

    // We need run results. Run the tests first.
    let runner_name = config.runner.keys().next().cloned().unwrap_or("rust".into());
    let runner = runners::from_name(&runner_name)?;

    let config_dir = config_path.parent().unwrap_or(std::path::Path::new("."));
    let test_dir = config
        .runner
        .get(&runner_name)
        .map(|r| config_dir.join(&r.test_dir))
        .unwrap_or_else(|| config_dir.join("ought/ought-gen"));

    let generated_tests = collect_generated_tests(&test_dir, &runner_name)?;
    let results = if !generated_tests.is_empty() && runner.is_available() {
        runner.run(&generated_tests, &test_dir)?
    } else {
        ought_run::RunResult {
            results: vec![],
            total_duration: std::time::Duration::ZERO,
        }
    };

    let clause_id = ought_spec::ClauseId(args.clause.clone());
    let result = ought_analysis::blame::blame(&clause_id, &specs, &results)?;

    if cli.json {
        let commit_json = if let Some(ref c) = result.likely_commit {
            format!(
                "{{\"hash\": {:?}, \"message\": {:?}, \"author\": {:?}}}",
                c.hash, c.message, c.author
            )
        } else {
            "null".to_string()
        };
        println!(
            "{{\"clause_id\": {:?}, \"narrative\": {:?}, \"likely_commit\": {}, \"suggested_fix\": {:?}}}",
            result.clause_id.0,
            result.narrative,
            commit_json,
            result.suggested_fix
        );
    } else {
        eprintln!("\x1b[1mBlame: {}\x1b[0m\n", result.clause_id);
        eprintln!("{}", result.narrative);
        if let Some(ref commit) = result.likely_commit {
            eprintln!(
                "\nLikely commit: \x1b[33m{}\x1b[0m {} ({})",
                &commit.hash[..7.min(commit.hash.len())],
                commit.message,
                commit.author
            );
        }
        if let Some(ref fix) = result.suggested_fix {
            eprintln!("\nSuggested fix: {}", fix);
        }
    }

    Ok(())
}

fn cmd_bisect(cli: &Cli, args: &BisectArgs) -> anyhow::Result<()> {
    let (config_path, config) = load_config(&cli.config)?;
    let specs = load_specs(&config, &config_path)?;

    let runner_name = config.runner.keys().next().cloned().unwrap_or("rust".into());
    let runner = runners::from_name(&runner_name)?;

    if !runner.is_available() {
        anyhow::bail!(
            "test runner '{}' is not available -- is the toolchain installed?",
            runner.name()
        );
    }

    let clause_id = ought_spec::ClauseId(args.clause.clone());
    let options = ought_analysis::bisect::BisectOptions {
        range: args.range.clone(),
        regenerate: args.regenerate,
    };

    eprintln!("Bisecting clause {}...", clause_id);

    let result = ought_analysis::bisect::bisect(&clause_id, &specs, runner.as_ref(), &options)?;

    if cli.json {
        println!(
            "{{\"clause_id\": {:?}, \"breaking_commit\": {{\"hash\": {:?}, \"message\": {:?}, \"author\": {:?}}}, \"diff_summary\": {:?}}}",
            result.clause_id.0,
            result.breaking_commit.hash,
            result.breaking_commit.message,
            result.breaking_commit.author,
            result.diff_summary
        );
    } else {
        eprintln!("\x1b[1mBisect result for {}\x1b[0m\n", result.clause_id);
        eprintln!(
            "Breaking commit: \x1b[31m{}\x1b[0m",
            &result.breaking_commit.hash[..7.min(result.breaking_commit.hash.len())]
        );
        eprintln!("  Message: {}", result.breaking_commit.message);
        eprintln!("  Author:  {}", result.breaking_commit.author);
        eprintln!("  Date:    {}", result.breaking_commit.date);
        if !result.diff_summary.is_empty() {
            eprintln!("\nDiff summary:\n{}", result.diff_summary);
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Init => cmd_init(),
        Command::Run(args) => cmd_run(&cli, args),
        Command::Generate(args) => cmd_generate(&cli, args),
        Command::Check => cmd_check(&cli),
        Command::Inspect(args) => cmd_inspect(&cli, args),
        Command::Diff => cmd_diff(&cli),
        Command::Survey(args) => cmd_survey(&cli, args),
        Command::Audit => cmd_audit(&cli),
        Command::Blame(args) => cmd_blame(&cli, args),
        Command::Bisect(args) => cmd_bisect(&cli, args),
        Command::Watch => cmd_watch(&cli),
        Command::View { port, no_open } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(ought_server::serve(cli.config.as_deref(), *port, !*no_open))
        }
        Command::Mcp(args) => match &args.command {
            McpCommand::Serve {
                transport: _,
                port: _,
                mode,
                assignment,
            } => {
                if mode == "generation" {
                    let assignment_path = assignment
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("--assignment is required for generation mode"))?;
                    let server =
                        ought_mcp::gen_server::GenMcpServer::from_assignment_path(assignment_path)?;
                    tokio::runtime::Runtime::new()?.block_on(server.serve_stdio())
                } else {
                    let (config_path, _config) = load_config(&cli.config)?;
                    let server = ought_mcp::server::McpServer::new(config_path);
                    tokio::runtime::Runtime::new()?.block_on(
                        server.serve(ought_mcp::server::Transport::Stdio),
                    )
                }
            }
            McpCommand::Install => {
                ought_mcp::server::McpServer::install()?;
                eprintln!("Registered ought with MCP-compatible coding agents.");
                Ok(())
            }
        },
    }
}
