# Runner

context: The runner executes generated tests by delegating to language-specific test harnesses (cargo test, pytest, jest, go test, etc). It does not implement its own test execution — it orchestrates external tools and captures their output. Results are mapped back to clause identifiers for the reporter.

source: src/runner/

## Execution

- **MUST** invoke the configured test command from `ought.toml` for each language runner
- **MUST** pass the generated test files/directory to the test harness
- **MUST** capture stdout, stderr, and exit code from the test harness
- **MUST** map individual test pass/fail results back to clause identifiers
- **MUST** support running tests for a single spec file (filtering generated tests by origin spec)
- **MUST NOT** modify generated test files during execution
- **MUST BY 5m** complete a full test suite execution (configurable via `ought.toml`)
  - **OTHERWISE** kill the test harness process and report a timeout error
- **MUST NOT** trigger generation — the runner only executes existing generated tests

## Result Collection

The runner is agnostic to how it obtains test results. Stdout parsing is one strategy, but not the only one — runners may use JUnit XML output, structured JSON, test harness APIs, or inline execution.

- **MUST** collect per-test results and map each back to its clause identifier
- **MUST** capture failure messages, assertion errors, and stack traces per test
- **MUST** classify each clause result as: passed, failed, errored (test itself broke), or skipped
- **MUST** capture test execution duration per clause (required for MUST BY reporting)
- **GIVEN** a clause has OTHERWISE children:
  - **MUST** run the parent test first
  - **MUST** run OTHERWISE tests only if the parent test fails
  - **MUST** stop the OTHERWISE chain at the first passing fallback
  - **MUST** mark remaining lower-priority OTHERWISE clauses as skipped (not reached)
- **GIVEN** a clause is MUST ALWAYS:
  - **MUST** capture the number of iterations/inputs tested
- **GIVEN** a clause is MUST BY:
  - **MUST** capture the measured duration for reporting

## Language Runners

Each runner implements result collection in whatever way is most natural for its ecosystem — JUnit XML output, structured JSON, harness APIs, or stdout parsing as a last resort.

- **MUST** ship with a Rust runner
- **SHOULD** ship with a Python runner
- **SHOULD** ship with a JavaScript/TypeScript runner
- **MAY** ship with a Go runner
- **SHOULD** support custom runners via the `[runner.<name>]` config in `ought.toml`

## Error Handling

- **MUST** distinguish between test failures (assertion failed) and test errors (test code itself crashed)
- **MUST** report when the test harness command is not found or fails to start
- **MUST** report when a generated test file is missing (referenced in manifest but not on disk)
- **MUST NOT** mask harness stderr — pass it through for debugging
- **SHOULD** detect and report when no tests were generated for a spec (nothing to run)
- **MUST ALWAYS** leave the test environment clean after execution (no leaked child processes, temp files removed)
