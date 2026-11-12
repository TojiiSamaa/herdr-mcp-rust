# Security Policy

## Reporting a vulnerability

**Do not open a public issue, discussion or pull request for a vulnerability.**

Report it privately through GitHub Security Advisories of this repository: open the **Security** tab, then **Report a vulnerability** ([direct link](../../security/advisories/new)). Include:

- the affected component (see Scope) and version or commit;
- the configuration needed to reproduce (redact every host, user, path and address: use `*.example.org` hosts, `/home/me/...` paths and documentation IP ranges);
- step-by-step reproduction, proof of concept if possible;
- the impact and how it could be exploited.

You will get an acknowledgement and a first assessment on a best-effort basis. Fixes are developed in the private advisory and published with it.

## Supported versions

| Version | Supported |
|---------|-----------|
| latest release (1.0.x) | yes |
| `main` branch | yes, fixes land here first |
| anything older | no |

## Scope

In scope:

- **Gates**: any way to run `exec` or `pane_send` without `HERDR_MCP_ALLOW_EXEC`, or `session_close` without `HERDR_MCP_ALLOW_CLOSE`, or to change a gate, the test mode or the approver from a tool call.
- **Human approval**: `session_close` closing anything after a refusal, a timeout, or with a client that cannot be asked (without `HERDR_MCP_CONFIRM_FALLBACK`).
- **Command policy**: a bypass of the refusals (kill by name, unbounded `journalctl`, `HERDR_MCP_DENY` entries) that the policy claims to catch. The policy is a seat belt, not a sandbox: running another command once `exec` is allowed is expected, not a vulnerability.
- **ssh tunnel**: argument injection through the host file or tool arguments into the `ssh` command line, a tunnel socket readable by other users.
- **Test mode** (`--test` / `MCP_TEST_MODE=1`): any path that reaches a socket or starts a process while active.
- **Input handling**: `project` names outside `[a-z0-9_-]{1,40}` reaching Herdr, output limits not enforced.

Out of scope: vulnerabilities in Herdr itself (report them to Herdr), in ssh, or in the MCP client; whatever a user allowed to call `exec` runs on purpose; missing hardening of a machine you control.

## Good practice for users

Keep `HERDR_MCP_ALLOW_EXEC` off where read access is enough, never set `HERDR_MCP_CONFIRM_FALLBACK` on a shared setup, use key-based ssh with a dedicated user, and add the commands you never want an agent to type to `HERDR_MCP_DENY`.
