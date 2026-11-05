//! Petits utilitaires réseau sans dépendance système.

use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Horodatage en millisecondes depuis l'époque Unix.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Vrai si l'adresse appartient à une plage privée ou locale.
pub fn is_private(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

/// Vrai si l'adresse est un groupe multicast.
pub fn is_multicast(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_multicast() || v4.is_broadcast(),
        IpAddr::V6(v6) => v6.is_multicast(),
    }
}

/// Classe une adresse distante pour l'affichage.
pub fn ip_scope(ip: &IpAddr) -> crate::connection::Scope {
    use crate::connection::Scope;
    if ip.is_unspecified() {
        return Scope::Local;
    }
    if is_multicast(ip) {
        return Scope::Multicast;
    }
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback() || v4.is_private() || v4.is_link_local() {
                Scope::Lan
            } else {
                Scope::Internet
            }
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            let ula = (s[0] & 0xfe00) == 0xfc00;
            let link_local = (s[0] & 0xffc0) == 0xfe80;
            if ula || link_local || v6.is_loopback() {
                Scope::Lan
            } else {
                Scope::Internet
            }
        }
    }
}

/// Normalise une adresse MAC : minuscules, séparateur `:`.
pub fn normalize_mac(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if cleaned.len() != 12 {
        return None;
    }
    if cleaned == "000000000000" || cleaned == "ffffffffffff" {
        return None;
    }
    let mut out = String::with_capacity(17);
    for (i, chunk) in cleaned.as_bytes().chunks(2).enumerate() {
        if i > 0 {
            out.push(':');
        }
        out.push_str(std::str::from_utf8(chunk).ok()?);
    }
    Some(out)
}

/// Les 6 premiers caractères d'une MAC, sans séparateur (`a4c138`).
pub fn oui_prefix(mac: &str) -> Option<String> {
    let norm = normalize_mac(mac)?;
    Some(norm.split(':').take(3).collect::<Vec<_>>().join(""))
}

/// Vrai si l'adresse MAC est une adresse aléatoire (bit local mis à 1).
pub fn is_randomized_mac(mac: &str) -> bool {
    match normalize_mac(mac) {
        Some(m) => {
            let first = u8::from_str_radix(&m[0..2], 16).unwrap_or(0);
            first & 0b10 != 0
        }
        None => false,
    }
}
