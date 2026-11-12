# herdr-mcp-rust

An [MCP](https://modelcontextprotocol.io) server (stdio) that lets an AI agent run commands in **persistent, named terminal sessions** on one or many machines, through [Herdr](https://github.com/herdrdev/herdr). One shell per `(host, project)`: `cd` and environment persist, a real **exit code** comes back, and no new SSH shell is opened per command.

Rust, one dependency (`serde_json`), ~1 binary.

## Tools
`hosts_list` · `sessions_list` · `exec` · `pane_read` · `pane_send` · `session_close` · `agents_list` · `herdr_stats` — full reference, limits and recipes in [`skill/herdr-mcp/SKILL.md`](skill/herdr-mcp/SKILL.md) (portable: Claude Code, OpenCode, Hermes, Codex… see [`AGENTS.md`](AGENTS.md)).

## Install
```bash
cargo build --release          # binary: target/release/herdr-mcp
# on each machine you want to drive, a Herdr server must run:  herdr server
claude mcp add herdr -- /path/to/herdr-mcp     # or any MCP client, stdio
```

## Configuration (environment, read once at startup)
| Variable | Default | Effect |
|---|---|---|
| `HERDR_MCP_ALLOW_EXEC` | off | enables `exec` and `pane_send` |
| `HERDR_MCP_ALLOW_CLOSE` | off | enables `session_close` (still asks the human) |
| `HERDR_MCP_CONFIRM_FALLBACK` | off | if the client cannot be asked (no elicitation), accept `session_close` anyway. Opt-in, not recommended |
| `HERDR_MCP_HOSTS` | one `local` host | path to a JSON host file (below) |
| `HERDR_MCP_DENY` | none | comma list of substrings refused in commands |
| `HERDR_MCP_STATS_FILE` | none | write the stats snapshot after each call |
| `MCP_TEST_MODE=1` / `--test` | off | simulate everything, no network, no process (see skill) |
| `MCP_TEST_APPROVER` | `deny` | `approve\|deny\|timeout\|absent`, test mode only |

Host file:
```json
{"hosts": {
  "local": {"socket": "/home/me/.config/herdr/herdr.sock"},
  "build": {"ssh": "me@build.example.org", "remote_socket": "/home/me/.config/herdr/herdr.sock"}
}}
```
An `ssh` host gets ONE long-lived `ssh -N -L` tunnel to the remote Herdr socket (key-based, `BatchMode`).

## Security model
- Closed by default: nothing executes until `HERDR_MCP_ALLOW_EXEC=1`.
- The policy refuses kill-by-name and unbounded `journalctl`; it is a seat belt, **not a sandbox**. Whoever can call `exec` can run any other command as the Herdr user.
- `session_close` asks the human (MCP elicitation) and fails closed.
- Test mode, approver and gates cannot be changed by a tool call; in test mode the real backend is never built (CI proves it, including a mutation test).

## Not tested
Real Herdr servers over the ssh tunnel on Windows clients; Herdr versions other than 0.9.5; very large outputs (>200 kB are truncated by design).

Apache-2.0.
