# Parser

context: The parser converts `.ought.md` files into a structured clause IR. It is pure Rust, uses no LLM, and is the component that defines the spec format. Published separately as the `ought-spec` crate for ecosystem interop.

source: src/parser/

## Spec File Structure

- **MUST** parse standard Markdown (CommonMark) as the base format
- **MUST** recognize files with the `.ought.md` extension
- **MUST** parse frontmatter-style metadata at the top of the file: `context:`, `source:`, `schema:`, `requires:`
- **MUST** treat H1 (`#`) as the spec name
- **MUST** treat H2+ (`##`, `###`, etc.) as nested test groups/sections
- **MUST** treat bullet points (`- **KEYWORD**`) as individual clauses
- **MUST** preserve all non-clause markdown as documentation (context for the LLM, readable for humans)
- **MUST NOT** fail on standard markdown that doesn't contain ought keywords (just produce zero clauses)

## Keywords

- **MUST** recognize RFC 2119 keywords: MUST, MUST NOT, SHOULD, SHOULD NOT, MAY
- **MUST** recognize the WONT keyword as an ought extension (not in RFC 2119)
- **MUST** recognize the GIVEN keyword as a conditional block opener (from deontic logic)
- **MUST** recognize the OTHERWISE keyword as a contrary-to-duty fallback (from deontic logic)
- **MUST** recognize temporal compound keywords: MUST ALWAYS, MUST BY
- **MUST** parse keywords case-insensitively but require them to appear in bold (`**MUST**`, `**GIVEN**`, etc.)
- **MUST** assign severity levels: MUST/MUST NOT/MUST ALWAYS/MUST BY = required, SHOULD/SHOULD NOT = recommended, MAY = optional, WONT = negative-confirmation
- **MUST NOT** treat bare (non-bold) keyword occurrences as clauses (e.g. "you must restart" in prose)

## Conditional Blocks (GIVEN)

GIVEN introduces a precondition that scopes the clauses nested under it. Rooted in deontic logic's conditional obligation O(p | q) — "p is obligated, given q."

```markdown
- **GIVEN** the user is authenticated:
  - **MUST** return their profile data
  - **MUST NOT** return other users' private data
```

- **MUST** parse `**GIVEN**` as a block-level keyword that contains nested clauses
- **MUST** require nested clauses to be indented under the GIVEN bullet (standard markdown nesting)
- **MUST** attach the GIVEN condition text to all clauses nested within it
- **MUST** support multiple GIVEN blocks within a section
- **MUST** support GIVEN blocks containing any keyword (MUST, SHOULD, MAY, WONT, OTHERWISE, etc.)
- **SHOULD** support nested GIVEN blocks (conditions that narrow further)
- **MUST NOT** treat GIVEN itself as a testable clause — it is a grouping construct with a precondition

## Contrary-to-Duty Chains (OTHERWISE)

OTHERWISE defines a fallback obligation that activates when its parent clause is violated. Rooted in Chisholm's contrary-to-duty obligations from deontic logic — "if you fail to X, you ought to at minimum Y."

```markdown
- **MUST** respond within 200ms
  - **OTHERWISE** return a cached response
  - **OTHERWISE** return 504 Gateway Timeout
```

- **MUST** parse `**OTHERWISE**` as a clause nested under a parent obligation
- **MUST** preserve the ordering of OTHERWISE clauses (they form a degradation chain)
- **MUST** link each OTHERWISE clause to its parent obligation in the clause IR
- **MUST** support multiple OTHERWISE clauses under a single parent (ordered fallback chain)
- **MUST** support OTHERWISE under any obligation keyword (MUST, SHOULD, MUST ALWAYS, MUST BY)
- **SHOULD** inherit the parent's severity unless the OTHERWISE clause specifies its own keyword
- **MUST NOT** allow OTHERWISE at the top level (it must have a parent obligation)
- **MUST NOT** allow OTHERWISE under MAY, WONT, or GIVEN (only under obligations that can be violated)

## Temporal Obligations

Temporal obligations extend MUST with time/state semantics, inspired by temporal deontic logic.

### MUST ALWAYS (Invariant)

An obligation that must hold continuously — across all states, all inputs, all time.

```markdown
- **MUST ALWAYS** keep database connections below pool maximum
- **MUST ALWAYS** return valid JSON from API endpoints
```

- **MUST** parse `**MUST ALWAYS**` as a single compound keyword
- **MUST** assign the `invariant` temporal qualifier to the clause
- **MUST** represent invariants distinctly in the clause IR (they generate different test patterns)

### MUST BY (Deadline)

An obligation that must be fulfilled within a specified time bound.

```markdown
- **MUST BY 5s** return a response
- **MUST BY 100ms** acknowledge the write
- **MUST BY 30s** complete the batch job
```

- **MUST** parse `**MUST BY <duration>**` as a compound keyword with a duration parameter
- **MUST** parse duration suffixes: `ms` (milliseconds), `s` (seconds), `m` (minutes)
- **MUST** store the duration value and unit in the clause IR
- **MUST NOT** accept MUST BY without a duration (it is a parse error)
- **SHOULD** warn if the duration seems unreasonably small (< 1ms) or large (> 1h)

## Clause IR

- **MUST** produce a clause IR struct containing: keyword, severity, clause text, source location (file, line), parent section path, and a stable identifier
- **MUST** generate stable clause identifiers from the section path and clause text (e.g. `auth::login::must_return_jwt`)
- **MUST** generate a content hash for each clause based on keyword + text + relevant context
- **MUST** include a `condition` field populated from the parent GIVEN block (null if unconditional)
- **MUST** include an `otherwise` field containing the ordered list of fallback clauses (empty if none)
- **MUST** include a `temporal` field for MUST ALWAYS (qualifier: invariant) and MUST BY (qualifier: deadline, duration: value+unit)
- **SHOULD** include any code blocks immediately following a clause as "hints" attached to that clause
- **SHOULD** include surrounding prose/markdown in the clause's context field for the LLM

## Cross-File References

- **MUST** parse `requires:` metadata as a list of relative paths to other .ought.md files
- **MUST** parse inline markdown links to other .ought.md files as cross-references
- **MUST** parse anchor links (e.g. `pricing.ought.md#discount-rules`) as references to specific sections
- **MUST** build a dependency graph from cross-file references
- **MUST** detect circular dependencies and report them as errors
- **SHOULD** validate that all cross-references resolve to existing files and sections

## Context Metadata

- **MUST** parse `source:` as a list of file paths or directories (source code hints for the LLM)
- **MUST** parse `schema:` as a list of file paths (schemas, configs, migrations)
- **MUST** parse `context:` as free-text context for the LLM
- **MUST** support multiple values per metadata key (one per line or comma-separated)
- **MAY** support glob patterns in `source:` and `schema:` paths

## Error Handling

- **MUST** report parse errors with the file path, line number, and a clear message
- **MUST** continue parsing after non-fatal errors (collect all errors, don't stop at the first)
- **MUST NOT** crash on malformed markdown — degrade gracefully
- **SHOULD** warn on likely typos (e.g. `**MUTS**` close to a known keyword)
