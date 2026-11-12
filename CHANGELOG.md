# Changelog

## [1.0.1]
- Version reported by `serverInfo` now matches the release (it said 0.1.0).
- Real before/after benchmark (`bench/`), README and Discord preview, SECURITY / CONTRIBUTING / CODE_OF_CONDUCT / NOTICE.
- Tests use neutral project names.

## [1.0.0]
- First release: `hosts_list`, `sessions_list`, `exec` (real exit code), `pane_read`, `pane_send`, `session_close` (human approval), `agents_list`, `herdr_stats`.
- Closed by default (exec and close are startup gates), command policy, test mode with simulated approver, stats.
- Test mode proof A (static) and B (sentinel sockets + fake ssh), with a mutation build that must fail B.
