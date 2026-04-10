use std::collections::HashMap;

use serde_json::{Value, json};

use ought_spec::{Clause, Keyword, Section, Spec};

use crate::proofs::{Proof, ProofIndex};

// ─── JSON serialization ────────────────────────────────────────────────────

pub(crate) fn spec_to_json(spec: &Spec, proofs: &ProofIndex) -> Value {
    json!({
        "name": spec.name,
        "source_path": spec.source_path.display().to_string(),
        "metadata": {
            "context": spec.metadata.context,
            "sources": spec.metadata.sources,
            "schemas": spec.metadata.schemas,
            "requires": spec.metadata.requires.iter().map(|r| json!({
                "label": r.label,
                "path": r.path.display().to_string(),
                "anchor": r.anchor,
            })).collect::<Vec<_>>(),
        },
        "sections": spec.sections.iter().map(|s| section_to_json(s, proofs)).collect::<Vec<_>>(),
    })
}

pub(crate) fn section_to_json(section: &Section, proofs: &ProofIndex) -> Value {
    json!({
        "title": section.title,
        "depth": section.depth,
        "prose": section.prose,
        "clauses": section.clauses.iter().map(|c| clause_to_json(c, proofs)).collect::<Vec<_>>(),
        "subsections": section.subsections.iter().map(|s| section_to_json(s, proofs)).collect::<Vec<_>>(),
    })
}

pub(crate) fn clause_to_json(clause: &Clause, proofs: &ProofIndex) -> Value {
    let temporal = clause.temporal.as_ref().map(|t| match t {
        ought_spec::Temporal::Invariant => json!({ "kind": "invariant" }),
        ought_spec::Temporal::Deadline(dur) => json!({ "kind": "deadline", "duration": format!("{:?}", dur) }),
    });

    let proofs_json = proofs
        .by_clause
        .get(&clause.id.0)
        .map(|(path, tests)| {
            json!({
                "file": path.display().to_string(),
                "tests": tests.iter().map(proof_to_json).collect::<Vec<_>>(),
            })
        })
        .unwrap_or_else(|| json!({ "file": null, "tests": [] }));

    json!({
        "id": clause.id.0,
        "keyword": format!("{:?}", clause.keyword),
        "severity": format!("{:?}", clause.severity),
        "text": clause.text,
        "condition": clause.condition,
        "otherwise": clause.otherwise.iter().map(|c| clause_to_json(c, proofs)).collect::<Vec<_>>(),
        "temporal": temporal,
        "hints": clause.hints,
        "proofs": proofs_json,
        "pending": clause.pending,
    })
}

fn proof_to_json(proof: &Proof) -> Value {
    json!({
        "name": proof.name,
        "summary": proof.summary,
        "code": proof.code,
        "language": proof.language,
    })
}

fn keyword_display(kw: &Keyword) -> &'static str {
    match kw {
        Keyword::Must => "Must",
        Keyword::MustNot => "MustNot",
        Keyword::Should => "Should",
        Keyword::ShouldNot => "ShouldNot",
        Keyword::May => "May",
        Keyword::Wont => "Wont",
        Keyword::Given => "Given",
        Keyword::Otherwise => "Otherwise",
        Keyword::MustAlways => "MustAlways",
        Keyword::MustBy => "MustBy",
    }
}

pub(crate) fn count_clauses(sections: &[Section]) -> usize {
    sections
        .iter()
        .map(|s| {
            s.clauses.len()
                + s.clauses.iter().map(|c| c.otherwise.len()).sum::<usize>()
                + count_clauses(&s.subsections)
        })
        .sum()
}

pub(crate) fn count_sections(sections: &[Section]) -> usize {
    sections
        .iter()
        .map(|s| 1 + count_sections(&s.subsections))
        .sum()
}

pub(crate) fn count_by_keyword(sections: &[Section], counts: &mut HashMap<&'static str, usize>) {
    for section in sections {
        for clause in &section.clauses {
            *counts.entry(keyword_display(&clause.keyword)).or_insert(0) += 1;
            for ow in &clause.otherwise {
                *counts.entry(keyword_display(&ow.keyword)).or_insert(0) += 1;
            }
        }
        count_by_keyword(&section.subsections, counts);
    }
}

pub(crate) fn build_api_response(specs: &[Spec], proofs: &ProofIndex) -> Value {
    let total_specs = specs.len();
    let total_sections: usize = specs.iter().map(|s| count_sections(&s.sections)).sum();
    let total_clauses: usize = specs.iter().map(|s| count_clauses(&s.sections)).sum();

    let mut by_keyword: HashMap<&str, usize> = HashMap::new();
    for spec in specs {
        count_by_keyword(&spec.sections, &mut by_keyword);
    }

    json!({
        "specs": specs.iter().map(|s| spec_to_json(s, proofs)).collect::<Vec<_>>(),
        "stats": {
            "total_specs": total_specs,
            "total_sections": total_sections,
            "total_clauses": total_clauses,
            "by_keyword": by_keyword,
            "total_proofs": proofs.proof_count(),
            "clauses_with_proofs": proofs.clause_count(),
        },
    })
}
