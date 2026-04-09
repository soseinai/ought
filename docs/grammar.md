# Ought Spec Grammar

The `.ought.md` format is CommonMark markdown with a layered grammar on top.
This document specifies that grammar in EBNF notation.

## Notation

```
=       definition
|       alternation
,       concatenation
[ ]     optional
{ }     repetition (zero or more)
( )     grouping
"..."   terminal string
'...'   terminal string (single-quoted)
(*...*) comment
```

## Grammar

```ebnf
(* ─── Top-level structure ─────────────────────────────────────── *)

spec            = h1_title , [ metadata_block ] , { section } ;

h1_title        = "# " , text_line , newline ;

metadata_block  = { metadata_line | blank_line } ;
                  (* ends at the first H2+ heading *)

metadata_line   = context_line
                | source_line
                | schema_line
                | requires_line ;

context_line    = "context:" , whitespace , text_line , newline ;
source_line     = "source:" , whitespace , value_list , newline ;
schema_line     = "schema:" , whitespace , value_list , newline ;
requires_line   = "requires:" , whitespace , ref_list , newline ;

value_list      = value , { "," , value } ;
value           = glob_pattern | file_path ;

ref_list        = spec_ref , { "," , spec_ref } ;
spec_ref        = md_link | file_path ;
md_link         = "[" , label , "]" , "(" , file_path , [ anchor ] , ")" ;
anchor          = "#" , identifier ;
label           = text ;

(* ─── Sections ────────────────────────────────────────────────── *)

section         = heading , [ prose ] , { clause_list } , { section } ;

heading         = heading_marker , whitespace , text_line , newline ;
heading_marker  = "##" | "###" | "####" | "#####" | "######" ;
                  (* depth derived from marker length *)

prose           = { paragraph | code_block | blank_line } ;
                  (* any markdown content that is not a clause list *)

paragraph       = text_line , { text_line } , newline ;

(* ─── Clause lists ────────────────────────────────────────────── *)

clause_list     = { clause_item | given_block | prose_item } ;

clause_item     = "- " , bold_keyword , whitespace , clause_text , newline ,
                  { otherwise_item } ,
                  [ hint_block ] ;

given_block     = "- " , bold_given , whitespace , condition_text , newline ,
                  indent , given_body ;

given_body      = { nested_clause | nested_given | prose_item } ;

nested_clause   = indent , "- " , bold_keyword , whitespace , clause_text , newline ,
                  { nested_otherwise } ,
                  [ hint_block ] ;

nested_given    = indent , "- " , bold_given , whitespace , condition_text , newline ,
                  indent , indent , given_body ;

otherwise_item  = indent , "- " , bold_otherwise , whitespace , clause_text , newline ,
                  { nested_otherwise } ;

nested_otherwise = indent , indent , "- " , bold_otherwise , whitespace , clause_text , newline ;

prose_item      = "- " , text_line , newline ;
                  (* list item without a bold keyword — treated as prose *)

hint_block      = code_block ;
                  (* code block immediately following a clause is attached as a hint *)

code_block      = "```" , [ language ] , newline , { text_line , newline } , "```" , newline ;
language        = identifier ;

(* ─── Keywords ────────────────────────────────────────────────── *)

bold_keyword    = "**" , keyword , "**" ;
bold_given      = "**" , "GIVEN" , "**" ;
bold_otherwise  = "**" , "OTHERWISE" , "**" ;

keyword         = obligation | permission | negative ;

obligation      = "MUST"
                | "MUST NOT"
                | "SHOULD"
                | "SHOULD NOT"
                | "MUST ALWAYS"
                | "MUST BY" , whitespace , duration ;

permission      = "MAY" ;

negative        = "WONT" ;

(* ─── Duration (for MUST BY) ──────────────────────────────────── *)

duration        = integer , duration_unit ;
duration_unit   = "ms" | "s" | "m" ;
integer         = digit , { digit } ;

(* ─── Terminals ───────────────────────────────────────────────── *)

clause_text     = text ;
                  (* the human-readable description of the requirement *)

condition_text  = text ;
                  (* the precondition description for GIVEN *)

text_line       = { character } ;
text            = { character } ;
identifier      = letter , { letter | digit | "_" | "-" } ;
file_path       = { character } ;
                  (* relative path, may include globs *)
glob_pattern    = { character } ;
                  (* e.g. src/**/*.rs *)

whitespace      = " " , { " " } ;
indent          = "  " ;
                  (* two spaces per nesting level *)
newline         = "\n" ;
blank_line      = newline ;
character       = ? any Unicode character except newline ? ;
letter          = ? Unicode letter ? ;
digit           = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" ;
```

## Structural constraints

These constraints are not expressible in EBNF but are enforced by the parser:

1. **OTHERWISE requires a parent obligation.** An OTHERWISE clause must be
   nested (indented) under a MUST, MUST NOT, SHOULD, SHOULD NOT, MUST ALWAYS,
   or MUST BY clause. It cannot appear at the top level or under MAY, WONT,
   or GIVEN.

2. **GIVEN scopes its children.** All clauses nested under a GIVEN inherit
   that condition. GIVEN itself is not testable — it produces no test.

3. **OTHERWISE chains are ordered.** Multiple OTHERWISE clauses under the same
   parent form a degradation ladder, evaluated in document order.

4. **Sections nest by heading depth.** An H3 under an H2 becomes a subsection.
   An H2 following an H3 closes the H3 and sits alongside the prior H2.

5. **Metadata appears only between H1 and the first H2.** Metadata lines
   after the first section heading are treated as prose.

6. **Hint blocks bind to the preceding clause.** A code block immediately
   after a clause (with no intervening prose or blank heading) is attached
   as a generation hint, not treated as prose.

7. **Keywords must be bold.** A list item starting with `MUST` (not `**MUST**`)
   is treated as prose, not as a clause.

8. **One H1 per file.** The first H1 defines the spec name. Subsequent H1s
   are ignored for naming but parsed normally.

## Example

```markdown
# Authentication API

context: REST API at `/api/auth`, uses JWT tokens
source: src/auth/
requires: [users](./users.ought.md)

## Login

Handles credential validation and token issuance.

- **MUST** return a valid JWT token when given correct credentials
- **MUST NOT** leak timing differences between valid and invalid usernames
- **MUST BY 200ms** return a response under normal load
  - **OTHERWISE** return a cached session token
  - **OTHERWISE** return 503 with a Retry-After header
- **SHOULD** rate-limit to 5 attempts per minute per IP
- **MAY** support "remember me" extended token expiry
- **WONT** support basic auth

## Session Management

- **GIVEN** the user has a valid session
  - **MUST** refresh the token on each request
  - **MUST ALWAYS** include the CSRF token in responses
- **GIVEN** the session has expired
  - **MUST** return 401
  - **SHOULD** include a `reason` field in the response body
```
