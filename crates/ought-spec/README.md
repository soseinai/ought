# ought-spec

Parser and clause IR for the `.ought.md` spec format.

This crate defines the open standard for ought specs: the grammar (see
`docs/grammar.md`), the parsed data types, and the walker that turns a
directory tree of spec files into a queryable graph. It has zero LLM
dependencies — any tool can consume specs by depending on this crate alone.

## Responsibilities

- Parse `.ought.md` files into structured `Spec`/`Section`/`Clause` values.
- Compute stable clause IDs and content hashes used by the rest of the stack.
- Walk a set of roots and build a `SpecGraph` with cross-file references
  resolved.
- Own `SpecsConfig` — the `[specs]` section of `ought.toml` (spec roots),
  since spec discovery is this crate's concern.

## Notable public API

- `trait Parser` — the public interface for parsing. Methods:
  `parse_file(&self, path)` (with a default impl that reads the file and
  delegates to `parse_string`), `parse_string(&self, content, path)`, and
  `name(&self)`. Most consumers should take `&dyn Parser` (or
  `impl Parser`) rather than a concrete type.
- `OughtMdParser` — the canonical implementation for `.ought.md` files.
  Zero-state unit struct, so `OughtMdParser.parse_file(&path)` is the
  usual call pattern. Pure Rust, no LLM dependency.
- `SpecGraph::from_roots(&roots)` — convenience that parses every spec
  under a set of roots with the default `OughtMdParser`.
- `SpecGraph::from_roots_with(&dyn Parser, &roots)` — inject a custom
  parser; useful for tests and for future alternate spec formats.
- Types: `Spec`, `Section`, `Clause`, `ClauseId`, `Keyword`, `Severity`,
  `Temporal`, `SourceLocation`, `Metadata`, `SpecRef`, `ParseError`.
- `SpecsConfig { roots }` — deserializable sub-config composed by the
  aggregate `Config` in `ought-cli`.
