//! The skill must list exactly the tools of tools/list (no missing tool, no ghost tool).
mod common;
use common::Mcp;

#[test]
fn skill_lists_every_tool_and_no_ghost() {
    let skill = std::fs::read_to_string("skill/herdr-mcp/SKILL.md").unwrap();
    let mut m = Mcp::start(&[("MCP_TEST_MODE", "1")]);
    let tools = m.tool_names();
    for t in &tools {
        assert!(
            skill.contains(&format!("| `{t}` |")),
            "tool {t} missing from the skill table"
        );
    }
    let table = skill
        .split("## Tools")
        .nth(1)
        .unwrap()
        .split("### Parameters")
        .next()
        .unwrap();
    let rows: Vec<&str> = table
        .lines()
        .filter_map(|l| l.strip_prefix("| `"))
        .filter_map(|l| l.split('`').next())
        .collect();
    for r in rows {
        assert!(tools.iter().any(|t| t == r), "ghost tool {r} in the skill");
    }
    for h in [
        "## Possible",
        "## Impossible",
        "## How to",
        "## Mode test",
        "## Stats",
    ] {
        assert!(skill.contains(h), "section {h} missing");
    }
}
