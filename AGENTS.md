# herdr-mcp — agent instructions

Using this MCP server: read [`skill/herdr-mcp/SKILL.md`](skill/herdr-mcp/SKILL.md). It lists every tool, what is possible and impossible, recipes and error remedies. Tool names are exactly those of `tools/list`.

Changing this repo:
- Rust, one dependency (`serde_json`). All sockets and processes live in `src/backend.rs` only; `tests/egress.rs` fails otherwise.
- Closed by default: exec and close are env gates read at startup. Never add a tool argument that weakens a gate, the test mode or the approver.
- A tool added, renamed or removed = update `skill/herdr-mcp/SKILL.md` and `CHANGELOG.md` in the same commit (`tests/skill.rs` checks that every tool is in the skill).
- A bug fix comes with a test that replays the bug.
- No real host, IP, user name or path in the repo, tests included.
