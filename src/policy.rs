//! Startup configuration and the command policy. Everything here is fixed at startup:
//! no tool call can change it (test mode, approver and write gates are env-only).
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Approver {
    Approve,
    Deny,
    Timeout,
    Absent,
}

pub struct Config {
    pub test_mode: bool,
    pub approver: Approver,
    pub allow_exec: bool,
    pub allow_close: bool,
    pub confirm_fallback: bool,
    pub deny_extra: Vec<String>,
    pub max_timeout: Duration,
    pub stats_file: Option<String>,
}

fn flag(name: &str) -> bool {
    matches!(std::env::var(name).as_deref(), Ok("1") | Ok("true"))
}

impl Config {
    pub fn from_env() -> Self {
        let test_mode = flag("MCP_TEST_MODE") || std::env::args().any(|a| a == "--test");
        let approver = match std::env::var("MCP_TEST_APPROVER").as_deref() {
            Ok("approve") => Approver::Approve,
            Ok("timeout") => Approver::Timeout,
            Ok("absent") => Approver::Absent,
            _ => Approver::Deny, // closed by default
        };
        Config {
            test_mode,
            approver,
            allow_exec: flag("HERDR_MCP_ALLOW_EXEC"),
            allow_close: flag("HERDR_MCP_ALLOW_CLOSE"),
            confirm_fallback: flag("HERDR_MCP_CONFIRM_FALLBACK"),
            deny_extra: std::env::var("HERDR_MCP_DENY")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
            max_timeout: Duration::from_secs(300),
            stats_file: std::env::var("HERDR_MCP_STATS_FILE").ok(),
        }
    }
}

/// A seat belt, not a sandbox: it stops the mistakes that already cost us outages.
pub fn check_exec(cmd: &str, extra: &[String]) -> Result<(), String> {
    let low = cmd.to_ascii_lowercase();
    let toks: Vec<&str> = low
        .split(|c: char| c.is_whitespace() || matches!(c, ';' | '|' | '&' | '(' | ')' | '`'))
        .filter(|t| !t.is_empty())
        .collect();
    let base = |t: &str| t.rsplit('/').next().unwrap_or(t).to_string();
    for t in &toks {
        if matches!(base(t).as_str(), "pkill" | "killall" | "taskkill") {
            return Err("kill by name is refused: kill an explicit numeric PID".into());
        }
    }
    if toks.contains(&"kill") && low.contains("pgrep") {
        return Err("kill $(pgrep ..) is refused: kill an explicit numeric PID".into());
    }
    if toks.iter().any(|t| base(t) == "journalctl")
        && !toks.iter().any(|t| {
            t.starts_with("-n")
                || t.starts_with("--lines")
                || t.starts_with("--since")
                || t.starts_with("--until")
        })
    {
        return Err("unbounded journalctl is refused: add -n <lines> or --since".into());
    }
    for d in extra {
        if low.contains(d.as_str()) {
            return Err(format!("command matches HERDR_MCP_DENY entry '{d}'"));
        }
    }
    Ok(())
}

pub fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_kill_by_name() {
        assert!(check_exec("pkill node", &[]).is_err());
        assert!(check_exec("sudo /usr/bin/killall -9 x", &[]).is_err());
        assert!(check_exec("kill $(pgrep node)", &[]).is_err());
        assert!(check_exec("kill 1234", &[]).is_ok());
    }
    #[test]
    fn journalctl_must_be_bounded() {
        assert!(check_exec("journalctl -u docker --no-pager", &[]).is_err());
        assert!(check_exec("journalctl -u docker -n 200", &[]).is_ok());
        assert!(check_exec("journalctl --since '-30 min'", &[]).is_ok());
    }
    #[test]
    fn names() {
        assert!(valid_name("demo-2"));
        assert!(!valid_name("a b"));
        assert!(!valid_name(""));
    }
}
