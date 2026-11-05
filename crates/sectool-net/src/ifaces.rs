//! Interfaces réseau de la machine locale.

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use if_addrs::{get_if_addrs, IfAddr};

use crate::sys;

/// Une interface locale, réduite à ce dont SECTOOL a besoin.
#[derive(Debug, Clone)]
pub struct LocalIface {
    pub name: String,
    pub mac: Option<String>,
    pub ipv4: Vec<(Ipv4Addr, u8)>,
    pub ipv6: Vec<Ipv6Addr>,
    pub is_up: bool,
}

impl LocalIface {
    /// Premier sous-réseau IPv4 utilisable (hors 127.0.0.0/8 et lien local).
    pub fn best_v4(&self) -> Option<(Ipv4Addr, u8)> {
        self.ipv4
            .iter()
            .copied()
            .find(|(ip, prefix)| !ip.is_loopback() && !ip.is_link_local() && *prefix <= 30)
            .or_else(|| self.ipv4.first().copied())
    }
}

/// Adresse réseau d'un CIDR, ex. `192.168.1.37` + 24 → `192.168.1.0/24`.
pub fn network_cidr(ip: Ipv4Addr, prefix: u8) -> String {
    let mask = if prefix == 0 {
        0u32
    } else {
        u32::MAX << (32 - prefix.min(32))
    };
    let net = Ipv4Addr::from(u32::from(ip) & mask);
    format!("{net}/{prefix}")
}

/// Liste les interfaces locales.
pub fn local_interfaces() -> Vec<LocalIface> {
    let mut out: Vec<LocalIface> = Vec::new();
    let addrs = match get_if_addrs() {
        Ok(a) => a,
        Err(_) => return out,
    };
    for iface in addrs {
        if iface.name.starts_with("lo") && cfg!(not(windows)) {
            continue;
        }
        let entry = match out.iter_mut().find(|i| i.name == iface.name) {
            Some(e) => e,
            None => {
                out.push(LocalIface {
                    name: iface.name.clone(),
                    mac: None,
                    ipv4: Vec::new(),
                    ipv6: Vec::new(),
                    is_up: true,
                });
                out.last_mut().unwrap()
            }
        };
        match iface.addr {
            IfAddr::V4(v4) => entry.ipv4.push((v4.ip, v4.prefixlen)),
            IfAddr::V6(v6) => entry.ipv6.push(v6.ip),
        }
    }
    out
}

/// Adresse MAC de chaque interface (best effort, dépend de l'OS).
pub async fn local_macs() -> HashMap<String, String> {
    let mut map = HashMap::new();
    if cfg!(target_os = "windows") {
        let script = "Get-NetAdapter | Select-Object Name,MacAddress | ConvertTo-Json -Compress";
        if let Some(out) = sys::run(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", script],
            Duration::from_secs(8),
        )
        .await
        {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(out.trim()) {
                let items = match value {
                    serde_json::Value::Array(a) => a,
                    other => vec![other],
                };
                for item in items {
                    let name = item
                        .get("Name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let mac = item
                        .get("MacAddress")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if let Some(norm) = sectool_core::normalize_mac(&mac) {
                        map.insert(name, norm);
                    }
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        if let Some(out) = sys::run("ifconfig", &["-a"], Duration::from_secs(4)).await {
            let mut current = String::new();
            for line in out.lines() {
                if !line.starts_with(char::is_whitespace) {
                    current = line.split(':').next().unwrap_or("").to_string();
                } else if let Some(rest) = line.trim().strip_prefix("ether ") {
                    if let Some(mac) = sectool_core::normalize_mac(rest.split_whitespace().next().unwrap_or("")) {
                        map.insert(current.clone(), mac);
                    }
                }
            }
        }
    } else {
        for iface in local_interfaces() {
            let path = format!("/sys/class/net/{}/address", iface.name);
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Some(mac) = sectool_core::normalize_mac(text.trim()) {
                    map.insert(iface.name.clone(), mac);
                }
            }
        }
    }
    map
}

/// Toutes les adresses IPv4 locales (pour ne pas se scanner soi-même).
pub fn local_v4_addresses(ifaces: &[LocalIface]) -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    for i in ifaces {
        for (ip, _) in &i.ipv4 {
            if ip.is_loopback() || ip.is_link_local() {
                continue;
            }
            out.push(*ip);
        }
    }
    out
}
