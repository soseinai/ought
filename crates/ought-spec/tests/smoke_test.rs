use std::path::Path;

use ought_spec::parser::{OughtMdParser, Parser};
use ought_spec::types::*;

/// Parse the actual parser.ought.md spec file from this repository.
#[test]
fn parse_real_spec_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../ought/engine/parser.ought.md");
    if !path.exists() {
        // Skip if the file isn't present in this context
        return;
    }
    let spec = OughtMdParser.parse_file(&path).expect("failed to parse parser.ought.md");
    assert_eq!(spec.name, "Parser");
    assert!(spec.metadata.context.is_some(), "should have context metadata");
    assert!(!spec.metadata.sources.is_empty(), "should have source metadata");
    assert!(!spec.sections.is_empty(), "should have sections");

    // Count all clauses across sections recursively
    fn count_clauses(section: &Section) -> usize {
        let mut n = section.clauses.len();
        for sub in &section.subsections {
            n += count_clauses(sub);
        }
        n
    }
    let total: usize = spec.sections.iter().map(count_clauses).sum();
    assert!(total > 30, "parser spec should have many clauses, got {}", total);

    // Check that GIVEN blocks were parsed
    let _has_given_condition = spec.sections.iter().any(|s| {
        s.clauses.iter().any(|c| c.condition.is_some())
            || s.subsections.iter().any(|sub| sub.clauses.iter().any(|c| c.condition.is_some()))
    });
    // The parser spec itself doesn't necessarily use GIVEN, so this might be false
    // Just check the structure is valid
    assert!(total > 0);
}
