---
title: Configuration
description: The ought.toml schema — every section and key.
order: 6
---

`ought init` writes an `ought.toml` at the project root. Every CLI command discovers it by walking up from the current directory. Pass `--config <path>` to point at a different file.

## Full example

```toml
[project]
name = "myapp"
version = "0.1.0"

[specs]
roots = ["ought/"]

[context]
search_paths = ["src/", "lib/"]
exclude = ["vendor/", "generated/"]
max_files = 50

[generator]
provider = "anthropic"          # "anthropic" | "openai" | "openai-codex" | "openrouter" | "ollama"
model = "claude-sonnet-4-6"
max_turns = 50
parallelism = 1

[generator.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
# base_url = "https://api.anthropic.com"   # proxies / Bedrock / Vertex

[generator.tolerance]
must_by_multiplier = 1.0        # CI timing slack for MUST BY clauses

[runner.rust]
test_dir = "ought/ought-gen/"
# Bare section name matches a built-in preset (rust, python, typescript, go).
# Override any field below to customize.

[mcp]
enabled = false
transport = "stdio"             # "stdio" | "sse"
```

## `[project]`

| Key       | Default | Notes                              |
| --------- | ------- | ---------------------------------- |
| `name`    | —       | Required. Used in banners.         |
| `version` | `0.1.0` | Free-form string; not parsed.      |

## `[specs]`

Where on disk Ought looks for `.ought.md` files.

| Key     | Default      | Notes                                                 |
| ------- | ------------ | ----------------------------------------------------- |
| `roots` | `["ought/"]` | Directories the parser walks. Relative to `ought.toml`. |

## `[context]`

Source files included as context for LLM-driven commands (`generate`, `extract`, `analyze survey`).

| Key            | Default | Notes                                                          |
| -------------- | ------- | -------------------------------------------------------------- |
| `search_paths` | `[]`    | Source roots. Empty means "no source context."                 |
| `exclude`      | `[]`    | Glob patterns relative to each search path.                    |
| `max_files`    | `50`    | Hard cap on files read into a single generation prompt.        |

## `[generator]`

LLM provider, model, and agent-loop limits.

| Key                         | Default              | Notes                                                                |
| --------------------------- | -------------------- | -------------------------------------------------------------------- |
| `provider`                  | `"anthropic"`        | One of `anthropic`, `openai`, `openai-codex`, `openrouter`, `ollama`. |
| `model`                     | `"claude-sonnet-4-6"`| Provider-specific model identifier.                                  |
| `max_turns`                 | `50`                 | Cap on agent-loop iterations per clause.                             |
| `max_tokens_per_response`   | `8192`               | Per-response token cap.                                              |
| `temperature`               | unset                | Optional sampling temperature.                                       |
| `read_source_limit_bytes`   | preset-defined       | Per-call cap on bytes from `read_source`; truncated reads can resume.|
| `context_budget_tokens`     | `180000`             | Hard cap on input tokens; aborts before the provider would 400.      |
| `eviction_threshold_tokens` | `130000`             | Soft threshold at which old `tool_result` blocks get rewritten.      |
| `parallelism`               | `1`                  | Concurrent assignments.                                              |

### Provider sub-blocks

Only the block matching `provider` is read — the rest are ignored, so it's fine to leave them in.

```toml
[generator.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
# base_url = "https://api.anthropic.com"

[generator.openai]
api_key_env = "OPENAI_API_KEY"
# base_url = "https://api.openai.com/v1"

[generator.openai-codex]
# Run `ought auth login openai-codex` first.
# auth_file = "/absolute/path/to/auth.json" # defaults to $OUGHT_AUTH_FILE or ~/.ought/auth.json
# base_url = "https://chatgpt.com/backend-api"

[generator.openrouter]
api_key_env = "OPENROUTER_API_KEY"
# app_url   = "https://example.com"      # HTTP-Referer header
# app_title = "myapp"                    # X-Title header

[generator.ollama]
# base_url = "http://localhost:11434/v1"
```

### `[generator.tolerance]`

| Key                  | Default | Notes                                                                |
| -------------------- | ------- | -------------------------------------------------------------------- |
| `must_by_multiplier` | `1.0`   | Multiplier applied to every `MUST BY` deadline. Raise on slow CI.    |

## `[runner.<name>]`

One section per language runner. The section name is significant: if it matches a built-in preset (`rust`, `python`, `typescript`, `go`), the preset fills in any unset field.

| Key               | Required        | Notes                                                                |
| ----------------- | --------------- | -------------------------------------------------------------------- |
| `preset`          | —               | Optional explicit preset. Defaults to the section name if it matches.|
| `command`         | yes (or preset) | Shell command. Tokens: `{test_dir}`, `{junit_path}`, `{tap_path}`, `{json_path}`. |
| `test_dir`        | yes             | Where generated test files are written.                              |
| `format`          | yes (or preset) | `junit-xml` \| `tap` \| `ought-json` \| `cargo-test`.                |
| `file_extensions` | yes (or preset) | Extensions Ought treats as generated tests, e.g. `["rs"]`.           |
| `output_path`     | —               | Fixed path for formatted output; otherwise inferred from `command`.  |
| `working_dir`     | —               | Working directory for the spawned command.                           |
| `env`             | —               | Env vars merged into the child process; values may use the same tokens.|
| `available_check` | —               | Shell command to probe runner availability. Defaults to first token of `command`. |

A bare `[runner.python]` with just `test_dir` is enough — the preset supplies the rest.

## `[mcp]`

Settings for the [`ought mcp`](/products/ought/docs/cli-reference#ought-mcp) server.

| Key         | Default   | Notes                                |
| ----------- | --------- | ------------------------------------ |
| `enabled`   | `false`   | Whether the server starts.           |
| `transport` | `"stdio"` | `stdio` for local IDEs, `sse` for HTTP. |
