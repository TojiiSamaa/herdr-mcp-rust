mod backend;
mod policy;
mod stats;
mod tools;

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct Ctx {
    pub cfg: policy::Config,
    pub backend: Box<dyn backend::Backend>,
    pub stats: Arc<stats::Stats>,
    pub elicit_ok: AtomicBool,
    out: Mutex<std::io::Stdout>,
    pending: Mutex<HashMap<String, mpsc::Sender<Value>>>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    pub panes: Mutex<HashMap<String, String>>,
}

static SEQ: AtomicU64 = AtomicU64::new(0);
pub fn new_id() -> String {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{:x}{:x}",
        n & 0xffff_ffff_ffff,
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

impl Ctx {
    pub fn send(&self, v: &Value) {
        let mut o = self.out.lock().unwrap();
        let _ = writeln!(o, "{v}");
        let _ = o.flush();
    }
    /// One mutex per (host, project): commands of one session never interleave.
    pub fn lock_for(&self, key: &str) -> Arc<Mutex<()>> {
        self.locks
            .lock()
            .unwrap()
            .entry(key.to_string())
            .or_default()
            .clone()
    }
    /// Ask the human through MCP elicitation. Err = refused / timeout.
    pub fn elicit(&self, message: &str) -> Result<(), &'static str> {
        let id = new_id();
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id.clone(), tx);
        self.send(
            &json!({"jsonrpc":"2.0","id":id,"method":"elicitation/create",
            "params":{"message":message,"requestedSchema":{"type":"object","properties":{}}}}),
        );
        let r = rx.recv_timeout(Duration::from_secs(60));
        self.pending.lock().unwrap().remove(&id);
        match r {
            Ok(v) if v.pointer("/result/action").and_then(|a| a.as_str()) == Some("accept") => {
                Ok(())
            }
            Ok(_) => Err("denied"),
            Err(_) => Err("approval_timeout"),
        }
    }
}

fn reply(ctx: &Ctx, id: &Value, result: Value) {
    ctx.send(&json!({"jsonrpc":"2.0","id":id,"result":result}));
}

fn main() {
    let cfg = policy::Config::from_env();
    let stats = Arc::new(stats::Stats::default());
    let backend = backend::select(&cfg, stats.clone());
    let ctx = Arc::new(Ctx {
        cfg,
        backend,
        stats,
        elicit_ok: AtomicBool::new(false),
        out: Mutex::new(std::io::stdout()),
        pending: Mutex::new(HashMap::new()),
        locks: Mutex::new(HashMap::new()),
        panes: Mutex::new(HashMap::new()),
    });

    // warm the tunnels while the client is still handshaking (real backend only)
    if !ctx.cfg.test_mode {
        let c = ctx.clone();
        std::thread::spawn(move || {
            let _ = tools::call(&c, "hosts_list", &json!({}));
        });
    }

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = msg.get("id").cloned();
        match msg.get("method").and_then(|m| m.as_str()) {
            None => {
                // a response to one of our requests (elicitation)
                if let Some(k) = id.as_ref().and_then(|i| i.as_str()) {
                    if let Some(tx) = ctx.pending.lock().unwrap().get(k) {
                        let _ = tx.send(msg.clone());
                    }
                }
            }
            Some("initialize") => {
                let has = msg.pointer("/params/capabilities/elicitation").is_some();
                ctx.elicit_ok.store(has, Ordering::Relaxed);
                let ver = msg
                    .pointer("/params/protocolVersion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("2025-06-18");
                reply(
                    &ctx,
                    &id.unwrap_or(Value::Null),
                    json!({
                        "protocolVersion": ver,
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name":"herdr-mcp","version":env!("CARGO_PKG_VERSION")}
                    }),
                );
            }
            Some("ping") => reply(&ctx, &id.unwrap_or(Value::Null), json!({})),
            Some("tools/list") => reply(
                &ctx,
                &id.unwrap_or(Value::Null),
                json!({"tools": tools::list()}),
            ),
            Some("tools/call") => {
                let ctx = ctx.clone();
                let id = id.unwrap_or(Value::Null);
                std::thread::spawn(move || {
                    let name = msg
                        .pointer("/params/name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let args = msg
                        .pointer("/params/arguments")
                        .cloned()
                        .unwrap_or(json!({}));
                    let t0 = Instant::now();
                    let (mut payload, is_err) = tools::call(&ctx, &name, &args);
                    ctx.stats.call(
                        &name,
                        t0.elapsed().as_millis() as u64,
                        is_err,
                        ctx.cfg.test_mode,
                    );
                    if ctx.cfg.test_mode {
                        payload["test_mode"] = json!(true);
                    }
                    if let Some(f) = &ctx.cfg.stats_file {
                        let _ = std::fs::write(f, ctx.stats.snapshot().to_string());
                    }
                    reply(
                        &ctx,
                        &id,
                        json!({"content":[{"type":"text","text":payload.to_string()}],"isError":is_err}),
                    );
                });
            }
            Some(m) if m.starts_with("notifications/") => {}
            Some(_) => {
                if let Some(id) = id {
                    ctx.send(&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not found"}}));
                }
            }
        }
    }
}
