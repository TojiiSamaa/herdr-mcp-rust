//! Test mode proof, part A (structural) and B (dynamic).
//! With `--features mutant` a deliberate real call is added: B must then FAIL (CI checks that).
mod common;
use common::Mcp;
use serde_json::json;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A: no network/process API outside src/backend.rs (the single egress point).
#[test]
fn a_no_egress_api_outside_the_backend_module() {
    let forbidden = [
        "UnixStream",
        "TcpStream",
        "UdpSocket",
        "std::process",
        "Command::new",
        "UnixDatagram",
    ];
    for f in std::fs::read_dir("src").unwrap() {
        let p = f.unwrap().path();
        if p.file_name().unwrap() == "backend.rs" {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        // ignore the test modules and comments of this file's own forbidden-word lists
        for w in forbidden {
            assert!(
                !src.contains(w),
                "{} uses {w} outside backend.rs",
                p.display()
            );
        }
    }
}

fn sentinel(path: &std::path::Path, hits: Arc<AtomicU64>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(path);
    let l = UnixListener::bind(path).unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            if s.is_ok() {
                hits.fetch_add(1, Ordering::SeqCst);
            }
        }
    });
}

/// B: every tool, called in test mode against real-looking targets, opens 0 connection and spawns no ssh.
#[test]
fn b_zero_connections_zero_spawn_in_test_mode() {
    let dir = std::env::temp_dir().join(format!("hm-egress-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let hits = Arc::new(AtomicU64::new(0));
    sentinel(&dir.join("xdg/herdr/herdr.sock"), hits.clone()); // default socket (used by the mutant)
    sentinel(&dir.join("configured.sock"), hits.clone());
    // a fake ssh on PATH that leaves a trace if it is ever executed
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let trace = dir.join("ssh-was-run");
    let ssh = bin.join("ssh");
    std::fs::write(&ssh, format!("#!/bin/sh\ntouch {}\n", trace.display())).unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&ssh)
        .status()
        .unwrap();
    let hosts = dir.join("hosts.json");
    std::fs::write(
        &hosts,
        json!({"hosts":{
        "sock":{"socket":dir.join("configured.sock")},
        "remote":{"ssh":"user@203.0.113.7","remote_socket":"/srv/herdr/herdr.sock"}}})
        .to_string(),
    )
    .unwrap();

    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let (xdg, hostsf) = (
        dir.join("xdg").display().to_string(),
        hosts.display().to_string(),
    );
    let mut m = Mcp::start(&[
        ("MCP_TEST_MODE", "1"),
        ("HERDR_MCP_ALLOW_EXEC", "1"),
        ("HERDR_MCP_ALLOW_CLOSE", "1"),
        ("MCP_TEST_APPROVER", "approve"),
        ("PATH", &path),
        ("XDG_CONFIG_HOME", &xdg),
        ("HERDR_MCP_HOSTS", &hostsf),
    ]);

    let mut seen = vec![];
    for h in ["sock", "remote"] {
        for (tool, args) in [
            ("hosts_list", json!({})),
            ("sessions_list", json!({"host":h})),
            ("exec", json!({"host":h,"project":"p","command":"ls"})),
            ("pane_read", json!({"host":h,"pane_id":"w1:p1"})),
            ("pane_send", json!({"host":h,"pane_id":"w1:p1","text":"x"})),
            ("agents_list", json!({"host":h})),
            ("session_close", json!({"host":h,"project":"p"})),
            ("herdr_stats", json!({})),
        ] {
            let (r, err) = m.tool(tool, args);
            assert!(!err, "{tool} on {h}: {r}"); // every tool answers
            seen.push(tool.to_string());
        }
    }
    let mut listed = m.tool_names();
    listed.sort();
    seen.sort();
    seen.dedup();
    assert_eq!(
        listed, seen,
        "this test must exercise every tool of tools/list"
    );

    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "a socket connection reached a real target in test mode"
    );
    assert!(!trace.exists(), "ssh was spawned in test mode");
    assert!(
        m.tool("herdr_stats", json!({})).0["egress_blocked"]
            .as_u64()
            .unwrap()
            > 0
    );
}
