# ought-report

Terminal, JSON, and JUnit reporting for ought.

Formats `RunResult`s from `ought-run` for humans (colored terminal output,
optional TTY-aware spinners) and machines (JSON for tooling, JUnit XML for
CI). Knows nothing about generation or analysis — it only reads results and
the specs they came from.

## Responsibilities

- Render a run's pass/fail/error breakdown with clause-level detail.
- Distinguish failure severity by keyword (MUST failures are errors, SHOULD
  failures are warnings by default).
- Emit machine-readable formats for CI pipelines.

## Notable public API

- `terminal::report(results, specs, &ReportOptions)` — human-readable summary
  to stdout; `terminal::report_to_writer(...)` for non-stdout sinks.
- `json::report(results, specs) -> String` — structured JSON report.
- `junit::report(results, specs, path)` — write a JUnit XML file (useful as
  `--junit out.xml` on the CLI).
- `ReportOptions`, `ColorChoice` — caller-facing format controls.
