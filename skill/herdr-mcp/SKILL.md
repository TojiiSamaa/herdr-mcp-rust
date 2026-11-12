---
name: herdr-mcp
description: "Run shell commands in persistent, named terminal sessions on one or many machines through Herdr, and read their output, exit code and state. Use when a task needs to execute or follow commands on a remote host without opening a new shell each time, follow a long command, or check which coding agents are working or blocked."
---

# herdr-mcp

MCP server (stdio) over [Herdr](https://github.com/herdrdev/herdr). One persistent shell per `(host, project)`: `cd` and environment persist between calls, and nothing opens a new SSH shell per command. Tool names below are exactly those of `tools/list`.

## Tools

| Tool | Role | Writes | Admin validation |
|---|---|---|---|
| `hosts_list` | Hosts configured and whether their Herdr server answers | no | no |
| `sessions_list` | Workspaces on one host or all; `managed:true` = created by `exec` (`claude-<project>`) | no | no |
| `exec` | Run a command in `claude-<project>`; returns `exit_code`, `output`, `timed_out`, `truncated`, `pane_id` | yes | no (needs `HERDR_MCP_ALLOW_EXEC=1`) |
| `pane_read` | Last N lines of a pane (follow a command that timed out) | no | no |
| `pane_send` | Type text and/or keys into a pane without waiting (`C-c` to interrupt) | yes | no (needs `HERDR_MCP_ALLOW_EXEC=1`) |
| `session_close` | Close `claude-<project>` and its shells | yes, destructive | **yes** (needs `HERDR_MCP_ALLOW_CLOSE=1`) |
| `agents_list` | Coding agents Herdr recognises and their state | no | no |
| `herdr_stats` | Calls, errors, p50/p95, real vs test calls, blocked egress, approvals | no | no |

### Parameters and examples
- `exec` `{host, project, command, timeout_s=25 (1-300), max_output_bytes=20000}` → `{"host":"a","project":"web","command":"git status --short"}`
- `pane_read` `{host, pane_id, lines=200 (1-3000)}`
- `pane_send` `{host, pane_id, text?, keys?[]}` → interrupt: `{"host":"a","pane_id":"w1:p1","keys":["C-c"]}`
- `session_close` `{host, project}`; `sessions_list` `{host?}`; `agents_list` `{host}`

## Possible
- Run any shell command and get its **real exit code** (a marker pair is printed around the command; the typed echo never matches it).
- Keep state between calls (cwd, exported variables, virtualenv).
- Many hosts: local Unix sockets, or remote hosts through ONE long-lived `ssh -N -L` tunnel per host (no shell per command).
- Several projects in parallel; commands of the same `(host, project)` are queued, never interleaved.
- Follow a long command: it keeps running after `timed_out:true`; read it with `pane_read`.

## Impossible / out of scope
- Not a sandbox. The policy only refuses what has already caused outages: kill by name (`pkill`, `killall`, `taskkill`, `kill $(pgrep …)`), `journalctl` without `-n`/`--since`/`--until`, plus your own `HERDR_MCP_DENY` entries.
- `exec` runs in the pane's current shell: `exit` inside a command ends that shell. Interactive programs (editors, pagers) need `pane_send`.
- No file transfer, no tunnels, no database tools (use scp/rsync or another MCP).
- Cannot start Herdr servers, install Herdr, or change the host list at runtime (startup config only).
- `exec` and `pane_send` are disabled until `HERDR_MCP_ALLOW_EXEC=1`; `session_close` until `HERDR_MCP_ALLOW_CLOSE=1`.
- Test mode, approver and gates cannot be changed by a tool call.

## How to
1. `hosts_list` → pick a `host` with `reachable:true`.
2. Run: `exec {host, project:"deploy", command:"cd /srv/app && git pull"}`; check `exit_code == 0`.
3. Next call in the same project is in `/srv/app` already.
4. Long command: `exec {…, timeout_s:120}`. If `timed_out:true` → `pane_read {host, pane_id}` until done, or `pane_send {keys:["C-c"]}`.
5. Big output: raise `max_output_bytes` (max 200000) or pipe through `tail`/`grep` in the command. `truncated:true` keeps the END of the output.
6. Cleanup: `sessions_list` → `session_close` (the human is asked).

### Errors and remedies
| `error` | Meaning | Do |
|---|---|---|
| `exec_disabled` / `close_disabled` | gate closed at startup | ask the operator to set the env var |
| `command_refused` | policy | rewrite: kill a numeric PID, bound `journalctl` |
| `unknown_host` | not in the host file | `hosts_list` |
| `invalid_argument` | bad/missing field, project not `[a-z0-9_-]{1,40}` | fix input |
| `herdr_error` | Herdr unreachable / refused (text included) | `hosts_list` shows `reachable:false`; start the Herdr server on that host |
| `denied`, `approval_timeout`, `confirmation_required` | the human did not approve, did not answer, or the client cannot be asked | nothing was closed; ask the human |
Absent session on `session_close` is `found:false`, not an error.

## Mode test
Start with `MCP_TEST_MODE=1` (or `--test`); impossible to enable or disable from a tool call.
- **Simulated:** every tool answers from an in-memory mock; replies carry `"test_mode": true` and output contains `[TEST]`. No Herdr, socket or ssh is used, none needs to exist.
- **Egress cut:** the real backend is never built. Configured real targets are only counted (`egress_blocked` in `herdr_stats`).
- **Proof (CI):** A = static check that no socket/process API exists outside `src/backend.rs`; B = all tools called against sentinel sockets and a fake `ssh`: 0 connection, 0 spawn; a mutation build (`--features mutant`) must make B fail.
- **Approver:** `MCP_TEST_APPROVER=approve|deny|timeout|absent` (default `deny`) drives `session_close`. A simulated approval never authorises anything real.
- **Validate all commands:** `tools/list`, call each tool with the examples above, check the schema and `test_mode:true`; for `session_close` run the four approver values and check that only `approve` closes.

## Stats
`herdr_stats` (read-only); `HERDR_MCP_STATS_FILE=/path.json` also writes the snapshot after each call. Per tool: `calls`, `errors`, `real_calls`, `test_mode_calls`, `p50_ms`, `p95_ms`, `last_call_unix`; global `egress_blocked` and `approvals{approved,refused,timeouts}`.
