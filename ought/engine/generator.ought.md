# Generator

context: The generator takes parsed clause IR + source code context and uses an LLM to produce concrete test implementations. It is provider-agnostic — different LLM backends are supported through a trait-based plugin system. Generated tests are written to `ought/ought-gen/` and tracked in a manifest.

source: src/generator/

## Test Generation

- **MUST** generate one test function per clause
- **MUST** include the original clause text as a doc comment in the generated test
- **MUST** include the clause identifier in the test function name
- **MUST** generate tests that are self-contained (no cross-test dependencies)
- **MUST** generate tests appropriate for the target language specified in `ought.toml`
- **MUST NOT** generate tests that perform real IO (network calls, filesystem writes, database operations) unless the clause explicitly describes integration behavior
- **SHOULD** generate descriptive assertion messages that reference the clause
- **SHOULD** use the target language's idiomatic test patterns (e.g. `#[test]` for Rust, `test()` for Jest)
- **MAY** generate helper functions or fixtures when multiple clauses in a section share setup

## WONT Clause Handling

- **MUST** generate two kinds of tests for WONT clauses based on the clause text:
  - Absence tests: verify the capability does not exist (e.g. no endpoint, no method, no feature flag)
  - Prevention tests: verify that attempting the behavior fails gracefully
- **SHOULD** use the clause text to determine which kind of WONT test to generate

## GIVEN Block Generation

- **MUST** use the GIVEN condition text to generate test setup/precondition code
- **MUST** generate a separate test function for each clause within the GIVEN block, all sharing the same precondition setup
- **MUST** include the GIVEN condition in the LLM prompt so it understands the precondition context
- **SHOULD** generate a shared setup function or fixture for clauses under the same GIVEN block
- **GIVEN** a clause has nested GIVEN blocks:
  - **MUST** compose the conditions — the inner test setup includes both outer and inner preconditions

## OTHERWISE Chain Generation

- **MUST** generate a test for the primary obligation (the parent clause)
- **MUST** generate a separate test for each OTHERWISE clause in the chain
- **MUST** instruct the LLM that OTHERWISE tests should simulate the parent obligation's failure condition, then verify the fallback behavior activates
- **MUST** preserve the chain order — each OTHERWISE test assumes all previous levels also failed
- **SHOULD** generate a single integration-style test that walks the full degradation chain in sequence
- **MUST NOT** generate OTHERWISE tests that depend on real infrastructure failures (simulate the failure condition in-process)

## Temporal Obligation Generation

### MUST ALWAYS (Invariant Tests)

- **MUST** instruct the LLM to generate property-based or fuzz-style tests for MUST ALWAYS clauses
- **MUST** generate tests that verify the invariant holds across multiple inputs, states, or iterations
- **SHOULD** generate tests that exercise boundary conditions and edge cases for the invariant
- **SHOULD** use the target language's property testing library when available (e.g. `proptest` for Rust, `hypothesis` for Python, `fast-check` for JS)
- **MAY** generate a loop-based stress test when no property testing library is available

### MUST BY (Deadline Tests)

- **MUST** generate tests that assert the operation completes within the specified duration
- **MUST** include the deadline duration from the clause in the test's timeout/assertion
- **MUST** instruct the LLM to measure wall-clock time around the operation under test
- **SHOULD** generate tests that run the operation multiple times and assert the p99 latency is within the deadline
- **SHOULD** account for CI environment variability by supporting a configurable tolerance multiplier in `ought.toml`

## Context Assembly

- **MUST** send the clause text, keyword, severity, and section context to the LLM
- **MUST** read and include source files referenced by `source:` metadata
- **MUST** read and include schema files referenced by `schema:` metadata
- **MUST** include the free-text `context:` block
- **MUST** include any code-block hints attached to the clause
- **MUST** respect the `max_files` limit in `ought.toml` to avoid exceeding LLM context
- **SHOULD** auto-discover relevant source files when no explicit `source:` is provided
- **SHOULD** rank discovered files by relevance to the clause text

## Provider Abstraction

Providers are invoked by exec-ing their CLI tools rather than calling APIs directly. This avoids all auth management — users authenticate through their CLI tools however they want and can use consumer accounts, pro plans, or API keys as they see fit.

- **MUST** define a `Generator` trait that all LLM providers implement
- **MUST** ship with a Claude provider that execs the `claude` CLI
- **MUST** ship with an OpenAI provider that execs the `openai` or `chatgpt` CLI
- **MUST** pass prompts via stdin and capture generated test code from stdout
- **MUST NOT** manage API keys or authentication — that is the CLI tool's responsibility
- **SHOULD** ship with an Ollama provider that execs the `ollama` CLI for local models
- **SHOULD** support provider-specific configuration in `ought.toml` under `[generator]`
- **SHOULD** detect when the required CLI tool is not installed and report a clear error
- **MAY** support custom providers by specifying an arbitrary executable in `ought.toml`

## Manifest and Hashing

- **MUST** compute a clause hash from the keyword + clause text + context metadata
- **MUST** compute a source hash from the contents of referenced source files
- **MUST** write both hashes to `ought/ought-gen/manifest.toml` after generation
- **MUST** record the model name and timestamp in the manifest entry
- **MUST** skip generation for clauses whose hashes match the manifest (unless `--force`)
- **MUST** detect and remove orphaned generated tests (clause was deleted from spec)

## Error Handling

- **MUST** report LLM API errors clearly (auth failure, rate limit, timeout)
- **MUST NOT** leave the manifest in an inconsistent state if generation is interrupted
- **SHOULD** retry transient API errors with exponential backoff (max 3 retries)
- **SHOULD** continue generating remaining clauses if one clause fails
