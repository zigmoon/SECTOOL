//! Lecture des connexions actives de la machine.
//!
//! Chaque OS a son propre format ; on le normalise vers `Connection`.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use sectool_core::connection::{ConnState, Connection, Scope};
use sectool_core::{ip_scope, now_ms};

use crate::sys;

/// Résultat d'un relevé : sockets + table PID → nom de processus.
pub struct SocketSnapshot {
    pub connections: Vec<Connection>,
    pub processes: HashMap<u32, String>,
}

impl SocketSnapshot {
    pub fn listening(&self) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.state == ConnState::Listen)
            .collect()
    }
}

/// Lit les sockets TCP et UDP de la machine.
pub async fn snapshot() -> SocketSnapshot {
    let (connections, processes) = if cfg!(target_os = "windows") {
        windows_sockets().await
    } else if cfg!(target_os = "macos") {
        macos_sockets().await
    } else {
        linux_sockets().await
    };
    SocketSnapshot {
        connections,
        processes,
    }
}

fn scope_of(remote: &str) -> Scope {
    match remote.parse::<IpAddr>() {
        Ok(ip) => ip_scope(&ip),
        Err(_) => Scope::Local,
    }
}

fn build(
    proto: &str,
    local: &str,
    remote: &str,
    state: ConnState,
    pid: Option<u32>,
    process: Option<String>,
    ts: i64,
) -> Option<Connection> {
    let (laddr, lport) = split_endpoint(local)?;
    let (raddr, rport) = split_endpoint(remote).unwrap_or(("*".into(), 0));
    let scope = if state == ConnState::Listen || raddr == "*" || raddr == "0.0.0.0" {
        Scope::Local
    } else {
        scope_of(&raddr)
    };
    Some(Connection {
        id: format!("{proto}|{laddr}:{lport}|{raddr}:{rport}"),
        proto: proto.to_string(),
        local_addr: laddr,
        local_port: lport,
        remote_addr: raddr,
        remote_port: rport,
        state,
        pid,
        process,
        scope,
        first_seen: ts,
        last_seen: ts,
    })
}

/// Découpe `1.2.3.4:80`, `[::]:80` ou `*:*`.
fn split_endpoint(text: &str) -> Option<(String, u16)> {
    let text = text.trim();
    if text.is_empty() || text == "*" || text == "*:*" {
        return Some(("*".to_string(), 0));
    }
    if let Some(rest) = text.strip_prefix('[') {
        // [ipv6]:port
        let close = rest.find(']')?;
        let addr = rest[..close].to_string();
        let port = rest[close + 1..]
            .trim_start_matches(':')
            .parse::<u16>()
            .unwrap_or(0);
        return Some((addr, port));
    }
    match text.rsplit_once(':') {
        Some((addr, port)) => {
            let p = port.parse::<u16>().unwrap_or(0);
            Some((addr.to_string(), p))
        }
        None => Some((text.to_string(), 0)),
    }
}

// ───────────────────────────── Windows ─────────────────────────────────────

async fn windows_sockets() -> (Vec<Connection>, HashMap<u32, String>) {
    let ts = now_ms();
    let processes = windows_processes().await;
    let mut out = Vec::new();
    if let Some(text) = sys::run("netstat", &["-ano", "-p", "tcp"], Duration::from_secs(8)).await {
        for line in text.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 5 || !cols[0].eq_ignore_ascii_case("TCP") {
                continue;
            }
            let pid = cols[4].parse::<u32>().ok();
            let state = ConnState::from_raw(cols[3]);
            if let Some(conn) = build(
                "tcp",
                cols[1],
                cols[2],
                state,
                pid,
                pid.and_then(|p| processes.get(&p).cloned()),
                ts,
            ) {
                out.push(conn);
            }
        }
    }
    if let Some(text) = sys::run("netstat", &["-ano", "-p", "udp"], Duration::from_secs(8)).await {
        for line in text.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 4 || !cols[0].eq_ignore_ascii_case("UDP") {
                continue;
            }
            let pid = cols[3].parse::<u32>().ok();
            if let Some(conn) = build(
                "udp",
                cols[1],
                cols[2],
                ConnState::Udp,
                pid,
                pid.and_then(|p| processes.get(&p).cloned()),
                ts,
            ) {
                out.push(conn);
            }
        }
    }
    out.dedup_by(|a, b| a.id == b.id);
    (out, processes)
}

async fn windows_processes() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let text = match sys::run("tasklist", &["/FO", "CSV", "/NH"], Duration::from_secs(8)).await {
        Some(t) => t,
        None => return map,
    };
    for line in text.lines() {
        let mut parts = line.split("\",\"");
        let name = parts
            .next()
            .map(|s| s.trim_matches('"').to_string())
            .unwrap_or_default();
        let pid = parts
            .next()
            .map(|s| s.trim_matches('"').to_string())
            .and_then(|s| s.parse::<u32>().ok());
        if let (Some(pid), false) = (pid, name.is_empty()) {
            map.insert(pid, name);
        }
    }
    map
}

// ───────────────────────────── Linux ───────────────────────────────────────

async fn linux_sockets() -> (Vec<Connection>, HashMap<u32, String>) {
    let ts = now_ms();
    let mut out = Vec::new();
    let mut processes: HashMap<u32, String> = HashMap::new();
    if let Some(text) = sys::run("ss", &["-tunap"], Duration::from_secs(6)).await {
        for line in text.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 6 {
                continue;
            }
            let proto = if cols[0].starts_with("tcp") { "tcp" } else { "udp" };
            let is_udp = proto == "udp";
            let state = if is_udp {
                ConnState::Udp
            } else {
                ConnState::from_raw(cols[1])
            };
            let (local, remote) = if is_udp {
                (cols[4], cols[5])
            } else {
                (cols[4], cols[5])
            };
            let pid = cols
                .iter()
                .find_map(|c| c.split("pid=").nth(1).map(|p| p.trim_end_matches(')')));
            let pid = pid.and_then(|p| p.split(',').next()).and_then(|p| p.parse::<u32>().ok());
            let process = pid
                .and_then(|p| std::fs::read_to_string(format!("/proc/{p}/comm")).ok())
                .map(|s| s.trim().to_string());
            if let Some(p) = pid {
                if let Some(name) = &process {
                    processes.insert(p, name.clone());
                }
            }
            if let Some(conn) = build(proto, local, remote, state, pid, process, ts) {
                out.push(conn);
            }
        }
    }
    out.dedup_by(|a, b| a.id == b.id);
    (out, processes)
}

// ───────────────────────────── macOS ───────────────────────────────────────

async fn macos_sockets() -> (Vec<Connection>, HashMap<u32, String>) {
    let ts = now_ms();
    let mut out = Vec::new();
    let mut processes: HashMap<u32, String> = HashMap::new();
    let text = match sys::run("lsof", &["-nP", "-i"], Duration::from_secs(10)).await {
        Some(t) => t,
        None => return (out, processes),
    };
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 9 {
            continue;
        }
        let process = cols[0].to_string();
        let pid = cols[1].parse::<u32>().ok();
        if let Some(p) = pid {
            processes.entry(p).or_insert_with(|| process.clone());
        }
        let proto = if cols[7].eq_ignore_ascii_case("tcp") {
            "tcp"
        } else {
            "udp"
        };
        let (local, remote) = match cols[8].split_once("->") {
            Some((l, r)) => (l, r),
            None => (cols[8], "*:*"),
        };
        let state = if remote == "*:*" {
            if proto == "tcp" {
                ConnState::Listen
            } else {
                ConnState::Udp
            }
        } else {
            ConnState::Established
        };
        if let Some(conn) = build(proto, local, remote, state, pid, Some(process), ts) {
            out.push(conn);
        }
    }
    out.dedup_by(|a, b| a.id == b.id);
    (out, processes)
}
