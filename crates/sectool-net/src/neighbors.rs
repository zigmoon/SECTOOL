//! Table des voisins (ARP / ND) : qui est présent sur le réseau local.

use std::net::Ipv4Addr;
use std::time::Duration;

use sectool_core::normalize_mac;

use crate::sys;

/// Une ligne de la table ARP.
#[derive(Debug, Clone)]
pub struct Neighbor {
    pub ip: Ipv4Addr,
    pub mac: Option<String>,
    pub iface: Option<String>,
    pub state: String,
}

/// Lit la table ARP du système.
pub async fn read_neighbors() -> Vec<Neighbor> {
    if cfg!(target_os = "windows") {
        windows_arp().await
    } else if cfg!(target_os = "macos") {
        macos_arp().await
    } else {
        linux_arp().await
    }
}

async fn windows_arp() -> Vec<Neighbor> {
    let mut out = Vec::new();
    let text = match sys::run("arp", &["-a"], Duration::from_secs(4)).await {
        Some(t) => t,
        None => return out,
    };
    let mut current_iface: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // En-tête localisé : « Interface: 192.168.1.5 --- 0x5 » → on garde l'IP
        // comme nom d'interface, c'est stable quelle que soit la langue.
        if trimmed.to_ascii_lowercase().starts_with("interface") {
            if let Some(ip) = trimmed
                .split_whitespace()
                .find(|tok| tok.parse::<Ipv4Addr>().is_ok())
            {
                current_iface = Some(ip.to_string());
            }
            continue;
        }
        let cols: Vec<&str> = trimmed.split_whitespace().collect();
        if cols.len() < 2 {
            continue;
        }
        let ip = match cols[0].parse::<Ipv4Addr>() {
            Ok(ip) => ip,
            Err(_) => continue,
        };
        let mac = normalize_mac(cols[1]);
        let state = cols
            .get(2)
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_else(|| "dynamic".into());
        out.push(Neighbor {
            ip,
            mac,
            iface: current_iface.clone(),
            state,
        });
    }
    out
}

async fn macos_arp() -> Vec<Neighbor> {
    let mut out = Vec::new();
    let text = match sys::run("arp", &["-an"], Duration::from_secs(4)).await {
        Some(t) => t,
        None => return out,
    };
    for line in text.lines() {
        // « ? (192.168.1.1) at a4:91:b1:00:11:22 on en0 ifscope [ethernet] »
        let open = match line.find('(') {
            Some(p) => p,
            None => continue,
        };
        let close = match line.find(')') {
            Some(p) => p,
            None => continue,
        };
        if close <= open {
            continue;
        }
        let ip = match line[open + 1..close].parse::<Ipv4Addr>() {
            Ok(ip) => ip,
            Err(_) => continue,
        };
        let rest = &line[close + 1..];
        let mac = rest
            .split_whitespace()
            .skip_while(|t| *t != "at")
            .nth(1)
            .and_then(normalize_mac);
        let iface = rest
            .split_whitespace()
            .skip_while(|t| *t != "on")
            .nth(1)
            .map(|s| s.to_string());
        out.push(Neighbor {
            ip,
            mac,
            iface,
            state: "dynamic".into(),
        });
    }
    out
}

async fn linux_arp() -> Vec<Neighbor> {
    let mut out: Vec<Neighbor> = Vec::new();
    if let Ok(text) = std::fs::read_to_string("/proc/net/arp") {
        for line in text.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 6 {
                continue;
            }
            let ip = match cols[0].parse::<Ipv4Addr>() {
                Ok(ip) => ip,
                Err(_) => continue,
            };
            out.push(Neighbor {
                ip,
                mac: normalize_mac(cols[3]),
                iface: Some(cols[5].to_string()),
                state: if cols[2] == "0x0" {
                    "incomplete".into()
                } else {
                    "dynamic".into()
                },
            });
        }
    }
    // `ip neigh` apporte les entrées récentes et les états (REACHABLE, STALE…).
    if let Some(text) = sys::run("ip", &["-4", "neigh", "show"], Duration::from_secs(3)).await {
        for line in text.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 3 {
                continue;
            }
            let ip = match cols[0].parse::<Ipv4Addr>() {
                Ok(ip) => ip,
                Err(_) => continue,
            };
            let mac = cols
                .iter()
                .position(|c| *c == "lladdr")
                .and_then(|p| cols.get(p + 1))
                .and_then(|m| normalize_mac(m));
            let iface = cols
                .iter()
                .position(|c| *c == "dev")
                .and_then(|p| cols.get(p + 1))
                .map(|s| s.to_string());
            let state = cols
                .iter()
                .find(|c| c.chars().all(|ch| ch.is_ascii_uppercase() || ch == '_'))
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_else(|| "dynamic".into());
            match out.iter_mut().find(|n| n.ip == ip) {
                Some(existing) => {
                    if existing.mac.is_none() {
                        existing.mac = mac;
                    }
                    if !state.is_empty() {
                        existing.state = state;
                    }
                }
                None => out.push(Neighbor {
                    ip,
                    mac,
                    iface,
                    state,
                }),
            }
        }
    }
    out
}
