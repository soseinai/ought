//! Output format parsers. Each submodule turns a raw stream (JUnit XML,
//! TAP, ought-json) into `Vec<TestResult>`.

pub mod cargo_test;
pub mod json;
pub mod junit;
pub mod tap;

use std::collections::HashMap;

use ought_spec::ClauseId;

/// Convert a `ClauseId` like `auth::login::must_return_jwt` into the test
/// function name used in generated code: `test_auth__login__must_return_jwt`.
/// Double-underscore preserves section boundaries so the mapping is reversible.
pub fn clause_id_to_test_name(clause_id: &ClauseId) -> String {
    format!("test_{}", clause_id.0.replace("::", "__"))
}

/// Recover a `ClauseId` from a test function name. Strips a leading `test_`
/// or `Test` prefix (Go convention) and maps `__` back to `::`.
pub fn test_name_to_clause_id(test_name: &str) -> ClauseId {
    let stripped = test_name
        .strip_prefix("test_")
        .or_else(|| test_name.strip_prefix("Test"))
        .unwrap_or(test_name);
    ClauseId(stripped.replace("__", "::"))
}

/// Look up a test name in the provided name→ClauseId map, falling back to
/// the `__`↔`::` convention. Tries a few common shapes that test harnesses
/// emit (fully-qualified `file::name`, trailing segment, etc.).
pub fn resolve_clause_id(name: &str, map: &HashMap<String, ClauseId>) -> ClauseId {
    if let Some(id) = map.get(name) {
        return id.clone();
    }
    // Try the last `::`-segment (cargo test prints `module::test_name`).
    if let Some(last) = name.rsplit("::").next()
        && let Some(id) = map.get(last)
    {
        return id.clone();
    }
    // Try the last `.`-segment (go subtest paths, e.g. `Pkg/TestName`).
    if let Some(last) = name.rsplit('/').next()
        && let Some(id) = map.get(last)
    {
        return id.clone();
    }
    test_name_to_clause_id(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_clause_id() {
        let id = ClauseId("auth::login::must_return_jwt".to_string());
        let round = test_name_to_clause_id(&clause_id_to_test_name(&id));
        assert_eq!(round, id);
    }

    #[test]
    fn resolve_from_map() {
        let mut map = HashMap::new();
        map.insert(
            "test_auth__login".to_string(),
            ClauseId("auth::login".to_string()),
        );
        assert_eq!(
            resolve_clause_id("test_auth__login", &map),
            ClauseId("auth::login".to_string())
        );
    }

    #[test]
    fn resolve_strips_module_path() {
        let mut map = HashMap::new();
        map.insert(
            "test_x__y".to_string(),
            ClauseId("x::y".to_string()),
        );
        assert_eq!(
            resolve_clause_id("mod_a::submod::test_x__y", &map),
            ClauseId("x::y".to_string())
        );
    }

    #[test]
    fn resolve_falls_back_to_convention() {
        let map = HashMap::new();
        assert_eq!(
            resolve_clause_id("test_auth__login", &map),
            ClauseId("auth::login".to_string())
        );
    }

    #[test]
    fn strips_go_test_prefix() {
        assert_eq!(
            test_name_to_clause_id("Testauth__login"),
            ClauseId("auth::login".to_string())
        );
    }
}
