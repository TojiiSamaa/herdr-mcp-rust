use crate::policy::{self, Approver};
use crate::{new_id, Ctx};
use serde_json::{json, Value};
use std::time::Duration;

type R = Result<Value, (&'static str, String)>;

fn schema(props: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":props,"required":required,"additionalProperties":false})
}

pub fn list() -> Value {
    let host = json!({"type":"string","description":"Host name from hosts_list."});
    let project = json!({"type":"string","description":"Session name, [a-z0-9_-]{1,40}. The workspace is labelled claude-<project>."});
    json!([
      {"name":"hosts_list","description":"List configured hosts and whether their Herdr server answers. Read-only.",
       "inputSchema":schema(json!({}),&[])},
      {"name":"sessions_list","description":"List Herdr workspaces on one host (or all). Sessions created by exec are labelled claude-<project>. Read-only.",
       "inputSchema":schema(json!({"host":host}),&[])},
      {"name":"exec","description":"Run a shell command in the persistent session claude-<project> on a host and return exit_code and output. The shell state (cwd, env) persists between calls. Long commands: raise timeout_s; on timeout the command keeps running, use pane_read with the returned pane_id. Needs HERDR_MCP_ALLOW_EXEC=1. Kill-by-name and unbounded journalctl are refused.",
       "inputSchema":schema(json!({"host":host,"project":project,
         "command":{"type":"string","description":"Shell command, may span several lines."},
         "timeout_s":{"type":"integer","minimum":1,"maximum":300,"default":25},
         "max_output_bytes":{"type":"integer","minimum":1000,"maximum":200000,"default":20000}}),&["host","project","command"])},
      {"name":"pane_read","description":"Read the recent output of a pane (also to follow a command that timed out). Read-only.",
       "inputSchema":schema(json!({"host":host,"pane_id":{"type":"string"},"lines":{"type":"integer","minimum":1,"maximum":3000,"default":200}}),&["host","pane_id"])},
      {"name":"pane_send","description":"Type text and/or keys (e.g. Enter, C-c) into a pane, without waiting. Needs HERDR_MCP_ALLOW_EXEC=1. Use C-c to interrupt a command that timed out.",
       "inputSchema":schema(json!({"host":host,"pane_id":{"type":"string"},"text":{"type":"string"},
         "keys":{"type":"array","items":{"type":"string"}}}),&["host","pane_id"])},
      {"name":"session_close","description":"Close the workspace claude-<project> and its shells. Destructive: asks the human (MCP elicitation). Needs HERDR_MCP_ALLOW_CLOSE=1. Missing session returns found:false.",
       "inputSchema":schema(json!({"host":host,"project":project}),&["host","project"])},
      {"name":"agents_list","description":"List coding agents Herdr recognises on a host with their state (idle/working/blocked/done/unknown). Read-only.",
       "inputSchema":schema(json!({"host":host}),&["host"])},
      {"name":"herdr_stats","description":"Usage statistics of this MCP server: calls, errors, p50/p95 latency per tool, real vs test-mode calls, blocked egress, approvals. Read-only.",
       "inputSchema":schema(json!({}),&[])}
    ])
}

fn s<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(|v| v.as_str())
}
fn need<'a>(a: &'a Value, k: &str) -> Result<&'a str, (&'static str, String)> {
    s(a, k).ok_or(("invalid_argument", format!("missing string argument '{k}'")))
}
fn host_ok(ctx: &Ctx, a: &Value) -> Result<String, (&'static str, String)> {
    let h = need(a, "host")?;
    if ctx.backend.hosts().iter().any(|x| x == h) {
        Ok(h.to_string())
    } else {
        Err((
            "unknown_host",
            format!("'{h}' is not configured; see hosts_list"),
        ))
    }
}
fn project_ok(a: &Value) -> Result<String, (&'static str, String)> {
    let p = need(a, "project")?;
    if policy::valid_name(p) {
        Ok(p.to_string())
    } else {
        Err((
            "invalid_argument",
            "project must match [a-z0-9_-]{1,40}".into(),
        ))
    }
}
fn herdr(
    ctx: &Ctx,
    host: &str,
    m: &str,
    p: Value,
    t: Duration,
) -> Result<Value, (&'static str, String)> {
    ctx.backend
        .call(host, m, p, t)
        .map_err(|e| ("herdr_error", e))
}

pub fn call(ctx: &Ctx, name: &str, a: &Value) -> (Value, bool) {
    let r = match name {
        "hosts_list" => hosts_list(ctx),
        "sessions_list" => sessions_list(ctx, a),
        "exec" => exec(ctx, a),
        "pane_read" => pane_read(ctx, a),
        "pane_send" => pane_send(ctx, a),
        "session_close" => session_close(ctx, a),
        "agents_list" => agents_list(ctx, a),
        "herdr_stats" => Ok(ctx.stats.snapshot()),
        _ => Err(("unknown_tool", format!("no tool named '{name}'"))),
    };
    match r {
        Ok(v) => (v, false),
        Err((code, msg)) => (json!({"error":code,"message":msg}), true),
    }
}

fn hosts_list(ctx: &Ctx) -> R {
    let rows: Vec<Value> = ctx
        .backend
        .hosts()
        .into_iter()
        .map(|h| match ctx.backend.call(&h, "workspace.list", json!({}), Duration::from_secs(8)) {
            Ok(v) => json!({"host":h,"reachable":true,"workspaces":v.get("workspaces").and_then(|w| w.as_array()).map(|w| w.len()).unwrap_or(0)}),
            Err(e) => json!({"host":h,"reachable":false,"error":e}),
        })
        .collect();
    Ok(json!({"hosts":rows}))
}

fn workspaces(ctx: &Ctx, host: &str) -> Result<Vec<Value>, (&'static str, String)> {
    let v = herdr(
        ctx,
        host,
        "workspace.list",
        json!({}),
        Duration::from_secs(10),
    )?;
    Ok(v.get("workspaces")
        .and_then(|w| w.as_array())
        .cloned()
        .unwrap_or_default())
}

fn sessions_list(ctx: &Ctx, a: &Value) -> R {
    let hosts = if s(a, "host").is_some() {
        vec![host_ok(ctx, a)?]
    } else {
        ctx.backend.hosts()
    };
    let mut out = vec![];
    for h in hosts {
        match workspaces(ctx, &h) {
            Ok(ws) => {
                for w in ws {
                    let label = w.get("label").and_then(|l| l.as_str()).unwrap_or("");
                    out.push(json!({"host":h,"workspace_id":w.get("workspace_id"),"label":label,
                        "managed":label.starts_with("claude-"),"pane_count":w.get("pane_count"),"agent_status":w.get("agent_status")}));
                }
            }
            Err((_, e)) => out.push(json!({"host":h,"error":e})),
        }
    }
    Ok(json!({"sessions":out}))
}

fn find_workspace(
    ctx: &Ctx,
    host: &str,
    project: &str,
) -> Result<Option<String>, (&'static str, String)> {
    let label = format!("claude-{project}");
    Ok(workspaces(ctx, host)?
        .into_iter()
        .find(|w| w.get("label").and_then(|l| l.as_str()) == Some(label.as_str()))
        .and_then(|w| {
            w.get("workspace_id")
                .and_then(|i| i.as_str())
                .map(String::from)
        }))
}

fn ensure_pane(ctx: &Ctx, host: &str, project: &str) -> Result<String, (&'static str, String)> {
    let p = lookup_pane(ctx, host, project)?;
    ctx.panes
        .lock()
        .unwrap()
        .insert(format!("{host}/{project}"), p.clone());
    Ok(p)
}

fn lookup_pane(ctx: &Ctx, host: &str, project: &str) -> Result<String, (&'static str, String)> {
    let t = Duration::from_secs(15);
    if let Some(ws) = find_workspace(ctx, host, project)? {
        let v = herdr(ctx, host, "pane.list", json!({"workspace_id":ws}), t)?;
        if let Some(p) = v.pointer("/panes/0/pane_id").and_then(|p| p.as_str()) {
            return Ok(p.to_string());
        }
    }
    let v = herdr(
        ctx,
        host,
        "workspace.create",
        json!({"label":format!("claude-{project}"),"focus":false}),
        t,
    )?;
    v.pointer("/root_pane/pane_id")
        .and_then(|p| p.as_str())
        .map(String::from)
        .ok_or((
            "herdr_error",
            "workspace.create returned no root pane".into(),
        ))
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                o.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                o.push('=');
            }
        }
    }
    o
}

/// Extract (output, exit_code) between our start and end markers; None if the end marker is absent.
fn parse_run(text: &str, id: &str) -> (Option<String>, Option<i32>) {
    let start = format!("__HM_{id}_S__");
    let end = format!("__HM_{id}_E_");
    let lines: Vec<&str> = text.lines().collect();
    let Some(si) = lines.iter().rposition(|l| l.trim() == start) else {
        return (None, None);
    };
    for (i, l) in lines.iter().enumerate().skip(si + 1) {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix(&end).and_then(|r| r.strip_suffix("__")) {
            if let Ok(code) = rest.parse::<i32>() {
                return (Some(lines[si + 1..i].join("\n")), Some(code));
            }
        }
    }
    (Some(lines[si + 1..].join("\n")), None)
}

fn exec(ctx: &Ctx, a: &Value) -> R {
    if !ctx.cfg.allow_exec {
        return Err((
            "exec_disabled",
            "start the server with HERDR_MCP_ALLOW_EXEC=1 to allow command execution".into(),
        ));
    }
    let host = host_ok(ctx, a)?;
    let project = project_ok(a)?;
    let cmd = need(a, "command")?;
    policy::check_exec(cmd, &ctx.cfg.deny_extra).map_err(|e| ("command_refused", e))?;
    let timeout = Duration::from_secs(
        a.get("timeout_s")
            .and_then(|v| v.as_u64())
            .unwrap_or(25)
            .clamp(1, ctx.cfg.max_timeout.as_secs()),
    );
    let max = a
        .get("max_output_bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(20000)
        .clamp(1000, 200000) as usize;

    let lock = ctx.lock_for(&format!("{host}/{project}"));
    let _g = lock.lock().unwrap();
    let key = format!("{host}/{project}");
    let cached = ctx.panes.lock().unwrap().get(&key).cloned();
    let mut pane = match cached {
        Some(p) => p,
        None => ensure_pane(ctx, &host, &project)?,
    };
    let id = new_id();
    // The typed line shows `%s`/`$?`, never a digit: only the real output matches the end marker.
    let text = format!(
        "printf '__HM_{id}_S__\\n'; eval \"$(printf %s {} | base64 -d)\"; printf '__HM_{id}_E_%s__\\n' $?",
        b64(cmd.as_bytes())
    );
    let send = |pane: &str| {
        herdr(
            ctx,
            &host,
            "pane.send_input",
            json!({"pane_id":pane,"text":text,"keys":["Enter"]}),
            Duration::from_secs(10),
        )
    };
    if send(&pane).is_err() {
        // stale cached pane (session closed): look it up again, once
        ctx.panes.lock().unwrap().remove(&key);
        pane = ensure_pane(ctx, &host, &project)?;
        send(&pane)?;
    }
    let wait = ctx.backend.call(
        &host,
        "pane.wait_for_output",
        json!({"pane_id":pane,"source":"recent_unwrapped","lines":3000,
               "match":{"type":"regex","value":format!("(?m)^__HM_{id}_E_[0-9]+__\\s*$")},
               "timeout_ms":timeout.as_millis() as u64,"strip_ansi":true}),
        timeout + Duration::from_secs(10),
    );
    let waited_out = match &wait {
        Ok(_) => false,
        Err(e)
            if e.to_ascii_lowercase().contains("timeout")
                || e.to_ascii_lowercase().contains("timed out") =>
        {
            true
        }
        Err(e) => return Err(("herdr_error", e.clone())),
    };
    // a matched wait already carries the text; read again only after a timeout
    let read = match wait.ok().filter(|w| w.pointer("/read/text").is_some()) {
        Some(w) => w,
        None => herdr(
            ctx,
            &host,
            "pane.read",
            json!({"pane_id":pane,"source":"recent_unwrapped","lines":3000,"format":"text","strip_ansi":true}),
            Duration::from_secs(10),
        )?,
    };
    let txt = read
        .pointer("/read/text")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let (out, code) = parse_run(txt, &id);
    let mut out = out.unwrap_or_default();
    let truncated = out.len() > max;
    if truncated {
        let mut cut = out.len() - max;
        while !out.is_char_boundary(cut) {
            cut += 1;
        }
        out = out[cut..].to_string();
    }
    Ok(
        json!({"host":host,"project":project,"pane_id":pane,"exit_code":code,
        "timed_out":waited_out || code.is_none(),"truncated":truncated,"output":out}),
    )
}

fn pane_read(ctx: &Ctx, a: &Value) -> R {
    let host = host_ok(ctx, a)?;
    let pane = need(a, "pane_id")?;
    let lines = a
        .get("lines")
        .and_then(|v| v.as_u64())
        .unwrap_or(200)
        .clamp(1, 3000);
    let v = herdr(
        ctx,
        &host,
        "pane.read",
        json!({"pane_id":pane,"source":"recent_unwrapped","lines":lines,"format":"text","strip_ansi":true}),
        Duration::from_secs(10),
    )?;
    Ok(
        json!({"host":host,"pane_id":pane,"text":v.pointer("/read/text").cloned().unwrap_or(json!(""))}),
    )
}

fn pane_send(ctx: &Ctx, a: &Value) -> R {
    if !ctx.cfg.allow_exec {
        return Err((
            "exec_disabled",
            "start the server with HERDR_MCP_ALLOW_EXEC=1".into(),
        ));
    }
    let host = host_ok(ctx, a)?;
    let pane = need(a, "pane_id")?;
    let text = s(a, "text").unwrap_or("");
    policy::check_exec(text, &ctx.cfg.deny_extra).map_err(|e| ("command_refused", e))?;
    let keys: Vec<Value> = a
        .get("keys")
        .and_then(|k| k.as_array())
        .cloned()
        .unwrap_or_default();
    if text.is_empty() && keys.is_empty() {
        return Err(("invalid_argument", "give text and/or keys".into()));
    }
    // pane.send_input types text then presses keys; with no keys we must not add Enter by default.
    herdr(
        ctx,
        &host,
        "pane.send_input",
        json!({"pane_id":pane,"text":text,"keys":keys}),
        Duration::from_secs(10),
    )?;
    Ok(json!({"host":host,"pane_id":pane,"sent":true}))
}

fn session_close(ctx: &Ctx, a: &Value) -> R {
    if !ctx.cfg.allow_close {
        return Err((
            "close_disabled",
            "start the server with HERDR_MCP_ALLOW_CLOSE=1".into(),
        ));
    }
    let host = host_ok(ctx, a)?;
    let project = project_ok(a)?;
    let Some(ws) = find_workspace(ctx, &host, &project)? else {
        return Ok(json!({"host":host,"project":project,"found":false}));
    };
    let verdict: Result<(), &'static str> = if ctx.cfg.test_mode {
        match ctx.cfg.approver {
            Approver::Approve => Ok(()),
            Approver::Deny => Err("denied"),
            Approver::Timeout => Err("approval_timeout"),
            Approver::Absent => Err("confirmation_required"),
        }
    } else if ctx.elicit_ok.load(std::sync::atomic::Ordering::Relaxed) {
        ctx.elicit(&format!(
            "Close session claude-{project} on {host}? Its shells and running commands end."
        ))
    } else if ctx.cfg.confirm_fallback {
        Ok(())
    } else {
        Err("confirmation_required")
    };
    match verdict {
        Ok(()) => ctx.stats.approval("approved"),
        Err("approval_timeout") => ctx.stats.approval("timeout"),
        Err(_) => ctx.stats.approval("refused"),
    }
    if let Err(code) = verdict {
        return Err((code, "the human did not approve; nothing was closed".into()));
    }
    herdr(
        ctx,
        &host,
        "workspace.close",
        json!({"workspace_id":ws}),
        Duration::from_secs(10),
    )?;
    ctx.panes
        .lock()
        .unwrap()
        .remove(&format!("{host}/{project}"));
    Ok(json!({"host":host,"project":project,"found":true,"closed":true}))
}

fn agents_list(ctx: &Ctx, a: &Value) -> R {
    let host = host_ok(ctx, a)?;
    let v = herdr(ctx, &host, "agent.list", json!({}), Duration::from_secs(10))?;
    Ok(json!({"host":host,"agents":v.get("agents").cloned().unwrap_or(json!([]))}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foo"), "Zm9v");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
    }
    #[test]
    fn typed_echo_never_matches_end_marker() {
        // the echoed command line contains the marker with %s, not digits
        let typed = "printf '__HM_ab_S__\\n'; eval ...; printf '__HM_ab_E_%s__\\n' $?";
        assert_eq!(parse_run(typed, "ab"), (None, None));
    }
    #[test]
    fn parses_output_and_exit_code() {
        let t = "$ typed\n__HM_ab_S__\nhello\nworld\n__HM_ab_E_3__\n$ ";
        assert_eq!(parse_run(t, "ab"), (Some("hello\nworld".into()), Some(3)));
    }
    #[test]
    fn running_command_has_no_exit_code() {
        let t = "__HM_ab_S__\npartial";
        assert_eq!(parse_run(t, "ab"), (Some("partial".into()), None));
    }
}
