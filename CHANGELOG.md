# Changelog

## [1.1.0]
- Host file notes: `description`, `group`, `default_dir`, `aliases` per host. `hosts_list` returns them, every tool accepts an alias as `host`, a new session starts in `default_dir`. The host file becomes the inventory of the fleet.

## [1.0.3]
- Tunnels to remote hosts open in parallel, in the background at startup, and no longer block each other (the lock is released while waiting for the socket; polling every 10 ms instead of 100 ms). Cold `hosts_list`: 706 ms to 105 ms.

## [1.0.2]
- `exec` makes 2 Herdr calls instead of 5: the pane id is cached (looked up again once if stale) and the text returned by the wait is reused instead of a second read. Remote host: 528 ms to 215 ms.

## [1.0.1]
- Version reported by `serverInfo` now matches the release (it said 0.1.0).
- Real before/after benchmark (`bench/`), README and Discord preview, SECURITY / CONTRIBUTING / CODE_OF_CONDUCT / NOTICE.
- Tests use neutral project names.

## [1.0.0]
- First release: `hosts_list`, `sessions_list`, `exec` (real exit code), `pane_read`, `pane_send`, `session_close` (human approval), `agents_list`, `herdr_stats`.
- Closed by default (exec and close are startup gates), command policy, test mode with simulated approver, stats.
- Test mode proof A (static) and B (sentinel sockets + fake ssh), with a mutation build that must fail B.
