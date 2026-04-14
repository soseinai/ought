//! Native ought RunResult JSON parser.
//!
//! This is the escape hatch for runners ought doesn't know how to adapt to:
//! the external tool emits a JSON document with shape `RunResult` (see
//! [`crate::types::RunResult`]) and ought passes it through verbatim.

use crate::types::RunResult;

pub fn parse(json: &str) -> anyhow::Result<RunResult> {
    let parsed: RunResult = serde_json::from_str(json)
        .map_err(|e| anyhow::anyhow!("failed to parse ought-json output: {e}"))?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TestDetails, TestResult, TestStatus};
    use ought_spec::ClauseId;
    use std::time::Duration;

    #[test]
    fn round_trip() {
        let original = RunResult {
            results: vec![TestResult {
                clause_id: ClauseId("auth::login".to_string()),
                status: TestStatus::Passed,
                message: None,
                duration: Duration::from_millis(500),
                details: TestDetails::default(),
            }],
            total_duration: Duration::from_millis(500),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed = parse(&json).unwrap();
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.results[0].status, TestStatus::Passed);
    }

    #[test]
    fn rejects_garbage() {
        let err = parse("not json").unwrap_err();
        assert!(err.to_string().contains("ought-json"));
    }
}
