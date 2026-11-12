//! The ONLY module allowed to touch sockets or spawn processes (checked by tests/egress.rs).
//! In test mode `Real` is never constructed: `select` returns `Mock`.
use crate::policy::Config;
use crate::stats::Stats;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub trait Backend: Send + Sync {
    fn hosts(&self) -> Vec<String>;
    fn call(
        &self,
        host: &str,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String>;
}

#[derive(Clone, Debug)]
pub enum Target {
    Socket(String),
    Ssh {
        target: String,
        remote_socket: String,
    },
}

pub fn default_socket() -> String {
    let base = std::env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", std::env::var("HOME").unwrap_or_default()));
    format!("{base}/herdr/herdr.sock")
}

/// HERDR_MCP_HOSTS = path to {"hosts":{"name":{"socket":".."} | {"ssh":"user@host","remote_socket":".."}}}
pub fn load_hosts() -> BTreeMap<String, Target> {
    let mut m = BTreeMap::new();
    let parsed = std::env::var("HERDR_MCP_HOSTS")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok());
    if let Some(h) = parsed
        .as_ref()
        .and_then(|v| v.get("hosts"))
        .and_then(|h| h.as_object())
    {
        for (name, c) in h {
            let t = if let Some(s) = c.get("ssh").and_then(|x| x.as_str()) {
                Target::Ssh {
                    target: s.to_string(),
                    remote_socket: c
                        .get("remote_socket")
                        .and_then(|x| x.as_str())
                        .unwrap_or("~/.config/herdr/herdr.sock")
                        .to_string(),
                }
            } else {
                Target::Socket(
                    c.get("socket")
                        .and_then(|x| x.as_str())
                        .map(String::from)
                        .unwrap_or_else(default_socket),
                )
            };
            m.insert(name.clone(), t);
        }
    }
    if m.is_empty() {
        m.insert("local".into(), Target::Socket(default_socket()));
    }
    m
}

pub fn select(cfg: &Config, stats: Arc<Stats>) -> Box<dyn Backend> {
    let hosts = load_hosts();
    if cfg.test_mode {
        Box::new(Mock {
            hosts,
            stats,
            st: Mutex::new(MockState::default()),
        })
    } else {
        Box::new(real::Real::new(hosts))
    }
}

// ---------------------------------------------------------------- real (single egress point)
mod real {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::process::{Child, Command, Stdio};

    pub struct Real {
        hosts: BTreeMap<String, Target>,
        tunnels: Mutex<BTreeMap<String, Child>>,
    }

    impl Real {
        pub fn new(hosts: BTreeMap<String, Target>) -> Self {
            Real {
                hosts,
                tunnels: Mutex::new(BTreeMap::new()),
            }
        }

        fn socket_for(&self, host: &str) -> Result<String, String> {
            match self.hosts.get(host).ok_or("unknown_host")? {
                Target::Socket(p) => Ok(p.clone()),
                Target::Ssh {
                    target,
                    remote_socket,
                } => {
                    let local = format!("{}/herdr-mcp-{host}.sock", std::env::temp_dir().display());
                    let mut t = self.tunnels.lock().unwrap();
                    let alive = t
                        .get_mut(host)
                        .map(|c| matches!(c.try_wait(), Ok(None)))
                        .unwrap_or(false);
                    if !alive {
                        if let Some(mut old) = t.remove(host) {
                            let _ = old.kill();
                            let _ = old.wait();
                        }
                        let _ = std::fs::remove_file(&local);
                        // One long-lived ssh process per host: no shell per command.
                        let child = Command::new("ssh")
                            .args([
                                "-N",
                                "-o",
                                "BatchMode=yes",
                                "-o",
                                "ExitOnForwardFailure=yes",
                                "-o",
                                "ControlPath=none",
                                "-o",
                                "ServerAliveInterval=30",
                                "-o",
                                "StreamLocalBindUnlink=yes",
                                "-L",
                            ])
                            .arg(format!("{local}:{remote_socket}"))
                            .arg(target)
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
                            .map_err(|e| format!("ssh_spawn_failed: {e}"))?;
                        t.insert(host.to_string(), child);
                    }
                    // release the lock before waiting: other hosts must not queue behind this one
                    drop(t);
                    for _ in 0..500 {
                        if std::path::Path::new(&local).exists() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Ok(local)
                }
            }
        }
    }

    impl Backend for Real {
        fn hosts(&self) -> Vec<String> {
            self.hosts.keys().cloned().collect()
        }
        fn call(
            &self,
            host: &str,
            method: &str,
            params: Value,
            timeout: Duration,
        ) -> Result<Value, String> {
            let path = self.socket_for(host)?;
            let mut s = UnixStream::connect(&path)
                .map_err(|e| format!("herdr_unreachable: {path}: {e}"))?;
            let _ = s.set_read_timeout(Some(timeout));
            let _ = s.set_write_timeout(Some(Duration::from_secs(10)));
            let req = json!({"id":"mcp","method":method,"params":params});
            writeln!(s, "{req}").map_err(|e| format!("write_failed: {e}"))?;
            let mut line = String::new();
            BufReader::new(s)
                .read_line(&mut line)
                .map_err(|e| format!("read_failed: {e}"))?;
            let v: Value = serde_json::from_str(&line).map_err(|e| format!("bad_response: {e}"))?;
            if let Some(err) = v.get("error") {
                let msg = err
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("herdr error");
                let code = err.get("code").and_then(|c| c.as_str()).unwrap_or("error");
                return Err(format!("{code}: {msg}"));
            }
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        }
    }
}

// ---------------------------------------------------------------- mock (test mode)
#[derive(Default)]
struct MockState {
    n: u32,
    ws: Vec<(String, String)>,
    text: BTreeMap<String, String>,
}

pub struct Mock {
    hosts: BTreeMap<String, Target>,
    stats: Arc<Stats>,
    st: Mutex<MockState>,
}

impl Backend for Mock {
    fn hosts(&self) -> Vec<String> {
        self.hosts.keys().cloned().collect()
    }
    fn call(&self, host: &str, method: &str, p: Value, _t: Duration) -> Result<Value, String> {
        #[cfg(feature = "mutant")]
        {
            // deliberate leak used by the mutation test: the egress proof MUST fail with this on
            let _ = std::os::unix::net::UnixStream::connect(default_socket());
        }
        if !self.hosts.contains_key(host) {
            return Err("unknown_host".into());
        }
        // Real-looking targets exist in the config: count them as blocked, never touch them.
        self.stats.egress_blocked();
        let mut s = self.st.lock().unwrap();
        let pid = |p: &Value| {
            p.get("pane_id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string()
        };
        Ok(match method {
            "workspace.list" => json!({"type":"workspace_list","workspaces":s.ws.iter()
                .map(|(i,l)| json!({"workspace_id":i,"label":l,"pane_count":1,"agent_status":"unknown"})).collect::<Vec<_>>()}),
            "workspace.create" => {
                s.n += 1;
                let id = format!("w{}", s.n);
                let label = p
                    .get("label")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                s.ws.push((id.clone(), label.clone()));
                json!({"type":"workspace_created","root_pane":{"pane_id":format!("{id}:p1")},
                       "workspace":{"workspace_id":id,"label":label}})
            }
            "pane.list" => {
                let w = p
                    .get("workspace_id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                json!({"type":"pane_list","panes":[{"pane_id":format!("{w}:p1"),"workspace_id":w}]})
            }
            "pane.send_input" | "pane.send_text" | "pane.send_keys" => {
                let text = p.get("text").and_then(|x| x.as_str()).unwrap_or("");
                if let Some(i) = text.find("__HM_") {
                    let id: String = text[i + 5..].chars().take_while(|c| *c != '_').collect();
                    let out = format!("__HM_{id}_S__\n[TEST] simulated output\n__HM_{id}_E_0__\n");
                    s.text.insert(pid(&p), out);
                }
                json!({"type":"ok"})
            }
            "pane.wait_for_output" => {
                let t = s.text.get(&pid(&p)).cloned().unwrap_or_default();
                json!({"type":"pane_output_matched","read":{"pane_id":pid(&p),"text":t}})
            }
            "pane.read" => {
                let t = s.text.get(&pid(&p)).cloned().unwrap_or_default();
                json!({"type":"pane_read","read":{"pane_id":pid(&p),"text":t}})
            }
            "workspace.close" => {
                let w = p.get("workspace_id").and_then(|x| x.as_str()).unwrap_or("");
                s.ws.retain(|(i, _)| i != w);
                json!({"type":"ok"})
            }
            "agent.list" => json!({"type":"agent_list","agents":[]}),
            other => return Err(format!("mock_unsupported: {other}")),
        })
    }
}
