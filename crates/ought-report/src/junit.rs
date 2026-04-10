use std::collections::HashMap;
use std::path::Path;

use ought_run::RunResult;
use ought_spec::{Clause, Keyword, Spec};

fn keyword_str(kw: Keyword) -> &'static str {
    match kw {
        Keyword::Must => "MUST",
        Keyword::MustNot => "MUST NOT",
        Keyword::Should => "SHOULD",
        Keyword::ShouldNot => "SHOULD NOT",
        Keyword::May => "MAY",
        Keyword::Wont => "WONT",
        Keyword::Given => "GIVEN",
        Keyword::Otherwise => "OTHERWISE",
        Keyword::MustAlways => "MUST ALWAYS",
        Keyword::MustBy => "MUST BY",
    }
}

fn severity_str(kw: Keyword) -> &'static str {
    match kw.severity() {
        ought_spec::Severity::Required => "required",
        ought_spec::Severity::Recommended => "recommended",
        ought_spec::Severity::Optional => "optional",
        ought_spec::Severity::NegativeConfirmation => "negative_confirmation",
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Collect clause info `(id, keyword, text, pending)` from a clause and its
/// otherwise chain.
fn collect_clauses(clauses: &[Clause], out: &mut Vec<(String, Keyword, String, bool)>) {
    for clause in clauses {
        out.push((
            clause.id.0.clone(),
            clause.keyword,
            clause.text.clone(),
            clause.pending,
        ));
        if !clause.otherwise.is_empty() {
            collect_clauses(&clause.otherwise, out);
        }
    }
}

fn collect_clauses_from_section(
    section: &ought_spec::Section,
    out: &mut Vec<(String, Keyword, String, bool)>,
) {
    collect_clauses(&section.clauses, out);
    for sub in &section.subsections {
        collect_clauses_from_section(sub, out);
    }
}

/// Write results as JUnit XML to the given file path.
///
/// Maps spec files to `<testsuite>` and clauses to `<testcase>`.
pub fn report(results: &RunResult, specs: &[Spec], path: &Path) -> anyhow::Result<()> {
    let result_map: HashMap<&str, &ought_run::TestResult> = results
        .results
        .iter()
        .map(|r| (r.clause_id.0.as_str(), r))
        .collect();

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<testsuites>\n");

    for spec in specs {
        let mut clause_infos = Vec::new();
        for section in &spec.sections {
            collect_clauses_from_section(section, &mut clause_infos);
        }

        // Compute counts for this suite. Pending clauses are emitted as
        // `<skipped message="pending"/>` — the standard JUnit convention for
        // deferred tests.
        let mut tests = 0usize;
        let mut failures = 0usize;
        let mut errors = 0usize;
        let mut skipped = 0usize;
        let mut suite_time = 0.0f64;

        for (clause_id, _, _, pending) in &clause_infos {
            if *pending {
                tests += 1;
                skipped += 1;
                continue;
            }
            if let Some(tr) = result_map.get(clause_id.as_str()) {
                tests += 1;
                suite_time += tr.duration.as_secs_f64();
                match tr.status {
                    ought_run::TestStatus::Failed => failures += 1,
                    ought_run::TestStatus::Errored => errors += 1,
                    ought_run::TestStatus::Skipped => skipped += 1,
                    ought_run::TestStatus::Passed => {}
                }
            }
        }

        xml.push_str(&format!(
            "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"{}\" skipped=\"{}\" time=\"{:.3}\">\n",
            xml_escape(&spec.name),
            tests,
            failures,
            errors,
            skipped,
            suite_time,
        ));

        for (clause_id, keyword, text, pending) in &clause_infos {
            if *pending {
                let classname = xml_escape(&spec.name);
                let name = xml_escape(&format!("{} {}", keyword_str(*keyword), text));
                xml.push_str(&format!(
                    "    <testcase classname=\"{}\" name=\"{}\" time=\"0.000\">\n",
                    classname, name,
                ));
                xml.push_str("      <properties>\n");
                xml.push_str(&format!(
                    "        <property name=\"keyword\" value=\"{}\" />\n",
                    xml_escape(keyword_str(*keyword)),
                ));
                xml.push_str(&format!(
                    "        <property name=\"severity\" value=\"{}\" />\n",
                    xml_escape(severity_str(*keyword)),
                ));
                xml.push_str(&format!(
                    "        <property name=\"clause_id\" value=\"{}\" />\n",
                    xml_escape(clause_id),
                ));
                xml.push_str(
                    "        <property name=\"pending\" value=\"true\" />\n",
                );
                xml.push_str("      </properties>\n");
                xml.push_str("      <skipped message=\"pending\" />\n");
                xml.push_str("    </testcase>\n");
                continue;
            }
            if let Some(tr) = result_map.get(clause_id.as_str()) {
                let classname = xml_escape(&spec.name);
                let name = xml_escape(&format!("{} {}", keyword_str(*keyword), text));
                let time = tr.duration.as_secs_f64();

                xml.push_str(&format!(
                    "    <testcase classname=\"{}\" name=\"{}\" time=\"{:.3}\">\n",
                    classname, name, time,
                ));

                // Properties: keyword and severity
                xml.push_str("      <properties>\n");
                xml.push_str(&format!(
                    "        <property name=\"keyword\" value=\"{}\" />\n",
                    xml_escape(keyword_str(*keyword)),
                ));
                xml.push_str(&format!(
                    "        <property name=\"severity\" value=\"{}\" />\n",
                    xml_escape(severity_str(*keyword)),
                ));
                xml.push_str(&format!(
                    "        <property name=\"clause_id\" value=\"{}\" />\n",
                    xml_escape(clause_id),
                ));
                xml.push_str("      </properties>\n");

                match tr.status {
                    ought_run::TestStatus::Failed => {
                        let msg = tr
                            .details
                            .failure_message
                            .as_deref()
                            .or(tr.message.as_deref())
                            .unwrap_or("test failed");
                        xml.push_str(&format!(
                            "      <failure message=\"{}\">{}</failure>\n",
                            xml_escape(msg),
                            xml_escape(msg),
                        ));
                    }
                    ought_run::TestStatus::Errored => {
                        let msg = tr
                            .message
                            .as_deref()
                            .unwrap_or("test errored");
                        xml.push_str(&format!(
                            "      <error message=\"{}\">{}</error>\n",
                            xml_escape(msg),
                            xml_escape(msg),
                        ));
                    }
                    ought_run::TestStatus::Skipped => {
                        xml.push_str("      <skipped />\n");
                    }
                    ought_run::TestStatus::Passed => {}
                }

                xml.push_str("    </testcase>\n");
            }
        }

        xml.push_str("  </testsuite>\n");
    }

    xml.push_str("</testsuites>\n");

    std::fs::write(path, xml)?;
    Ok(())
}
