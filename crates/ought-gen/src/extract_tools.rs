//! Tool primitives for spec extraction.
//!
//! Plain sync functions over plain types, mirroring the shape of
//! [`crate::tools`]. The [`crate::extract_tool_set`] adapter wraps these
//! as async tools for the in-process agent loop.
//!
//! Load-bearing semantics (prompts depend on these):
//!
//! * `validate_spec` parses with the canonical `OughtMdParser`; anything
//!   that doesn't parse is rejected.
//! * `write_spec` re-validates before touching disk, refuses paths
//!   outside `specs_root`, and under `force=false` refuses to overwrite
//!   existing files.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use ought_spec::parser::{OughtMdParser, Parser as _};

use crate::extract::ExtractAssignment;

// ── Output types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateSpecOutput {
    pub ok: bool,
    /// When `ok` is false, one formatted error per parse failure.
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WriteSpecOutput {
    Written {
        target_path: String,
        resolved_path: String,
        bytes: u64,
    },
    DryRun {
        target_path: String,
        resolved_path: String,
    },
    SkippedExists {
        target_path: String,
        reason: String,
    },
    Rejected {
        target_path: String,
        errors: Vec<String>,
    },
}

// ── Primitives ──────────────────────────────────────────────────────────

/// Return the agent's assignment.
pub fn get_assignment(assignment: &ExtractAssignment) -> ExtractAssignment {
    assignment.clone()
}

/// Parse a draft `.ought.md` spec without touching disk. Returns a
/// structured summary of parse errors for the agent to react to.
pub fn validate_spec(content: &str) -> ValidateSpecOutput {
    match parse_spec_content(content) {
        Ok(()) => ValidateSpecOutput {
            ok: true,
            errors: Vec::new(),
        },
        Err(errors) => ValidateSpecOutput {
            ok: false,
            errors: errors
                .into_iter()
                .map(|e| format!("line {}: {}", e.line, e.message))
                .collect(),
        },
    }
}

/// Write a validated spec to disk, or preview under `dry_run`. Never
/// overwrites without `force`.
pub fn write_spec(
    assignment: &ExtractAssignment,
    target_rel: &str,
    content: &str,
) -> anyhow::Result<WriteSpecOutput> {
    // Gate: refuse anything that doesn't parse.
    let validation = validate_spec(content);
    if !validation.ok {
        return Ok(WriteSpecOutput::Rejected {
            target_path: target_rel.to_string(),
            errors: validation.errors,
        });
    }

    let specs_root = PathBuf::from(&assignment.specs_root);
    let resolved = specs_root.join(target_rel);

    // Sandbox: the target must lexically sit inside specs_root. The file
    // may not yet exist, so canonicalize would fail — walk components
    // instead.
    let canonical_root = specs_root
        .canonicalize()
        .unwrap_or_else(|_| specs_root.clone());
    let normalized = lexical_normalize(&resolved);
    if !normalized.starts_with(&canonical_root) && !normalized.starts_with(&specs_root) {
        anyhow::bail!(
            "target_path '{}' resolves outside specs_root '{}'",
            target_rel,
            specs_root.display()
        );
    }

    if assignment.dry_run {
        println!("# --- {} ---", resolved.display());
        println!("{}", content);
        return Ok(WriteSpecOutput::DryRun {
            target_path: target_rel.to_string(),
            resolved_path: resolved.to_string_lossy().into_owned(),
        });
    }

    if resolved.exists() && !assignment.force {
        return Ok(WriteSpecOutput::SkippedExists {
            target_path: target_rel.to_string(),
            reason: "target already exists; rerun with --force to overwrite".to_string(),
        });
    }

    if let Some(parent) = resolved.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            anyhow::anyhow!("failed to create directory {}: {}", parent.display(), e)
        })?;
    }

    std::fs::write(&resolved, content)
        .map_err(|e| anyhow::anyhow!("failed to write {}: {}", resolved.display(), e))?;

    Ok(WriteSpecOutput::Written {
        target_path: target_rel.to_string(),
        resolved_path: resolved.to_string_lossy().into_owned(),
        bytes: content.len() as u64,
    })
}

// ── Internals ───────────────────────────────────────────────────────────

fn parse_spec_content(content: &str) -> Result<(), Vec<ought_spec::ParseError>> {
    let parser = OughtMdParser;
    parser
        .parse_string(content, Path::new("<draft>.ought.md"))
        .map(|_| ())
}

/// Collapse `.` and `..` components without touching the filesystem, so
/// we can sandbox paths that don't exist yet.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_assignment(tmp: &Path, dry_run: bool, force: bool) -> ExtractAssignment {
        ExtractAssignment {
            id: "test".into(),
            project_root: tmp.to_string_lossy().into_owned(),
            config_path: tmp.join("ought.toml").to_string_lossy().into_owned(),
            specs_root: tmp.join("ought").to_string_lossy().into_owned(),
            dry_run,
            force,
            groups: vec![],
        }
    }

    const VALID_SPEC: &str =
        "# Demo\n\ncontext: example\n\n## Behavior\n\n- **MUST** work\n";
    // `MUST BY` without a duration is a documented parse error.
    const INVALID_SPEC: &str =
        "# Demo\n\ncontext: example\n\n## Behavior\n\n- **MUST BY** respond quickly\n";

    #[test]
    fn validate_accepts_well_formed() {
        let out = validate_spec(VALID_SPEC);
        assert!(out.ok);
        assert!(out.errors.is_empty());
    }

    #[test]
    fn validate_rejects_malformed() {
        let out = validate_spec(INVALID_SPEC);
        assert!(!out.ok);
        assert!(!out.errors.is_empty());
    }

    #[test]
    fn write_spec_writes_valid_content() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("ought")).unwrap();
        let asn = make_assignment(tmp.path(), false, false);
        let out = write_spec(&asn, "demo.ought.md", VALID_SPEC).unwrap();
        assert!(matches!(out, WriteSpecOutput::Written { .. }));
        let disk = std::fs::read_to_string(tmp.path().join("ought/demo.ought.md")).unwrap();
        assert_eq!(disk, VALID_SPEC);
    }

    #[test]
    fn write_spec_rejects_invalid_content() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("ought")).unwrap();
        let asn = make_assignment(tmp.path(), false, false);
        let out = write_spec(&asn, "demo.ought.md", INVALID_SPEC).unwrap();
        assert!(matches!(out, WriteSpecOutput::Rejected { .. }));
        assert!(!tmp.path().join("ought/demo.ought.md").exists());
    }

    #[test]
    fn write_spec_skips_when_exists_without_force() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("ought")).unwrap();
        std::fs::write(tmp.path().join("ought/demo.ought.md"), "pre-existing").unwrap();
        let asn = make_assignment(tmp.path(), false, false);
        let out = write_spec(&asn, "demo.ought.md", VALID_SPEC).unwrap();
        assert!(matches!(out, WriteSpecOutput::SkippedExists { .. }));
        let disk = std::fs::read_to_string(tmp.path().join("ought/demo.ought.md")).unwrap();
        assert_eq!(disk, "pre-existing");
    }

    #[test]
    fn write_spec_overwrites_with_force() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("ought")).unwrap();
        std::fs::write(tmp.path().join("ought/demo.ought.md"), "pre-existing").unwrap();
        let asn = make_assignment(tmp.path(), false, true);
        let out = write_spec(&asn, "demo.ought.md", VALID_SPEC).unwrap();
        assert!(matches!(out, WriteSpecOutput::Written { .. }));
        let disk = std::fs::read_to_string(tmp.path().join("ought/demo.ought.md")).unwrap();
        assert_eq!(disk, VALID_SPEC);
    }

    #[test]
    fn write_spec_rejects_path_escaping_specs_root() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("ought")).unwrap();
        let asn = make_assignment(tmp.path(), false, false);
        assert!(write_spec(&asn, "../escaped.ought.md", VALID_SPEC).is_err());
    }

    #[test]
    fn write_spec_dry_run_does_not_touch_disk() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("ought")).unwrap();
        let asn = make_assignment(tmp.path(), true, false);
        let out = write_spec(&asn, "demo.ought.md", VALID_SPEC).unwrap();
        assert!(matches!(out, WriteSpecOutput::DryRun { .. }));
        assert!(!tmp.path().join("ought/demo.ought.md").exists());
    }
}
