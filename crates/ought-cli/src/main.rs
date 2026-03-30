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

/// Collect all clauses from all specs, flattened.
fn collect_clauses(specs: &SpecGraph) -> Vec<&ought_spec::Clause> {
    let mut clauses = Vec::new();
    for spec in specs.specs() {
        collect_clauses_from_sections(&spec.sections, &mut clauses);
    }
    clauses
}

fn collect_clauses_from_sections<'a>(
    sections: &'a [ought_spec::Section],
    out: &mut Vec<&'a ought_spec::Clause>,
) {
    for section in sections {
        for clause in &section.clauses {
            out.push(clause);
            collect_otherwise_clauses(clause, out);
        }
        collect_clauses_from_sections(&section.subsections, out);
    }
}

fn collect_otherwise_clauses<'a>(
    clause: &'a ought_spec::Clause,
    out: &mut Vec<&'a ought_spec::Clause>,
) {
    for ow in &clause.otherwise {
        out.push(ow);
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

    // Exit code logic
    let has_must_failures = results.results.iter().any(|r| {
        r.status == ought_run::TestStatus::Failed
    });

    if has_must_failures {
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
    let clauses = collect_clauses(&specs);
    let all_specs = specs.specs();

    let mut generated_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;

    for clause in &clauses {
        // Skip non-testable keywords
        if clause.keyword == ought_spec::Keyword::Given {
            continue;
        }

        let clause_key = clause.id.0.clone();

        // Check if stale
        if !args.force && !manifest.is_stale(&clause.id, &clause.content_hash, "") {
            if args.check {
                // In check mode, just count
            }
            skipped_count += 1;
            continue;
        }

        if args.check {
            // Check mode: just report that it's stale
            eprintln!("  stale: {}", clause.id);
            error_count += 1;
            continue;
        }

        // Find the spec this clause belongs to
        let spec = all_specs
            .iter()
            .find(|s| {
                s.sections
                    .iter()
                    .any(|sec| sec.clauses.iter().any(|c| c.id == clause.id))
            })
            .unwrap_or(&all_specs[0]);

        eprint!("  generating: {} ... ", clause.id);

        let context = assembler.assemble(clause, spec).unwrap_or_else(|_| {
            ought_gen::context::GenerationContext {
                spec_context: spec.metadata.context.clone(),
                source_files: vec![],
                schema_files: vec![],
                target_language: ought_gen::generator::Language::Rust,
            }
        });

        match generator.generate(clause, &context) {
            Ok(test) => {
                // Write the test file
                let file_path = test_dir.join(&test.file_path);
                if let Some(parent) = file_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&file_path, &test.code)?;

                // Update manifest
                manifest.entries.insert(
                    clause_key,
                    ought_gen::manifest::ManifestEntry {
                        clause_hash: clause.content_hash.clone(),
                        source_hash: String::new(),
                        generated_at: chrono::Utc::now(),
                        model: config
                            .generator
                            .model
                            .clone()
                            .unwrap_or_else(|| "default".into()),
                    },
                );

                eprintln!("ok");
                generated_count += 1;
            }
            Err(e) => {
                eprintln!("error: {}", e);
                error_count += 1;
            }
        }
    }

    // Remove orphaned entries
    let valid_ids: Vec<&ought_spec::ClauseId> = clauses.iter().map(|c| &c.id).collect();
    manifest.remove_orphans(&valid_ids);

    // Save manifest
    manifest.save(&manifest_path)?;

    eprintln!(
        "\n{} generated, {} skipped, {} errors",
        generated_count, skipped_count, error_count
    );

    if args.check && error_count > 0 {
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

    // Find a file matching the clause ID
    let clause_path = args.clause.replace("::", "/");
    let candidates = [
        test_dir.join(format!("{}.rs", clause_path)),
        test_dir.join(format!("{}.py", clause_path)),
        test_dir.join(format!("{}.ts", clause_path)),
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

/// Collect GeneratedTest structs from files in the ought-gen directory.
fn collect_generated_tests(
    test_dir: &std::path::Path,
    _runner_name: &str,
) -> anyhow::Result<Vec<ought_gen::GeneratedTest>> {
    let mut tests = Vec::new();

    if !test_dir.exists() {
        return Ok(tests);
    }

    fn walk(dir: &std::path::Path, tests: &mut Vec<ought_gen::GeneratedTest>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, tests);
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let language = match ext {
                        "rs" => ought_gen::generator::Language::Rust,
                        "py" => ought_gen::generator::Language::Python,
                        "ts" => ought_gen::generator::Language::TypeScript,
                        "js" => ought_gen::generator::Language::JavaScript,
                        "go" => ought_gen::generator::Language::Go,
                        _ => continue,
                    };

                    // Derive clause ID from path
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    let clause_id = ought_spec::ClauseId(stem.replace("__", "::"));

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

    walk(test_dir, &mut tests);
    Ok(tests)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Init => cmd_init(),
        Command::Run(args) => cmd_run(&cli, args),
        Command::Generate(args) => cmd_generate(&cli, args),
        Command::Check => cmd_check(&cli),
        Command::Inspect(args) => cmd_inspect(&cli, args),
        Command::Diff => {
            eprintln!("ought diff is not yet implemented");
            Ok(())
        }
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
        Command::Watch => {
            eprintln!("ought watch is not yet implemented");
            Ok(())
        }
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
