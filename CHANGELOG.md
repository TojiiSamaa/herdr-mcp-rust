# Changelog

## [1.0.0]
- First release: `hosts_list`, `sessions_list`, `exec` (real exit code), `pane_read`, `pane_send`, `session_close` (human approval), `agents_list`, `herdr_stats`.
- Closed by default (exec and close are startup gates), command policy, test mode with simulated approver, stats.
- Test mode proof A (static) and B (sentinel sockets + fake ssh), with a mutation build that must fail B.
