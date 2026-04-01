use std::collections::HashMap;

use serde_json::{Value, json};

use ought_spec::{Clause, Section, Spec};

/// A flattened, searchable clause entry.
#[derive(Clone)]
struct IndexedClause {
    clause_id: String,
    keyword: String,
    text: String,
    condition: Option<String>,
    spec_name: String,
    section_path: String,
    temporal: Option<Value>,
    /// All searchable text concatenated and lowercased for matching.
    search_text: String,
    /// Individual tokens for scoring.
    tokens: Vec<String>,
}

/// In-memory search index over all spec clauses.
pub struct SearchIndex {
    clauses: Vec<IndexedClause>,
    /// Inverted index: token → list of clause indices.
    inverted: HashMap<String, Vec<usize>>,
}

impl SearchIndex {
    /// Build the search index from parsed specs.
    pub fn build(specs: &[Spec]) -> Self {
        let mut clauses = Vec::new();

        for spec in specs {
            collect_indexed_clauses(&spec.sections, &spec.name, &spec.name, &mut clauses);
        }

        // Build inverted index.
        let mut inverted: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, clause) in clauses.iter().enumerate() {
            for token in &clause.tokens {
                inverted.entry(token.clone()).or_default().push(idx);
            }
        }

        Self { clauses, inverted }
    }

    /// Number of clauses in the index.
    pub fn clause_count(&self) -> usize {
        self.clauses.len()
    }

    /// Search for clauses matching the query. Returns ranked results as JSON.
    pub fn search(&self, query: &str, limit: usize) -> Value {
        let query_lower = query.to_lowercase();
        let query_tokens = tokenize(&query_lower);

        if query_tokens.is_empty() {
            return json!({ "query": query, "results": [], "total": 0 });
        }

        // Score each clause by how many query tokens match and how well.
        let mut scored: Vec<(usize, f64)> = Vec::new();

        // Gather candidate clauses from the inverted index.
        let mut candidates: HashMap<usize, f64> = HashMap::new();

        for token in &query_tokens {
            // Exact token match.
            if let Some(indices) = self.inverted.get(token.as_str()) {
                for &idx in indices {
                    *candidates.entry(idx).or_insert(0.0) += 2.0;
                }
            }
            // Prefix match (for partial typing).
            for (indexed_token, indices) in &self.inverted {
                if indexed_token.starts_with(token.as_str()) && indexed_token != token {
                    for &idx in indices {
                        *candidates.entry(idx).or_insert(0.0) += 1.0;
                    }
                }
            }
        }

        // Boost for substring match in full text.
        for (&idx, score) in &mut candidates {
            let clause = &self.clauses[idx];
            if clause.search_text.contains(&query_lower) {
                *score += 5.0;
            }
            // Boost clause ID matches.
            if clause.clause_id.to_lowercase().contains(&query_lower) {
                *score += 3.0;
            }
            // Boost keyword matches.
            if clause.keyword.to_lowercase().contains(&query_lower) {
                *score += 2.0;
            }
        }

        scored.extend(candidates);
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let total = scored.len();
        let results: Vec<Value> = scored
            .into_iter()
            .take(limit)
            .map(|(idx, score)| {
                let clause = &self.clauses[idx];
                let highlight = build_highlight(&clause.text, &query_tokens);
                json!({
                    "clause_id": clause.clause_id,
                    "keyword": clause.keyword,
                    "text": clause.text,
                    "spec_name": clause.spec_name,
                    "section_path": clause.section_path,
                    "condition": clause.condition,
                    "temporal": clause.temporal,
                    "score": (score * 100.0).round() / 100.0,
                    "highlight": highlight,
                })
            })
            .collect();

        json!({
            "query": query,
            "results": results,
            "total": total,
        })
    }
}

/// Recursively collect all clauses (including otherwise) into the flat index.
fn collect_indexed_clauses(
    sections: &[Section],
    spec_name: &str,
    parent_path: &str,
    out: &mut Vec<IndexedClause>,
) {
    for section in sections {
        let section_path = format!("{} > {}", parent_path, section.title);
        for clause in &section.clauses {
            add_clause(clause, spec_name, &section_path, out);
            for ow in &clause.otherwise {
                add_clause(ow, spec_name, &section_path, out);
            }
        }
        collect_indexed_clauses(&section.subsections, spec_name, &section_path, out);
    }
}

fn add_clause(clause: &Clause, spec_name: &str, section_path: &str, out: &mut Vec<IndexedClause>) {
    let keyword = format!("{:?}", clause.keyword);
    let temporal = clause.temporal.as_ref().map(|t| match t {
        ought_spec::Temporal::Invariant => json!({ "kind": "invariant" }),
        ought_spec::Temporal::Deadline(dur) => json!({ "kind": "deadline", "duration": format!("{:?}", dur) }),
    });

    // Build the searchable text from all fields.
    let mut search_parts = vec![
        clause.id.0.to_lowercase(),
        keyword.to_lowercase(),
        clause.text.to_lowercase(),
        spec_name.to_lowercase(),
        section_path.to_lowercase(),
    ];
    if let Some(ref cond) = clause.condition {
        search_parts.push(cond.to_lowercase());
    }
    for hint in &clause.hints {
        search_parts.push(hint.to_lowercase());
    }
    let search_text = search_parts.join(" ");
    let tokens = tokenize(&search_text);

    out.push(IndexedClause {
        clause_id: clause.id.0.clone(),
        keyword,
        text: clause.text.clone(),
        condition: clause.condition.clone(),
        spec_name: spec_name.to_string(),
        section_path: section_path.to_string(),
        temporal,
        search_text,
        tokens,
    });
}

/// Tokenize text into lowercase alphanumeric words.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_lowercase())
        .collect()
}

/// Build a highlight snippet showing where query terms appear in the text.
fn build_highlight(text: &str, query_tokens: &[String]) -> String {
    let text_lower = text.to_lowercase();
    let mut result = String::new();
    let mut last_end = 0;

    // Find positions of all query token matches.
    let mut matches: Vec<(usize, usize)> = Vec::new();
    for token in query_tokens {
        let mut start = 0;
        while let Some(pos) = text_lower[start..].find(token.as_str()) {
            let abs_pos = start + pos;
            matches.push((abs_pos, abs_pos + token.len()));
            start = abs_pos + 1;
        }
    }

    if matches.is_empty() {
        // No direct matches — return the whole text.
        return text.to_string();
    }

    // Sort and merge overlapping matches.
    matches.sort_by_key(|m| m.0);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for m in matches {
        if let Some(last) = merged.last_mut()
            && m.0 <= last.1 {
                last.1 = last.1.max(m.1);
                continue;
            }
        merged.push(m);
    }

    // Build highlighted string.
    for (start, end) in merged {
        result.push_str(&text[last_end..start]);
        result.push_str("<mark>");
        result.push_str(&text[start..end]);
        result.push_str("</mark>");
        last_end = end;
    }
    result.push_str(&text[last_end..]);
    result
}
