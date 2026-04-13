# ought-run

Test runner and result collection for ought.

Delegates test execution to each language's existing harness (`cargo test`,
`pytest`, `npx jest`, `go test`, …) and maps pass/fail output back to the
`ClauseId`s the tests were generated from. New languages plug in by
implementing the `Runner` trait.

## Responsibilities

- Invoke the configured test harness in the runner's `test_dir`.
- Parse harness output and produce a `RunResult` of per-clause `TestResult`s
  (status, message, duration, captured stdout/stderr).
- Provide a factory (`runners::from_name`) mapping config keys like `"rust"`,
  `"python"`, `"typescript"`, `"go"` to the shipped runner implementations.
- Own `RunnerConfig` — the per-runner `[runner.<name>]` sub-config.

## Notable public API

- `trait Runner` — `run(tests, test_dir) -> RunResult`, `is_available()`,
  `name()`. Implemented by `RustRunner`, `PythonRunner`, `TypeScriptRunner`,
  `GoRunner`.
- `runners::from_name(name)` — factory that returns a `Box<dyn Runner>` for
  a config key (also honors short aliases like `"ts"` → TypeScript).
- `RunResult`, `TestResult`, `TestStatus`, `TestDetails` — the result model
  that reporters and analysis consume.
- `RunnerConfig { command, test_dir }` — deserializable sub-config composed
  by the aggregate `Config` in `ought-cli`.
