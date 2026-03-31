pub mod resources;
pub mod server;
pub mod tools;

use ought_spec::{Clause, Section};

/// Recursively collect all clauses from a section and its subsections.
pub fn collect_clauses(section: &Section) -> Vec<&Clause> {
    let mut result: Vec<&Clause> = section.clauses.iter().collect();
    for clause in &section.clauses {
        for otherwise in &clause.otherwise {
            result.push(otherwise);
        }
    }
    for sub in &section.subsections {
        result.extend(collect_clauses(sub));
    }
    result
}

/// Count all clauses in a spec's sections.
pub fn count_clauses(sections: &[Section]) -> usize {
    sections.iter().map(|s| collect_clauses(s).len()).sum()
}
