# Reporter

context: The reporter renders test results back to the user, mapped to the original spec clauses. It is the visual face of ought — designed to be attractive, information-dense, and immediately useful. Also responsible for LLM-powered failure narratives.

source: src/reporter/

## Terminal Output

- **MUST** display results grouped by spec file, then by section, then by clause
- **MUST** show each clause with its keyword, text, and pass/fail status
- **MUST** use status indicators: `✓` passed, `✗` failed, `!` errored, `⊘` confirmed absent (WONT), `~` skipped
- **MUST** color-code by severity: MUST failures in red, SHOULD failures in yellow, MAY in dim/gray
- **MUST** print a summary line at the end: total passed, failed, errored, by severity
- **MUST** show MUST coverage percentage in the summary (passed MUST clauses / total MUST clauses)
- **SHOULD** show section-level pass/fail rollup alongside each section header
- **SHOULD** use box-drawing characters for failure detail panels
- **SHOULD** dim passing clauses and highlight failures for visual scanning
- **WONT** use animated spinners or progress bars in non-TTY mode (pipe-friendly)

## GIVEN Block Display

- **MUST** display GIVEN blocks as a visual group with the condition as a header line
- **MUST** indent clauses under their GIVEN condition to show the relationship
- **SHOULD** dim the GIVEN condition line when all nested clauses pass
- **SHOULD** highlight the GIVEN condition line when any nested clause fails (the condition is relevant context)

```
 GIVEN the user is authenticated:
   ✓ MUST  return their profile data
   ✗ MUST  NOT return other users' private data
 GIVEN the token is expired:
   ✓ MUST  return 401
   ✓ SHOULD include WWW-Authenticate header
```

## OTHERWISE Chain Display

- **MUST** display OTHERWISE clauses indented under their parent obligation
- **MUST** use a distinct indicator for OTHERWISE results: `↳` prefix to show the fallback relationship
- **MUST** show the full chain status: if the parent passes, OTHERWISE clauses show as `~` (not needed)
- **GIVEN** the parent obligation fails:
  - **MUST** show which OTHERWISE level caught the failure
  - **SHOULD** visually distinguish the "active" fallback from lower ones that weren't reached

```
 ✗ MUST  respond within 200ms
   ↳ ✓ OTHERWISE return a cached response
   ↳ ~ OTHERWISE return 504              (not reached — caught above)
```

## Temporal Result Display

- **MUST** display MUST ALWAYS results with the number of iterations/inputs tested
- **MUST** display MUST BY results with the measured duration alongside the deadline
- **SHOULD** show a timing bar or ratio for MUST BY clauses: `[47ms / 200ms]`

```
 ✓ MUST ALWAYS return valid JSON          (tested 1000 inputs)
 ✓ MUST BY 200ms return a response        [47ms / 200ms]
 ✗ MUST BY 100ms acknowledge the write    [230ms / 100ms]
```

## Failure Details

- **MUST** show the assertion error or failure message for each failed clause
- **MUST** show the file and line number of the failure in the generated test
- **SHOULD** show a snippet of the failing generated test code inline
- **SHOULD** show the original clause text alongside the failure for easy comparison

## Failure Narratives (LLM-powered)

- **MUST** support a `--diagnose` flag that enables LLM-powered failure diagnosis
- **MUST** send the failing clause, generated test, failure output, and relevant source code to the LLM
- **MUST** display the diagnosis in a distinct visual panel below the failure
- **MUST** include a suggested fix (file, line, what to change) when the LLM can determine one
- **MUST NOT** run diagnosis automatically without `--diagnose` (it costs API calls)
- **SHOULD** diagnose all failures in a single batch LLM call when possible
- **SHOULD** include the git diff since the last passing run in the diagnosis context

## Test Quality Grading

- **MUST** support a `--grade` flag that enables LLM-powered test quality assessment
- **MUST** assign a letter grade (A-F) to each generated test based on how well it validates the clause
- **MUST** display the grade alongside each clause in the output
- **SHOULD** include a brief explanation for grades below B
- **SHOULD** suggest improvements for low-graded tests
- **MUST NOT** run grading automatically without `--grade`

## JSON Output

- **MUST** support `--json` flag that outputs structured results as JSON to stdout
- **MUST** include all fields: clause identifier, keyword, severity, status, failure message, duration
- **MUST** include diagnosis and grade data when those flags are also active
- **MUST NOT** mix JSON output with human-readable output (one or the other)

## JUnit XML Output

- **MUST** support `--junit <path>` flag that writes results in JUnit XML format
- **MUST** map spec files to `<testsuite>` elements and clauses to `<testcase>` elements
- **MUST** include failure messages and clause identifiers in `<failure>` elements
- **SHOULD** include the clause keyword and severity as properties on each `<testcase>`
- **MAY** be combined with other output modes (e.g. human-readable to terminal + JUnit XML to file)

## Progress During Generation

- **SHOULD** show a progress bar or spinner during LLM generation with clause count
- **SHOULD** stream LLM token output when in verbose mode
- **MAY** show estimated time remaining based on per-clause generation speed
