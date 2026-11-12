#![allow(dead_code)]
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct Mcp {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    n: u64,
}

impl Mcp {
    pub fn start(env: &[(&str, &str)]) -> Mcp {
        let mut c = Command::new(env!("CARGO_BIN_EXE_herdr-mcp"));
        c.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (k, v) in env {
            c.env(k, v);
        }
        let mut child = c.spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let out = BufReader::new(child.stdout.take().unwrap());
        let mut m = Mcp {
            child,
            stdin,
            out,
            n: 0,
        };
        m.rpc("initialize", json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}));
        m
    }
    pub fn rpc(&mut self, method: &str, params: Value) -> Value {
        self.n += 1;
        let req = json!({"jsonrpc":"2.0","id":self.n,"method":method,"params":params});
        writeln!(self.stdin, "{req}").unwrap();
        loop {
            let mut l = String::new();
            self.out.read_line(&mut l).unwrap();
            let v: Value = serde_json::from_str(&l).unwrap();
            if v.get("id") == Some(&json!(self.n)) {
                return v;
            }
        }
    }
    /// Returns (payload, isError)
    pub fn tool(&mut self, name: &str, args: Value) -> (Value, bool) {
        let r = self.rpc("tools/call", json!({"name":name,"arguments":args}));
        let res = &r["result"];
        let text = res["content"][0]["text"].as_str().unwrap();
        (
            serde_json::from_str(text).unwrap(),
            res["isError"].as_bool().unwrap(),
        )
    }
    pub fn tool_names(&mut self) -> Vec<String> {
        self.rpc("tools/list", json!({}))["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
