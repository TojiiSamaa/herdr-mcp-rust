mod common;
use common::Mcp;
use serde_json::json;

const TEST: &[(&str, &str)] = &[
    ("MCP_TEST_MODE", "1"),
    ("HERDR_MCP_ALLOW_EXEC", "1"),
    ("HERDR_MCP_ALLOW_CLOSE", "1"),
];

fn with(extra: &[(&'static str, &'static str)]) -> Mcp {
    let mut e = TEST.to_vec();
    e.extend_from_slice(extra);
    Mcp::start(&e)
}

#[test]
fn exec_returns_exit_code_and_output_marked_test() {
    let mut m = with(&[]);
    let (r, err) = m.tool(
        "exec",
        json!({"host":"local","project":"demo","command":"echo hi"}),
    );
    assert!(!err, "{r}");
    assert_eq!(r["exit_code"], 0);
    assert_eq!(r["timed_out"], false);
    assert_eq!(r["test_mode"], true);
    assert!(r["output"].as_str().unwrap().contains("[TEST]"));
}

#[test]
fn session_is_reused_for_the_same_project() {
    let mut m = with(&[]);
    let a = m
        .tool(
            "exec",
            json!({"host":"local","project":"demo","command":"true"}),
        )
        .0;
    let b = m
        .tool(
            "exec",
            json!({"host":"local","project":"demo","command":"true"}),
        )
        .0;
    assert_eq!(a["pane_id"], b["pane_id"]);
    let s = m.tool("sessions_list", json!({})).0;
    assert_eq!(s["sessions"].as_array().unwrap().len(), 1);
}

#[test]
fn exec_is_closed_by_default() {
    let mut m = Mcp::start(&[("MCP_TEST_MODE", "1")]);
    let (r, err) = m.tool("exec", json!({"host":"local","project":"x","command":"ls"}));
    assert!(err);
    assert_eq!(r["error"], "exec_disabled");
}

#[test]
fn policy_refuses_before_any_herdr_call() {
    let mut m = with(&[]);
    for c in ["pkill node", "journalctl -u docker --no-pager"] {
        let (r, err) = m.tool("exec", json!({"host":"local","project":"x","command":c}));
        assert!(err);
        assert_eq!(r["error"], "command_refused", "{c}");
    }
    // nothing reached the backend: no session was created
    assert_eq!(
        m.tool("sessions_list", json!({})).0["sessions"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn invalid_input_is_typed() {
    let mut m = with(&[]);
    assert_eq!(
        m.tool("exec", json!({"host":"nope","project":"x","command":"ls"}))
            .0["error"],
        "unknown_host"
    );
    assert_eq!(
        m.tool(
            "exec",
            json!({"host":"local","project":"Bad Name","command":"ls"})
        )
        .0["error"],
        "invalid_argument"
    );
    assert_eq!(
        m.tool("exec", json!({"host":"local","project":"x"})).0["error"],
        "invalid_argument"
    );
    assert_eq!(m.tool("nothing", json!({})).0["error"], "unknown_tool");
}

fn close_with(approver: &'static str) -> (serde_json::Value, bool, Mcp) {
    let mut m = with(&[("MCP_TEST_APPROVER", approver)]);
    m.tool(
        "exec",
        json!({"host":"local","project":"zz","command":"true"}),
    );
    let (r, e) = m.tool("session_close", json!({"host":"local","project":"zz"}));
    (r, e, m)
}

#[test]
fn approver_approve_closes() {
    let (r, e, mut m) = close_with("approve");
    assert!(!e, "{r}");
    assert_eq!(r["closed"], true);
    assert_eq!(
        m.tool("sessions_list", json!({})).0["sessions"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn approver_deny_timeout_absent_close_nothing() {
    for (ap, code) in [
        ("deny", "denied"),
        ("timeout", "approval_timeout"),
        ("absent", "confirmation_required"),
    ] {
        let (r, e, mut m) = close_with(ap);
        assert!(e, "{ap}");
        assert_eq!(r["error"], code);
        assert_eq!(
            m.tool("sessions_list", json!({})).0["sessions"]
                .as_array()
                .unwrap()
                .len(),
            1,
            "{ap} must not close"
        );
    }
}

#[test]
fn default_approver_is_deny() {
    let mut m = with(&[]);
    m.tool(
        "exec",
        json!({"host":"local","project":"zz","command":"true"}),
    );
    assert_eq!(
        m.tool("session_close", json!({"host":"local","project":"zz"}))
            .0["error"],
        "denied"
    );
}

#[test]
fn closing_a_missing_session_is_an_answer_not_an_error() {
    let mut m = with(&[("MCP_TEST_APPROVER", "approve")]);
    let (r, e) = m.tool("session_close", json!({"host":"local","project":"ghost"}));
    assert!(!e);
    assert_eq!(r["found"], false);
}

#[test]
fn stats_count_calls_errors_approvals() {
    let mut m = with(&[("MCP_TEST_APPROVER", "deny")]);
    m.tool(
        "exec",
        json!({"host":"local","project":"zz","command":"true"}),
    );
    m.tool(
        "exec",
        json!({"host":"local","project":"zz","command":"pkill x"}),
    );
    m.tool("session_close", json!({"host":"local","project":"zz"}));
    let s = m.tool("herdr_stats", json!({})).0;
    assert_eq!(s["tools"]["exec"]["calls"], 2);
    assert_eq!(s["tools"]["exec"]["errors"], 1);
    assert_eq!(s["tools"]["exec"]["test_mode_calls"], 2);
    assert_eq!(s["tools"]["exec"]["real_calls"], 0);
    assert_eq!(s["approvals"]["refused"], 1);
    assert!(s["egress_blocked"].as_u64().unwrap() > 0);
}

#[test]
fn every_listed_tool_has_a_strict_schema() {
    let mut m = with(&[]);
    let r = m.rpc("tools/list", json!({}));
    for t in r["result"]["tools"].as_array().unwrap() {
        assert_eq!(
            t["inputSchema"]["additionalProperties"], false,
            "{}",
            t["name"]
        );
        assert!(!t["description"].as_str().unwrap().is_empty());
    }
}
