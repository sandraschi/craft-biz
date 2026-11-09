//! "Every task is doable from the window": each app keeps a table that maps
//! every MCP tool to the screen element that performs it, and a test calls
//! [`check`]. A new tool without a UI entry fails the build.

use serde_json::Value;

/// `covered`: (tool name, where in the window). Fails on tools without an
/// entry and on entries naming tools that no longer exist.
pub fn check(tools: &[Value], covered: &[(&str, &str)]) -> Result<(), String> {
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    let missing: Vec<&str> = names
        .iter()
        .copied()
        .filter(|n| !covered.iter().any(|(c, w)| c == n && !w.trim().is_empty()))
        .collect();
    let stale: Vec<&str> = covered
        .iter()
        .map(|(c, _)| *c)
        .filter(|c| !names.contains(c))
        .collect();
    if missing.is_empty() && stale.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "tools without a window entry: {missing:?}; entries for unknown tools: {stale:?}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reports_missing_and_stale() {
        let tools = vec![json!({"name": "a_x"}), json!({"name": "a_y"})];
        assert!(check(&tools, &[("a_x", "Board"), ("a_y", "Detail panel")]).is_ok());
        let e = check(&tools, &[("a_x", "Board"), ("a_z", "gone")]).unwrap_err();
        assert!(e.contains("a_y") && e.contains("a_z"));
        assert!(
            check(&tools, &[("a_x", "Board"), ("a_y", " ")]).is_err(),
            "empty location does not count"
        );
    }
}
