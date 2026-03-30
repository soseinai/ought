#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

use ought_run::{RunResult, TestStatus};
use ought_spec::{Clause, Keyword, Section, Severity, Spec, Temporal};

use crate::types::{ColorChoice, ReportOptions};

// ANSI escape codes
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const GRAY: &str = "\x1b[90m";

/// Whether to emit ANSI color codes.
fn use_color(options: &ReportOptions) -> bool {
    match options.color {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => atty_stdout(),
    }
}

/// Simple TTY detection for stdout.
fn atty_stdout() -> bool {
    std::io::stdout().is_terminal()
}

struct Painter {
    color: bool,
}

impl Painter {
    fn new(color: bool) -> Self {
        Self { color }
    }

    fn style(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("{}{}{}", code, text, RESET)
        } else {
            text.to_string()
        }
    }

    fn green(&self, text: &str) -> String {
        self.style(GREEN, text)
    }

    fn red(&self, text: &str) -> String {
        self.style(RED, text)
    }

    fn yellow(&self, text: &str) -> String {
        self.style(YELLOW, text)
    }

    fn dim(&self, text: &str) -> String {
        self.style(DIM, text)
    }

    fn gray(&self, text: &str) -> String {
        self.style(GRAY, text)
    }

    fn bold(&self, text: &str) -> String {
        self.style(BOLD, text)
    }

    fn bold_red(&self, text: &str) -> String {
        if self.color {
            format!("{}{}{}{}", BOLD, RED, text, RESET)
        } else {
            text.to_string()
        }
    }

    fn bold_yellow(&self, text: &str) -> String {
        if self.color {
            format!("{}{}{}{}", BOLD, YELLOW, text, RESET)
        } else {
            text.to_string()
        }
    }
}

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

/// Status icon for a test result.
fn status_icon(status: TestStatus, keyword: Keyword) -> &'static str {
    match (status, keyword) {
        (TestStatus::Passed, Keyword::Wont) => "\u{2298}", // ⊘
        (TestStatus::Passed, _) => "\u{2713}",             // ✓
        (TestStatus::Failed, _) => "\u{2717}",             // ✗
        (TestStatus::Errored, _) => "!",
        (TestStatus::Skipped, _) => "~",
    }
}

fn format_duration(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{}ms", ms)
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}

/// Render results to the terminal with colors, status indicators,
/// and clause-level detail.
pub fn report(
    results: &RunResult,
    specs: &[Spec],
    options: &ReportOptions,
) -> anyhow::Result<()> {
    let color = use_color(options);
    let p = Painter::new(color);
    let mut out = io::stdout().lock();

    // Build lookup from clause_id -> TestResult
    let result_map: HashMap<&str, &ought_run::TestResult> = results
        .results
        .iter()
        .map(|r| (r.clause_id.0.as_str(), r))
        .collect();

    // Counters for summary
    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut total_errored = 0usize;
    let mut total_skipped = 0usize;
    let mut confirmed_absent = 0usize;
    let mut must_total = 0usize;
    let mut must_passed = 0usize;

    writeln!(out)?;
    writeln!(out, " {}", p.bold("ought run"))?;

    for spec in specs {
        writeln!(out)?;

        // Spec header: name + source file
        let source_name = spec
            .source_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        writeln!(
            out,
            " {}          {}",
            p.bold(&spec.name),
            p.dim(&source_name),
        )?;
        writeln!(out, " {}", p.dim(&"\u{2500}".repeat(44)))?;

        for section in &spec.sections {
            render_section(
                &mut out,
                &p,
                section,
                &result_map,
                1,
                &mut total_passed,
                &mut total_failed,
                &mut total_errored,
                &mut total_skipped,
                &mut confirmed_absent,
                &mut must_total,
                &mut must_passed,
            )?;
        }
    }

    // Summary line
    writeln!(out)?;

    let mut summary_parts = Vec::new();
    if total_passed > 0 {
        summary_parts.push(p.green(&format!("{} passed", total_passed)));
    }
    if total_failed > 0 {
        summary_parts.push(p.red(&format!("{} failed", total_failed)));
    }
    if total_errored > 0 {
        summary_parts.push(p.red(&format!("{} errored", total_errored)));
    }
    if confirmed_absent > 0 {
        summary_parts.push(p.dim(&format!("{} confirmed absent", confirmed_absent)));
    }
    if total_skipped > 0 {
        summary_parts.push(p.dim(&format!("{} skipped", total_skipped)));
    }

    writeln!(out, " {}", summary_parts.join(" \u{00b7} "))?;

    // MUST coverage
    if must_total > 0 {
        let pct = (must_passed as f64 / must_total as f64) * 100.0;
        let coverage_str = format!(
            " MUST coverage: {}/{} ({:.0}%)",
            must_passed, must_total, pct
        );
        if must_passed == must_total {
            writeln!(out, "{}", p.green(&coverage_str))?;
        } else {
            writeln!(out, "{}", p.red(&coverage_str))?;
        }
    }

    writeln!(out)?;
    out.flush()?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_section(
    out: &mut impl Write,
    p: &Painter,
    section: &Section,
    result_map: &HashMap<&str, &ought_run::TestResult>,
    depth: usize,
    total_passed: &mut usize,
    total_failed: &mut usize,
    total_errored: &mut usize,
    total_skipped: &mut usize,
    confirmed_absent: &mut usize,
    must_total: &mut usize,
    must_passed: &mut usize,
) -> anyhow::Result<()> {
    let indent = " ".repeat(depth);

    // Section header
    writeln!(out, "{}{}", indent, p.bold(&section.title))?;

    // Render clauses in this section
    for clause in &section.clauses {
        render_clause(
            out,
            p,
            clause,
            result_map,
            depth + 1,
            false, // not an otherwise
            total_passed,
            total_failed,
            total_errored,
            total_skipped,
            confirmed_absent,
            must_total,
            must_passed,
        )?;
    }

    // Render subsections
    for sub in &section.subsections {
        render_section(
            out,
            p,
            sub,
            result_map,
            depth + 1,
            total_passed,
            total_failed,
            total_errored,
            total_skipped,
            confirmed_absent,
            must_total,
            must_passed,
        )?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_clause(
    out: &mut impl Write,
    p: &Painter,
    clause: &Clause,
    result_map: &HashMap<&str, &ought_run::TestResult>,
    depth: usize,
    is_otherwise: bool,
    total_passed: &mut usize,
    total_failed: &mut usize,
    total_errored: &mut usize,
    total_skipped: &mut usize,
    confirmed_absent: &mut usize,
    must_total: &mut usize,
    must_passed: &mut usize,
) -> anyhow::Result<()> {
    let indent = "  ".repeat(depth);

    // Handle GIVEN blocks: render as a group header
    if clause.keyword == Keyword::Given {
        // Check if any nested clause failed
        let any_failed = has_failure(clause, result_map);

        let given_line = format!("GIVEN {}:", &clause.text);
        if any_failed {
            writeln!(out, "{}{}", indent, p.bold(&given_line))?;
        } else {
            writeln!(out, "{}{}", indent, p.dim(&given_line))?;
        }

        // Render nested clauses (clauses with condition matching this GIVEN)
        // In the spec model, GIVEN clauses contain their children in `otherwise`.
        // But actually, GIVEN blocks likely use the condition field on child clauses.
        // Since GIVEN is itself a clause, let's check if there are otherwise entries
        // that represent the nested clauses.
        for child in &clause.otherwise {
            render_clause(
                out,
                p,
                child,
                result_map,
                depth + 1,
                false,
                total_passed,
                total_failed,
                total_errored,
                total_skipped,
                confirmed_absent,
                must_total,
                must_passed,
            )?;
        }
        return Ok(());
    }

    // Look up the test result for this clause
    let tr = result_map.get(clause.id.0.as_str());

    // Track MUST keyword counts
    let is_must = matches!(
        clause.keyword,
        Keyword::Must | Keyword::MustNot | Keyword::MustAlways | Keyword::MustBy
    );

    if let Some(tr) = tr {
        if is_must {
            *must_total += 1;
        }

        match tr.status {
            TestStatus::Passed => {
                *total_passed += 1;
                if clause.keyword == Keyword::Wont {
                    *confirmed_absent += 1;
                }
                if is_must {
                    *must_passed += 1;
                }
            }
            TestStatus::Failed => *total_failed += 1,
            TestStatus::Errored => *total_errored += 1,
            TestStatus::Skipped => *total_skipped += 1,
        }
    }

    let status = tr.map(|r| r.status).unwrap_or(TestStatus::Skipped);
    let icon = status_icon(status, clause.keyword);
    let kw = keyword_str(clause.keyword);

    // Build the suffix (timing, iterations, etc.)
    let mut suffix = String::new();

    // MUST BY: show measured vs deadline
    if let Some(Temporal::Deadline(deadline)) = &clause.temporal
        && let Some(tr) = tr
            && let Some(measured) = tr.details.measured_duration {
                suffix = format!(
                    "  [{}{}/ {}]",
                    format_duration(measured),
                    " ",
                    format_duration(*deadline),
                );
            }

    // MUST ALWAYS: show iteration count
    if let Some(Temporal::Invariant) = &clause.temporal
        && let Some(tr) = tr
            && let Some(iters) = tr.details.iterations {
                suffix = format!("  (tested {} inputs)", iters);
            }

    // WONT passed: show "confirmed absent"
    if clause.keyword == Keyword::Wont && status == TestStatus::Passed {
        suffix = "  (confirmed absent)".to_string();
    }

    // Build the otherwise prefix
    let prefix = if is_otherwise { "\u{21b3} " } else { "" };

    // Format the line
    let keyword_padded = format!("{:<6}", kw);
    let clause_line = format!(
        "{}{} {} {}{}",
        prefix, icon, keyword_padded, &clause.text, suffix
    );

    // Apply color based on status and severity
    let colored_line = match status {
        TestStatus::Passed => {
            if clause.keyword == Keyword::Wont {
                // Confirmed absent: use dim
                p.dim(&clause_line)
            } else {
                // Pass: dim the line, green the icon
                let icon_colored = p.green(icon);
                let rest = format!(
                    "{}{} {}{}",
                    prefix, keyword_padded, &clause.text, suffix
                );
                format!("{}{} {}", indent, icon_colored, p.dim(&rest))
            }
        }
        TestStatus::Failed => {
            match clause.keyword.severity() {
                Severity::Required => {
                    // MUST failure: bold + red
                    format!("{}{}", indent, p.bold_red(&clause_line))
                }
                Severity::Recommended => {
                    // SHOULD failure: bold + yellow
                    format!("{}{}", indent, p.bold_yellow(&clause_line))
                }
                _ => {
                    // MAY/other failure
                    format!("{}{}", indent, p.yellow(&clause_line))
                }
            }
        }
        TestStatus::Errored => format!("{}{}", indent, p.bold_red(&clause_line)),
        TestStatus::Skipped => {
            if is_otherwise {
                // Skipped otherwise: show as "not reached"
                let line = format!(
                    "{}\u{21b3} ~ {} {}  (not reached)",
                    indent, kw, &clause.text,
                );
                p.dim(&line)
            } else {
                format!("{}{}", indent, p.dim(&clause_line))
            }
        }
    };

    // Write with proper indenting (if we didn't already add it above)
    if colored_line.starts_with(&indent) {
        writeln!(out, "{}", colored_line)?;
    } else {
        writeln!(out, "{}{}", indent, colored_line)?;
    }

    // Show failure details
    if let Some(tr) = tr
        && (status == TestStatus::Failed || status == TestStatus::Errored)
        && let Some(msg) = tr.details.failure_message.as_deref().or(tr.message.as_deref())
    {
        let detail_indent = format!("{}    ", indent);
        for line in msg.lines() {
            writeln!(out, "{}{}", detail_indent, p.red(line))?;
        }
    }

    // Render OTHERWISE chain
    for otherwise_clause in &clause.otherwise {
        render_clause(
            out,
            p,
            otherwise_clause,
            result_map,
            depth + 1,
            true,
            total_passed,
            total_failed,
            total_errored,
            total_skipped,
            confirmed_absent,
            must_total,
            must_passed,
        )?;
    }

    Ok(())
}

/// Check if any clause or its otherwise chain has a failure.
fn has_failure(clause: &Clause, result_map: &HashMap<&str, &ought_run::TestResult>) -> bool {
    if let Some(tr) = result_map.get(clause.id.0.as_str())
        && (tr.status == TestStatus::Failed || tr.status == TestStatus::Errored) {
            return true;
        }
    for child in &clause.otherwise {
        if has_failure(child, result_map) {
            return true;
        }
    }
    false
}
