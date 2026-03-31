use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

use ought_gen::manifest::Manifest;
use ought_gen::providers;
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

/// A section group ready for batch generation.
struct SectionGroup<'a> {
    spec: &'a ought_spec::Spec,
    section_path: String,
    testable_clauses: Vec<&'a ought_spec::Clause>,
    conditions: Vec<String>,
}

/// Collect section groups from all specs. Each section becomes one batch.
/// GIVEN clauses become conditions (context), not testable clauses.
/// OTHERWISE clauses are included with their parent.
fn collect_section_groups(specs: &SpecGraph) -> Vec<SectionGroup<'_>> {
    let mut groups = Vec::new();
    for spec in specs.specs() {
        collect_groups_from_sections(spec, &spec.sections, &spec.name, &mut groups);
    }
    groups
}

fn collect_groups_from_sections<'a>(
    spec: &'a ought_spec::Spec,
    sections: &'a [ought_spec::Section],
    parent_path: &str,
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
                spec,
                section_path: section_path.clone(),
                testable_clauses: testable,
                conditions,
            });
        }

        // Recurse into subsections
        collect_groups_from_sections(spec, &section.subsections, &section_path, groups);
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

    let generator = providers::from_config(
        &config.generator.provider,
        config.generator.model.as_deref(),
    )?;

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

    let assembler = ought_gen::ContextAssembler::new(&config);
    let groups = collect_section_groups(&specs);

    let mut generated_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;
    let mut stale_count = 0;

    for group in &groups {
        // Filter to only stale clauses in this group
        let stale_clauses: Vec<&ought_spec::Clause> = group
            .testable_clauses
            .iter()
            .filter(|c| {
                if args.force {
                    true
                } else {
                    manifest.is_stale(&c.id, &c.content_hash, "")
                }
            })
            .copied()
            .collect();

        let fresh_count = group.testable_clauses.len() - stale_clauses.len();
        skipped_count += fresh_count;

        if stale_clauses.is_empty() {
            continue;
        }

        if args.check {
            for clause in &stale_clauses {
                eprintln!("  stale: {}", clause.id);
            }
            stale_count += stale_clauses.len();
            continue;
        }

        // Build the batch group
        let batch = ought_gen::ClauseGroup {
            section_path: group.section_path.clone(),
            clauses: stale_clauses.clone(),
            conditions: group.conditions.clone(),
        };

        let clause_count = batch.clauses.len();

        // Print section header
        eprintln!();
        eprintln!(
            "  \x1b[1m{}\x1b[0m ({} clauses)",
            group.section_path, clause_count
        );

        // In verbose mode, list the clauses being generated
        if cli.verbose {
            for clause in &stale_clauses {
                eprintln!(
                    "    \x1b[2m{} {}\x1b[0m",
                    ought_gen::providers::keyword_str(clause.keyword),
                    clause.text,
                );
            }
            if !group.conditions.is_empty() {
                for cond in &group.conditions {
                    eprintln!("    \x1b[2mGIVEN: {}\x1b[0m", cond);
                }
            }
        }

        // Assemble context from the first clause (they share a section/spec)
        let mut context = assembler
            .assemble(stale_clauses[0], group.spec)
            .unwrap_or_else(|_| ought_gen::context::GenerationContext {
                spec_context: group.spec.metadata.context.clone(),
                source_files: vec![],
                schema_files: vec![],
                target_language: ought_gen::generator::Language::Rust,
                verbose: false,
            });
        context.verbose = cli.verbose;

        if cli.verbose && !context.source_files.is_empty() {
            eprintln!("    \x1b[2mcontext: {} source files\x1b[0m", context.source_files.len());
        }

        match generator.generate_batch(&batch, &context) {
            Ok(tests) => {
                for test in &tests {
                    let file_path = test_dir.join(&test.file_path);
                    if let Some(parent) = file_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&file_path, &test.code)?;

                    manifest.entries.insert(
                        test.clause_id.0.clone(),
                        ought_gen::manifest::ManifestEntry {
                            clause_hash: stale_clauses
                                .iter()
                                .find(|c| c.id == test.clause_id)
                                .map(|c| c.content_hash.clone())
                                .unwrap_or_default(),
                            source_hash: String::new(),
                            generated_at: chrono::Utc::now(),
                            model: config
                                .generator
                                .model
                                .clone()
                                .unwrap_or_else(|| "default".into()),
                        },
                    );
                }
                eprintln!(
                    "  \x1b[32m\u{2713}\x1b[0m {} tests generated",
                    tests.len()
                );
                if cli.verbose {
                    for test in &tests {
                        eprintln!(
                            "    \x1b[2mwrote {}\x1b[0m",
                            test.file_path.display()
                        );
                    }
                }
                generated_count += tests.len();

                // Save manifest after each batch so ctrl+c doesn't lose progress
                manifest.save(&manifest_path)?;
            }
            Err(e) => {
                eprintln!("  \x1b[31m\u{2717}\x1b[0m error: {}", e);
                error_count += clause_count;
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
        "\n{} generated, {} up-to-date, {} errors",
        generated_count, skipped_count, error_count
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
            println!("// Clause: {} {}", ought_gen::providers::keyword_str(clause.keyword), clause.text);
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
            let kw = ought_gen::providers::keyword_str(sc.keyword);
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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Init => cmd_init(),
        Command::Run(args) => cmd_run(&cli, args),
        Command::Generate(args) => cmd_generate(&cli, args),
        Command::Check => cmd_check(&cli),
        Command::Inspect(args) => cmd_inspect(&cli, args),
        Command::Diff => cmd_diff(&cli),
        Command::Survey(_args) => {
            eprintln!("ought survey is not yet implemented");
            Ok(())
        }
        Command::Audit => {
            eprintln!("ought audit is not yet implemented");
            Ok(())
        }
        Command::Blame(_args) => {
            eprintln!("ought blame is not yet implemented");
            Ok(())
        }
        Command::Bisect(_args) => {
            eprintln!("ought bisect is not yet implemented");
            Ok(())
        }
        Command::Watch => cmd_watch(&cli),
        Command::Mcp(args) => match &args.command {
            McpCommand::Serve {
                transport: _,
                port: _,
            } => {
                eprintln!("ought mcp serve is not yet implemented");
                Ok(())
            }
            McpCommand::Install => {
                eprintln!("ought mcp install is not yet implemented");
                Ok(())
            }
        },
    }
}
