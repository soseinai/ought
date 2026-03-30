# CLI

context: The `ought` binary is the primary interface. Built with `clap` for argument parsing. All commands write structured output to stdout and diagnostics to stderr. Exit code 0 on success, 1 on test failure, 2 on usage error.

source: src/cli/

## Init

- **MUST** scaffold an `ought.toml`, an `ought/` directory, and an example spec file inside it when run in a project directory
- **MUST** detect the project language from existing config files (Cargo.toml, package.json, pyproject.toml, go.mod) and set defaults accordingly
- **MUST NOT** overwrite an existing `ought.toml`
- **MAY** prompt the user interactively for generator provider and model preferences

## Run

- **MUST** parse all spec files, execute generated tests, and report results mapped back to clauses
- **MUST** accept a path argument to run a specific spec file: `ought run ought/auth.ought.md`
- **MUST** accept a glob pattern to run a subset: `ought run "ought/auth*.ought.md"`
- **MUST** exit with code 1 if any MUST, MUST NOT, MUST ALWAYS, or MUST BY clause fails
- **MUST** exit with code 0 if only SHOULD or MAY clauses fail
- **GIVEN** a clause has an OTHERWISE chain and the primary obligation fails:
  - **MUST** exit with code 0 if any OTHERWISE clause in the chain passes (graceful degradation accepted)
  - **MUST** exit with code 1 if all OTHERWISE clauses also fail (full degradation chain exhausted)
- **MUST NOT** trigger test generation — `ought run` only executes existing generated tests
- **SHOULD** support `--fail-on-should` flag to also fail on SHOULD clause failures
- **SHOULD** print a summary at the end showing pass/fail counts by severity level
- **WONT** execute tests in parallel by default in v0.1 (sequential is fine to start)

## Generate

- **MUST** regenerate test code for all clauses where the clause hash or source hash has changed
- **MUST** support `--force` flag to regenerate all clauses regardless of hash
- **MUST** support `--check` flag that exits with code 1 if any generated tests are stale (for CI)
- **MUST** update the manifest.toml with new hashes after generation
- **MUST** write generated tests to the `ought/ought-gen/` directory
- **MUST NOT** execute tests during generation (that is `run`'s job)
- **SHOULD** show a progress indicator during LLM generation
- **SHOULD** support targeting a specific spec file: `ought generate ought/auth.ought.md`

## Check

- **MUST** validate the syntax of all spec files without generating or running anything
- **MUST** report parse errors with file, line number, and a human-readable message
- **MUST** validate that cross-file references (links to other .ought.md files) resolve
- **MUST** exit with code 0 if all specs are valid, 1 if any are invalid

## Inspect

- **MUST** print the generated test code for a given clause identifier
- **MUST** accept clause identifiers in the form `file::section::clause` (e.g. `auth::login::must_return_jwt`)
- **SHOULD** syntax-highlight the output when stdout is a terminal
- **SHOULD** show the clause text alongside the generated code for easy comparison

## Diff

- **MUST** show the diff between current generated tests and what would be generated now
- **SHOULD** use a familiar unified diff format
- **SHOULD** group diffs by spec file

## Watch

- **MUST** watch `ought.md` files and source files for changes
- **MUST** re-run affected specs when a change is detected
- **SHOULD** debounce rapid file changes (at least 500ms)
- **SHOULD** clear the terminal and reprint results on each cycle

## Global Flags

- **MUST** support `--config <path>` to specify an alternate ought.toml location
- **MUST** support `--quiet` flag that suppresses all output except errors and the final summary
- **MUST** support `--json` flag that outputs structured JSON for programmatic consumption
- **MUST** support `--junit <path>` flag that writes JUnit XML results to the given file
- **MUST** support `--color <auto|always|never>` for terminal color control
- **SHOULD** support `--verbose` flag for debug-level output
- **MUST ALWAYS** write diagnostic messages to stderr, never stdout (stdout is reserved for structured output and results)
- **MUST ALWAYS** return a valid exit code (0, 1, or 2) — never crash without an exit code
