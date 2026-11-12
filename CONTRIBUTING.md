# Contributing

Thanks for helping. Keep changes small, tested and documented.

## Build

```sh
cargo build
```

Rust stable, Linux or macOS (Unix sockets).

## Checks before a pull request

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
! cargo test --features mutant --test egress b_zero_connections_zero_spawn_in_test_mode
```

- **Tests need no Herdr, no ssh and no network.** The suite runs the server in test mode (`MCP_TEST_MODE=1`), where every tool answers from a mock and the real backend is never built.
- The last command must **fail**: the `mutant` feature adds a real call in test mode, and the egress proof has to catch it. CI runs it.
- Benchmarks: update `bench/results.json`, then `node bench/table.mjs && node bench/chart.mjs`, and attach the numbers to performance changes. Never commit a value you did not measure; leave it `null`.
- A bug fix comes with a test that fails before the fix and passes after it.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`, `test:`, `chore:`, `refactor:`, `perf:`. One subject line, imperative, lower case after the colon.

## Rules

- **No real data in the repository**: no real host, IP, user name, e-mail or local path, tests and fixtures included. Use `*.example.org` hosts, `/home/me/...` paths and the documentation IP ranges (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24).
- **One dependency** (`serde_json`). Adding one needs a reason in the pull request.
- **One egress point**: sockets and processes live in `src/backend.rs` only; `tests/egress.rs` fails otherwise.
- **Closed by default**: gates, test mode and approver are read from the environment at startup. Never add a tool argument that weakens them.
- **When a tool changes** (name, parameters, behavior), update in the same pull request [`skill/herdr-mcp/SKILL.md`](skill/herdr-mcp/SKILL.md), [`CHANGELOG.md`](CHANGELOG.md) and the [README](README.md); `tests/skill.rs` checks that every tool is in the skill.

## Conduct and security

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).
