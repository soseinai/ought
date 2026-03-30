pub mod claude;
pub mod custom;
pub mod ollama;
pub mod openai;

use std::fmt::Write as _;
use std::path::PathBuf;

use ought_spec::Clause;

use crate::context::GenerationContext;
use crate::generator::{ClauseGroup, GeneratedTest, Generator, Language};

/// Strip markdown code fences from LLM output.
/// LLMs frequently wrap code in ```lang ... ``` despite being told not to.
pub fn strip_markdown_fences(code: &str) -> String {
    let trimmed = code.trim();

    // Check if it starts with a fence like ```rust, ```python, ``` etc.
    if let Some(rest) = trimmed.strip_prefix("```") {
        // Skip the language identifier on the first line
        let after_lang = if let Some(idx) = rest.find('\n') {
            &rest[idx + 1..]
        } else {
            rest
        };
        // Strip trailing fence
        let stripped = if let Some(body) = after_lang.strip_suffix("```") {
            body
        } else {
            after_lang
        };
        stripped.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

/// Create a generator from the provider name in config.
pub fn from_config(provider: &str, model: Option<&str>) -> anyhow::Result<Box<dyn Generator>> {
    match provider.to_lowercase().as_str() {
        "anthropic" | "claude" => {
            Ok(Box::new(claude::ClaudeGenerator::new(model.map(String::from))))
        }
        "openai" | "chatgpt" => {
            Ok(Box::new(openai::OpenAiGenerator::new(model.map(String::from))))
        }
        "ollama" => {
            let model = model
                .map(String::from)
                .unwrap_or_else(|| "llama3".to_string());
            Ok(Box::new(ollama::OllamaGenerator::new(model)))
        }
        other => {
            // Try as a custom executable path
            let path = PathBuf::from(other);
            Ok(Box::new(custom::CustomGenerator::new(path)))
        }
    }
}

/// Build a prompt for the LLM from a clause and its generation context.
pub fn build_prompt(clause: &Clause, context: &GenerationContext) -> String {
    let mut prompt = String::new();

    // Instructions
    let lang_name = language_name(context.target_language);
    let _ = writeln!(
        prompt,
        "You are a test generation assistant. Generate a single, self-contained {lang_name} test \
         function for the following specification clause. Output ONLY the test code with no \
         explanation, no markdown fences, and no surrounding text."
    );
    prompt.push('\n');

    // Clause details
    let _ = writeln!(prompt, "## Clause");
    let _ = writeln!(prompt, "- Keyword: {}", keyword_str(clause.keyword));
    let _ = writeln!(prompt, "- Severity: {:?}", clause.severity);
    let _ = writeln!(prompt, "- ID: {}", clause.id);
    let _ = writeln!(prompt, "- Text: {}", clause.text);

    if let Some(ref condition) = clause.condition {
        let _ = writeln!(prompt, "- GIVEN condition: {condition}");
    }

    if let Some(ref temporal) = clause.temporal {
        match temporal {
            ought_spec::Temporal::Invariant => {
                let _ = writeln!(
                    prompt,
                    "- Temporal: MUST ALWAYS (invariant). Generate property-based or fuzz-style tests."
                );
            }
            ought_spec::Temporal::Deadline(dur) => {
                let _ = writeln!(
                    prompt,
                    "- Temporal: MUST BY {:?}. Generate a test asserting the operation completes within this duration.",
                    dur
                );
            }
        }
    }

    if !clause.otherwise.is_empty() {
        let _ = writeln!(prompt, "- This clause has OTHERWISE fallbacks.");
    }

    prompt.push('\n');

    // Hints (code blocks from the spec)
    if !clause.hints.is_empty() {
        let _ = writeln!(prompt, "## Hints");
        for hint in &clause.hints {
            let _ = writeln!(prompt, "```\n{hint}\n```");
        }
        prompt.push('\n');
    }

    // Spec-level context
    if let Some(ref ctx) = context.spec_context {
        let _ = writeln!(prompt, "## Context\n{ctx}\n");
    }

    // Source code
    if !context.source_files.is_empty() {
        let _ = writeln!(prompt, "## Source Code");
        for sf in &context.source_files {
            let _ = writeln!(prompt, "### File: {}", sf.path.display());
            let _ = writeln!(prompt, "```\n{}\n```\n", sf.content);
        }
    }

    // Schema files
    if !context.schema_files.is_empty() {
        let _ = writeln!(prompt, "## Schema Files");
        for sf in &context.schema_files {
            let _ = writeln!(prompt, "### File: {}", sf.path.display());
            let _ = writeln!(prompt, "```\n{}\n```\n", sf.content);
        }
    }

    // Output instructions
    let _ = writeln!(prompt, "## Requirements");
    let _ = writeln!(
        prompt,
        "- Include the original clause text as a doc comment on the test function."
    );
    let _ = writeln!(
        prompt,
        "- The test function name should be derived from the clause ID: {}",
        clause.id
    );
    let _ = writeln!(
        prompt,
        "- The test must be self-contained with no cross-test dependencies."
    );

    if clause.keyword == ought_spec::Keyword::Wont {
        let _ = writeln!(
            prompt,
            "- This is a WONT clause: generate an absence test (verify the capability does not exist) \
             or a prevention test (verify that attempting the behavior fails gracefully)."
        );
    }

    match context.target_language {
        Language::Rust => {
            let _ = writeln!(prompt, "- Use #[test] attribute and assert! macros.");
        }
        Language::Python => {
            let _ = writeln!(prompt, "- Use def test_... function with assert statements.");
        }
        Language::TypeScript | Language::JavaScript => {
            let _ = writeln!(
                prompt,
                "- Use test() or it() with expect() assertions (Jest style)."
            );
        }
        Language::Go => {
            let _ = writeln!(
                prompt,
                "- Use func Test...(t *testing.T) with t.Error/t.Fatal."
            );
        }
    }

    prompt
}

/// The marker used to separate test functions in batch output.
pub const BATCH_MARKER: &str = "// === CLAUSE:";

/// Build a prompt for a batch of clauses from the same section.
/// Asks the LLM to output all test functions separated by marker comments.
pub fn build_batch_prompt(group: &ClauseGroup<'_>, context: &GenerationContext) -> String {
    let mut prompt = String::new();
    let lang_name = language_name(context.target_language);

    // Instructions
    let _ = writeln!(
        prompt,
        "You are a test generation assistant. Generate self-contained {lang_name} test functions \
         for the following specification clauses. These clauses belong to the same section and \
         should be understood together.\n\
         \n\
         Output ONLY the test code with no explanation and no markdown fences.\n\
         \n\
         IMPORTANT: Separate each test function with a marker comment on its own line:\n\
         {BATCH_MARKER} <clause_id> ===\n\
         \n\
         For example:\n\
         {BATCH_MARKER} auth::login::must_return_jwt ===\n\
         #[test]\n\
         fn test_auth__login__must_return_jwt() {{ ... }}\n\
         \n\
         {BATCH_MARKER} auth::login::must_return_401 ===\n\
         #[test]\n\
         fn test_auth__login__must_return_401() {{ ... }}"
    );
    prompt.push('\n');

    // Section context
    let _ = writeln!(prompt, "## Section: {}", group.section_path);
    prompt.push('\n');

    // GIVEN conditions as context
    if !group.conditions.is_empty() {
        let _ = writeln!(prompt, "## Preconditions (GIVEN)");
        let _ = writeln!(
            prompt,
            "The following conditions apply to clauses in this section. \
             Use them to set up test preconditions:"
        );
        for cond in &group.conditions {
            let _ = writeln!(prompt, "- GIVEN {cond}");
        }
        prompt.push('\n');
    }

    // List all clauses
    let _ = writeln!(prompt, "## Clauses to test ({} total)", group.clauses.len());
    for clause in &group.clauses {
        let _ = write!(prompt, "\n### {} {}", keyword_str(clause.keyword), clause.text);
        let _ = writeln!(prompt, "  (ID: {})", clause.id);

        if let Some(ref condition) = clause.condition {
            let _ = writeln!(prompt, "  GIVEN: {condition}");
        }
        if let Some(ref temporal) = clause.temporal {
            match temporal {
                ought_spec::Temporal::Invariant => {
                    let _ = writeln!(prompt, "  Temporal: MUST ALWAYS (invariant). Generate property-based or fuzz-style tests.");
                }
                ought_spec::Temporal::Deadline(dur) => {
                    let _ = writeln!(prompt, "  Temporal: MUST BY {dur:?}. Assert operation completes within this duration.");
                }
            }
        }
        if !clause.otherwise.is_empty() {
            for ow in &clause.otherwise {
                let _ = writeln!(prompt, "  OTHERWISE: {} (ID: {})", ow.text, ow.id);
            }
        }
        if clause.keyword == ought_spec::Keyword::Wont {
            let _ = writeln!(prompt, "  (WONT: generate an absence or prevention test)");
        }
        if !clause.hints.is_empty() {
            for hint in &clause.hints {
                let _ = writeln!(prompt, "  Hint:\n  ```\n  {hint}\n  ```");
            }
        }
    }
    prompt.push('\n');

    // Spec-level context
    if let Some(ref ctx) = context.spec_context {
        let _ = writeln!(prompt, "## Context\n{ctx}\n");
    }

    // Source code
    if !context.source_files.is_empty() {
        let _ = writeln!(prompt, "## Source Code");
        for sf in &context.source_files {
            let _ = writeln!(prompt, "### File: {}", sf.path.display());
            let _ = writeln!(prompt, "```\n{}\n```\n", sf.content);
        }
    }

    // Schema files
    if !context.schema_files.is_empty() {
        let _ = writeln!(prompt, "## Schema Files");
        for sf in &context.schema_files {
            let _ = writeln!(prompt, "### File: {}", sf.path.display());
            let _ = writeln!(prompt, "```\n{}\n```\n", sf.content);
        }
    }

    // Output requirements
    let _ = writeln!(prompt, "## Requirements");
    let _ = writeln!(prompt, "- Include the original clause text as a doc comment on each test function.");
    let _ = writeln!(prompt, "- Each test must be self-contained with no cross-test dependencies.");
    let _ = writeln!(prompt, "- Separate each test function with: {BATCH_MARKER} <clause_id> ===");

    match context.target_language {
        Language::Rust => {
            let _ = writeln!(prompt, "- Use #[test] attribute and assert! macros.");
        }
        Language::Python => {
            let _ = writeln!(prompt, "- Use def test_... function with assert statements.");
        }
        Language::TypeScript | Language::JavaScript => {
            let _ = writeln!(prompt, "- Use test() or it() with expect() assertions (Jest style).");
        }
        Language::Go => {
            let _ = writeln!(prompt, "- Use func Test...(t *testing.T) with t.Error/t.Fatal.");
        }
    }

    prompt
}

/// Parse batch LLM output into individual test functions, keyed by clause ID.
/// Splits on `// === CLAUSE: <id> ===` markers.
pub fn parse_batch_response(
    response: &str,
    group: &ClauseGroup<'_>,
    language: Language,
) -> Vec<GeneratedTest> {
    let mut tests = Vec::new();
    let mut current_id: Option<String> = None;
    let mut current_code = String::new();

    for line in response.lines() {
        if let Some(rest) = line.trim().strip_prefix(BATCH_MARKER) {
            // Flush previous
            if let Some(id) = current_id.take() {
                let code = current_code.trim().to_string();
                if !code.is_empty() {
                    let clause_id = ought_spec::ClauseId(id);
                    let file_path = derive_file_path_from_id(&clause_id, language);
                    tests.push(GeneratedTest {
                        clause_id,
                        code,
                        language,
                        file_path,
                    });
                }
                current_code.clear();
            }
            // Parse the clause ID from the marker
            let id = rest.trim().trim_end_matches("===").trim().to_string();
            if !id.is_empty() {
                current_id = Some(id);
            }
        } else if current_id.is_some() {
            current_code.push_str(line);
            current_code.push('\n');
        }
    }

    // Flush last
    if let Some(id) = current_id.take() {
        let code = current_code.trim().to_string();
        if !code.is_empty() {
            let clause_id = ought_spec::ClauseId(id);
            let file_path = derive_file_path_from_id(&clause_id, language);
            tests.push(GeneratedTest {
                clause_id,
                code,
                language,
                file_path,
            });
        }
    }

    // If parsing failed (no markers found), and there's only one clause,
    // treat the whole response as that clause's test
    if tests.is_empty() && group.clauses.len() == 1 {
        let clause = group.clauses[0];
        let code = response.trim().to_string();
        if !code.is_empty() {
            tests.push(GeneratedTest {
                clause_id: clause.id.clone(),
                code,
                language,
                file_path: derive_file_path(clause, language),
            });
        }
    }

    tests
}

fn derive_file_path_from_id(clause_id: &ought_spec::ClauseId, language: Language) -> PathBuf {
    let ext = match language {
        Language::Rust => "_test.rs",
        Language::Python => "_test.py",
        Language::TypeScript => ".test.ts",
        Language::JavaScript => ".test.js",
        Language::Go => "_test.go",
    };
    let path_str = clause_id.0.replace("::", "/");
    PathBuf::from(format!("{path_str}{ext}"))
}

/// Derive the output file path from a clause ID and target language.
pub fn derive_file_path(clause: &Clause, language: Language) -> PathBuf {
    let ext = match language {
        Language::Rust => "_test.rs",
        Language::Python => "_test.py",
        Language::TypeScript => ".test.ts",
        Language::JavaScript => ".test.js",
        Language::Go => "_test.go",
    };

    let path_str = clause.id.0.replace("::", "/");
    PathBuf::from(format!("{path_str}{ext}"))
}

/// Execute a CLI command with the prompt on stdin, return stdout.
/// When `verbose` is true, streams stdout to stderr in real-time.
pub fn exec_cli(
    command: &str,
    args: &[&str],
    prompt: &str,
) -> anyhow::Result<String> {
    exec_cli_inner(command, args, Some(prompt), false)
}

/// Verbose version: streams LLM output to stderr in real-time.
pub fn exec_cli_verbose(
    command: &str,
    args: &[&str],
    prompt: Option<&str>,
) -> anyhow::Result<String> {
    exec_cli_inner(command, args, prompt, true)
}

fn exec_cli_inner(
    command: &str,
    args: &[&str],
    stdin_data: Option<&str>,
    verbose: bool,
) -> anyhow::Result<String> {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    let stdin_cfg = if stdin_data.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };

    let mut child = Command::new(command)
        .args(args)
        .stdin(stdin_cfg)
        .stdout(Stdio::piped())
        .stderr(if verbose {
            Stdio::inherit() // let provider stderr (progress, etc.) show through
        } else {
            Stdio::piped()
        })
        .spawn()
        .map_err(|e| cli_spawn_error(command, e))?;

    // Write stdin if provided
    if let Some(data) = stdin_data {
        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(data.as_bytes())?;
        }
        drop(child.stdin.take());
    }

    if verbose {
        // Stream stdout to stderr in real-time while accumulating it
        let stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture stdout from '{}'", command))?;

        let reader = BufReader::new(stdout_pipe);
        let mut accumulated = String::new();
        let stderr = std::io::stderr();

        for line in reader.lines() {
            let line = line?;
            // Dim the streaming output so it's visually distinct
            let _ = writeln!(stderr.lock(), "    \x1b[2m{}\x1b[0m", line);
            accumulated.push_str(&line);
            accumulated.push('\n');
        }

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("'{}' exited with status {}", command, status);
        }

        Ok(strip_markdown_fences(&accumulated))
    } else {
        // Non-verbose: buffer everything
        let output = child.wait_with_output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let detail = if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            anyhow::bail!(
                "'{}' exited with status {}:\n{}",
                command,
                output.status,
                detail
            );
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|e| anyhow::anyhow!("invalid UTF-8 from '{}': {}", command, e))?;

        Ok(strip_markdown_fences(&stdout))
    }
}

fn cli_spawn_error(command: &str, e: std::io::Error) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::NotFound {
        anyhow::anyhow!(
            "CLI tool '{}' not found. Please install it and ensure it is on your PATH.",
            command
        )
    } else {
        anyhow::anyhow!("failed to spawn '{}': {}", command, e)
    }
}

pub fn keyword_str(kw: ought_spec::Keyword) -> &'static str {
    match kw {
        ought_spec::Keyword::Must => "MUST",
        ought_spec::Keyword::MustNot => "MUST NOT",
        ought_spec::Keyword::Should => "SHOULD",
        ought_spec::Keyword::ShouldNot => "SHOULD NOT",
        ought_spec::Keyword::May => "MAY",
        ought_spec::Keyword::Wont => "WONT",
        ought_spec::Keyword::Given => "GIVEN",
        ought_spec::Keyword::Otherwise => "OTHERWISE",
        ought_spec::Keyword::MustAlways => "MUST ALWAYS",
        ought_spec::Keyword::MustBy => "MUST BY",
    }
}

fn language_name(lang: Language) -> &'static str {
    match lang {
        Language::Rust => "Rust",
        Language::Python => "Python",
        Language::TypeScript => "TypeScript",
        Language::JavaScript => "JavaScript",
        Language::Go => "Go",
    }
}
