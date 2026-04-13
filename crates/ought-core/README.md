# ought-core

Shared cross-cutting types used by multiple crates in the ought workspace.

This is the lowest-level crate in the workspace — it depends on nothing else
from ought and is depended on by any crate that needs a genuinely shared type.
Crate-specific types belong in their owning crate; only things that legitimately
cross boundaries live here.

## Responsibilities

- Hold small, stable data types that would otherwise force unrelated crates to
  depend on each other just to name a struct.
- Stay minimal. Most new types belong in a domain crate first; they graduate
  here only when a second crate needs them.

## Notable public API

- `ContextConfig` — project source-context settings (`search_paths`, `exclude`,
  `max_files`). Consumed by the CLI (watch, survey) and available to any
  future subsystem that reasons about project source layout.
