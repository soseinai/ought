# Analysis

context: The analysis commands are LLM-powered features that go beyond basic test generation and execution. They reason about the relationships between intent (specs), implementation (source), and evidence (results). These are the features that differentiate ought from conventional test tools.

source: src/analysis/

requires: [parser](../engine/parser.ought.md), [generator](../engine/generator.ought.md), [runner](../engine/runner.ought.md)

## Survey

`ought analyze survey [path]` — discovers behaviors in source code that are not covered by any spec.

- **MUST** read source files from the given path (or project source roots if no path given)
- **MUST** read all existing spec files to know what is already covered
- **MUST** use the LLM to identify public behaviors, APIs, and logic branches in the source that lack corresponding clauses
- **MUST** output a list of uncovered behaviors with file and line references
- **MUST** suggest concrete clause text (with appropriate keyword) for each uncovered behavior
- **SHOULD** offer to append suggested clauses to the relevant spec file (or create a new one)
- **SHOULD** group suggestions by the spec file they would belong to
- **SHOULD** rank uncovered behaviors by risk (public API > internal helper)
- **WONT** auto-add clauses without user confirmation

## Audit

`ought analyze audit` — cross-spec analysis for contradictions, gaps, and coherence issues.

- **MUST** read all spec files and their cross-references
- **MUST** use the LLM to identify contradictions between clauses (across files or within)
- **MUST** use the LLM to identify gaps — areas where related clauses exist but expected companion clauses are missing
- **MUST** categorize findings as: contradiction, gap, ambiguity, or redundancy
- **MUST** reference the specific clauses involved in each finding (file, section, line)
- **MUST** detect MUST BY deadline conflicts (e.g. an operation with a 100ms deadline that calls a sub-operation with a 200ms deadline)
- **MUST** detect MUST ALWAYS invariant conflicts (e.g. two invariants that cannot simultaneously hold)
- **SHOULD** detect GIVEN blocks with overlapping conditions that impose contradictory obligations
- **SHOULD** detect MUST obligations that lack OTHERWISE fallbacks where degradation is likely (e.g. network-dependent operations)
- **SHOULD** read relevant source code to ground the analysis in implementation reality
- **SHOULD** suggest resolutions for each finding
- **MAY** assign a confidence score to each finding

## Blame

`ought debug blame <clause>` — explains why a clause is failing by correlating with source changes.

- **MUST** accept a clause identifier (e.g. `auth::login::must_return_401`)
- **MUST** retrieve the clause, its generated test, and the failure output
- **MUST** use git history to find when the clause last passed and what changed since
- **MUST** use the LLM to correlate the source diff with the failure and produce a causal explanation
- **MUST** output the timeline: last passing run, first failure, relevant commits
- **MUST** output a narrative explanation of what broke and why
- **SHOULD** identify the specific commit and file change most likely responsible
- **SHOULD** name the author of the likely-responsible commit
- **SHOULD** suggest a fix when the cause is clear
- **MUST NOT** require a running LLM if the clause has never passed (just report "never passed")

## Bisect

`ought debug bisect <clause>` — automated binary search through git history to find the breaking commit.

- **MUST** accept a clause identifier
- **MUST** perform a git-bisect-style binary search: checkout commit, generate test for clause, run it, narrow range
- **MUST** report the first commit where the clause fails
- **MUST** show the commit message, author, date, and diff summary for the breaking commit
- **MUST ALWAYS** restore the working tree to its original state after completion (never leave on detached HEAD)
- **SHOULD** use the generated test from the current manifest (not regenerate per commit) unless `--regenerate` is passed
- **SHOULD** cache test results per commit to avoid redundant runs
- **SHOULD** support `--range <from>..<to>` to limit the search space
- **GIVEN** the bisect is interrupted (SIGINT, crash):
  - **MUST** restore the working tree to the original branch
  - **SHOULD** save progress so `ought debug bisect --continue` can resume
