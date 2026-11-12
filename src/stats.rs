use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
struct Tool {
    calls: u64,
    errors: u64,
    real: u64,
    test: u64,
    lat_ms: Vec<u64>,
    last: u64,
}

#[derive(Default)]
struct Inner {
    tools: BTreeMap<String, Tool>,
    egress_blocked: u64,
    approvals: u64,
    refusals: u64,
    timeouts: u64,
}

#[derive(Default)]
pub struct Stats(Mutex<Inner>);

fn pct(v: &[u64], p: f64) -> u64 {
    if v.is_empty() {
        return 0;
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    s[(((s.len() - 1) as f64) * p).round() as usize]
}

impl Stats {
    pub fn call(&self, tool: &str, ms: u64, err: bool, test: bool) {
        let mut g = self.0.lock().unwrap();
        let t = g.tools.entry(tool.to_string()).or_default();
        t.calls += 1;
        t.errors += err as u64;
        if test {
            t.test += 1
        } else {
            t.real += 1
        }
        t.lat_ms.push(ms);
        if t.lat_ms.len() > 1000 {
            t.lat_ms.remove(0);
        }
        t.last = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }
    pub fn egress_blocked(&self) {
        self.0.lock().unwrap().egress_blocked += 1;
    }
    pub fn approval(&self, outcome: &str) {
        let mut g = self.0.lock().unwrap();
        match outcome {
            "approved" => g.approvals += 1,
            "timeout" => g.timeouts += 1,
            _ => g.refusals += 1,
        }
    }
    pub fn snapshot(&self) -> Value {
        let g = self.0.lock().unwrap();
        let tools: BTreeMap<_, _> = g
            .tools
            .iter()
            .map(|(k, t)| {
                (
                    k.clone(),
                    json!({"calls":t.calls,"errors":t.errors,"real_calls":t.real,"test_mode_calls":t.test,
                           "p50_ms":pct(&t.lat_ms,0.5),"p95_ms":pct(&t.lat_ms,0.95),"last_call_unix":t.last}),
                )
            })
            .collect();
        json!({"tools":tools,"egress_blocked":g.egress_blocked,
               "approvals":{"approved":g.approvals,"refused":g.refusals,"timeouts":g.timeouts}})
    }
}
