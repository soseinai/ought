# ought-analysis

LLM-powered analysis passes over specs and run results: survey, audit, blame,
bisect.

Where `ought-gen` writes tests and `ought-run` executes them, this crate
reasons about *why* things look the way they do — uncovered behaviors in
source code, contradictions between specs, the likely cause of a regression.

## Responsibilities

- Scan source directories for behaviors not covered by any spec (`survey`).
- Cross-check specs for contradictions, duplication, and gaps (`audit`).
- Correlate a failing clause with git history and produce a narrative
  explanation (`blame`).
- Binary-search git history to find the breaking commit for a clause
  (`bisect`).

## Notable public API

- `survey::survey(&specs, &paths) -> SurveyResult` — returns
  `UncoveredBehavior`s with suggested clause text, keyword, and target spec.
- `audit::audit(&specs) -> AuditResult` — `AuditFinding`s classified by
  `AuditFindingKind`, each linking the implicated clauses.
- `blame::blame(&clause_id, &specs, &run_result) -> BlameResult` — timeline,
  likely commit, and LLM narrative for a failing clause.
- `bisect::bisect(&clause_id, &specs, runner, &BisectOptions) -> BisectResult`
  — identifies the `CommitInfo` where a clause started failing.
