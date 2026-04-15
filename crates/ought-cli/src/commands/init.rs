pub fn run() -> anyhow::Result<()> {
    if std::path::Path::new("ought.toml").exists() {
        anyhow::bail!("ought.toml already exists in this directory");
    }

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
        "rust"
    };

    std::fs::create_dir_all("ought")?;

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
# Pick one: anthropic | openai | openrouter | ollama
provider = "anthropic"
model = "claude-sonnet-4-6"
# parallelism = 1
# max_turns = 50

# Auth via env var (no keys in this file). Only the block matching
# `provider` above is read.
[generator.anthropic]
api_key_env = "ANTHROPIC_API_KEY"

# [generator.openai]
# api_key_env = "OPENAI_API_KEY"
#
# [generator.openrouter]
# api_key_env = "OPENROUTER_API_KEY"
# app_url = "https://example.com"
# app_title = "{name}"
#
# [generator.ollama]
# base_url = "http://localhost:11434/v1"

[runner.{lang}]
test_dir = "ought/ought-gen/"
"#,
        name = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "myproject".into()),
        lang = language,
    );
    std::fs::write("ought.toml", config_content)?;

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
