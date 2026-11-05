//! mDNS (Bonjour / Zeroconf) — découverte des noms et modèles d'appareils.
//!
//! Best effort : la requête est envoyée en multicast avec le bit « réponse
//! unicast » (QU), et on lit ce qui arrive pendant une courte fenêtre. Si rien
//! ne répond, on continue simplement sans ces informations.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;

/// Ce qu'un appareil a bien voulu annoncer.
#[derive(Debug, Clone)]
pub struct MdnsHost {
    pub ip: Ipv4Addr,
    pub name: Option<String>,
    pub services: Vec<String>,
    pub model: Option<String>,
}

const SERVICES: &[&str] = &[
    "_http._tcp.local",
    "_airplay._tcp.local",
    "_raop._tcp.local",
    "_ipp._tcp.local",
    "_printer._tcp.local",
    "_googlecast._tcp.local",
    "_hap._tcp.local",
    "_workstation._tcp.local",
    "_device-info._tcp.local",
    "_smb._tcp.local",
    "_afpovertcp._tcp.local",
    "_ssh._tcp.local",
    "_sonos._tcp.local",
    "_spotify-connect._tcp.local",
    "_companion-link._tcp.local",
];

/// Interroge le réseau local en mDNS.
pub async fn probe(window: Duration) -> Vec<MdnsHost> {
    let socket = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let _ = socket.set_broadcast(true);
    let packet = build_query(SERVICES);
    let dest: SocketAddr = (Ipv4Addr::new(224, 0, 0, 251), 5353).into();
    let _ = socket.send_to(&packet, dest).await;

    let mut buf = [0u8; 4096];
    let deadline = Instant::now() + window;
    let mut records: Vec<Record> = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await {
            Ok(Ok((len, _))) => {
                if let Ok(mut parsed) = parse_packet(&buf[..len]) {
                    records.append(&mut parsed);
                }
            }
            _ => break,
        }
    }
    aggregate(records)
}

#[derive(Debug, Clone)]
struct Record {
    name: String,
    kind: u16,
    value: Vec<u8>,
}

fn encode_name(name: &str, out: &mut Vec<u8>) {
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() {
            continue;
        }
        let bytes = label.as_bytes();
        let len = bytes.len().min(63);
        out.push(len as u8);
        out.extend_from_slice(&bytes[..len]);
    }
    out.push(0);
}

fn build_query(services: &[&str]) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(&[0, 0, 0, 0]); // ID 0, flags 0
    out.extend_from_slice(&(services.len() as u16).to_be_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // AN, NS, AR = 0
    for service in services {
        encode_name(service, &mut out);
        out.extend_from_slice(&12u16.to_be_bytes()); // PTR
        out.extend_from_slice(&(0x8001u16).to_be_bytes()); // IN + QU
    }
    out
}

fn read_name(buf: &[u8], start: usize) -> Option<(String, usize)> {
    let mut labels: Vec<String> = Vec::new();
    let mut pos = start;
    let mut jumped = false;
    let mut end = start;
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 128 {
            return None;
        }
        let len = *buf.get(pos)? as usize;
        if len == 0 {
            pos += 1;
            if !jumped {
                end = pos;
            }
            break;
        }
        if len & 0xC0 == 0xC0 {
            let b2 = *buf.get(pos + 1)? as usize;
            let pointer = ((len & 0x3F) << 8) | b2;
            if !jumped {
                end = pos + 2;
            }
            jumped = true;
            pos = pointer;
            continue;
        }
        let slice = buf.get(pos + 1..pos + 1 + len)?;
        labels.push(String::from_utf8_lossy(slice).to_string());
        pos += 1 + len;
        if !jumped {
            end = pos;
        }
    }
    Some((labels.join("."), end))
}

fn parse_packet(buf: &[u8]) -> Result<Vec<Record>, ()> {
    if buf.len() < 12 {
        return Err(());
    }
    let qd = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    let an = u16::from_be_bytes([buf[6], buf[7]]) as usize;
    let ns = u16::from_be_bytes([buf[8], buf[9]]) as usize;
    let ar = u16::from_be_bytes([buf[10], buf[11]]) as usize;

    let mut pos = 12;
    for _ in 0..qd {
        let (_, next) = read_name(buf, pos).ok_or(())?;
        pos = next + 4; // type + class
    }
    let mut out = Vec::new();
    for _ in 0..(an + ns + ar) {
        let (name, next) = match read_name(buf, pos) {
            Some(v) => v,
            None => break,
        };
        pos = next;
        if pos + 10 > buf.len() {
            break;
        }
        let kind = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let rdlen = u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]) as usize;
        pos += 10;
        if pos + rdlen > buf.len() {
            break;
        }
        let value = buf[pos..pos + rdlen].to_vec();
        out.push(Record { name, kind, value });
        pos += rdlen;
    }
    Ok(out)
}

fn aggregate(records: Vec<Record>) -> Vec<MdnsHost> {
    // 1. A / AAAA : nom → IP
    let mut name_to_ip: HashMap<String, Ipv4Addr> = HashMap::new();
    // 2. SRV : instance → cible
    let mut srv_target: HashMap<String, String> = HashMap::new();
    // 3. TXT : nom → modèle
    let mut txt_model: HashMap<String, String> = HashMap::new();
    // 4. PTR : service → instances
    let mut ptrs: Vec<(String, String)> = Vec::new();

    for rec in &records {
        match rec.kind {
            1 if rec.value.len() == 4 => {
                let ip = Ipv4Addr::new(rec.value[0], rec.value[1], rec.value[2], rec.value[3]);
                name_to_ip.insert(rec.name.to_ascii_lowercase(), ip);
            }
            33 if rec.value.len() >= 7 => {
                if let Some((target, _)) = read_name(&rec.value, 6) {
                    srv_target.insert(rec.name.to_ascii_lowercase(), target.to_ascii_lowercase());
                }
            }
            16 => {
                let text = String::from_utf8_lossy(&rec.value).replace('\0', "|");
                let model = text
                    .split('|')
                    .find_map(|kv| {
                        let kv = kv.trim();
                        for key in ["model=", "md=", "mdl=", "ty=", "usb_MDL="] {
                            if let Some(v) = kv.strip_prefix(key) {
                                return Some(v.replace('\\', "").trim().to_string());
                            }
                        }
                        None
                    })
                    .filter(|m| !m.is_empty());
                if let Some(m) = model {
                    txt_model.entry(rec.name.to_ascii_lowercase()).or_insert(m);
                }
            }
            12 => {
                if let Some((target, _)) = read_name(&rec.value, 0) {
                    ptrs.push((rec.name.to_ascii_lowercase(), target));
                }
            }
            _ => {}
        }
    }

    let mut by_ip: HashMap<Ipv4Addr, MdnsHost> = HashMap::new();
    for (service, instance) in ptrs {
        let instance_key = instance.to_ascii_lowercase();
        let host = instance_key
            .strip_suffix(&format!(".{service}"))
            .map(|s| s.to_string())
            .unwrap_or_else(|| instance_key.clone());
        let target = srv_target.get(&instance_key);
        let ip = target
            .and_then(|t| name_to_ip.get(t).copied())
            .or_else(|| name_to_ip.get(&instance_key).copied())
            .or_else(|| name_to_ip.get(&host).copied());
        let ip = match ip {
            Some(ip) => ip,
            None => continue,
        };
        let service_name = service
            .split('.')
            .next()
            .unwrap_or(&service)
            .trim_start_matches('_')
            .to_string();
        let entry = by_ip.entry(ip).or_insert_with(|| MdnsHost {
            ip,
            name: None,
            services: Vec::new(),
            model: None,
        });
        if !entry.services.contains(&service_name) {
            entry.services.push(service_name);
        }
        if entry.name.is_none() {
            let pretty = instance
                .split('.')
                .next()
                .unwrap_or(&instance)
                .replace('\\', "")
                .replace(" - ", " ");
            if !pretty.is_empty() && pretty.to_ascii_lowercase() != "localhost" {
                // « Salon - AirPlay » → on garde la partie parlante.
                entry.name = Some(pretty);
            }
        }
        if entry.model.is_none() {
            entry.model = txt_model.get(&instance_key).cloned();
        }
    }
    by_ip.into_values().collect()
}
