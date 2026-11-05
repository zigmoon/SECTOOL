//! Instantané complet envoyé à l'interface.

use serde::{Deserialize, Serialize};

use crate::alert::Alert;
use crate::connection::Connection;
use crate::device::Device;
use crate::firewall::FirewallStatus;
use crate::traffic::TrafficSnapshot;

/// Une interface réseau de la machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    pub ipv4: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ipv6: Vec<String>,
    /// Sous-réseau au format CIDR, ex. `192.168.1.0/24`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnet: Option<String>,
    pub is_up: bool,
    pub is_default: bool,
}

/// Compteurs affichés dans les tuiles du tableau de bord.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub devices: usize,
    pub devices_online: usize,
    pub connections: usize,
    pub connections_internet: usize,
    pub listening: usize,
    pub alerts: usize,
    pub alerts_critical: usize,
    pub blocked_devices: usize,
    pub active_rules: usize,
}

/// Tout ce que l'interface a besoin de savoir, en une seule structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub ts: i64,
    pub version: String,
    pub platform: String,
    pub hostname: String,
    pub is_elevated: bool,
    pub scanning: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_scan_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_scan_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_ip: Option<String>,
    #[serde(default)]
    pub interfaces: Vec<InterfaceInfo>,
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub alerts: Vec<Alert>,
    pub firewall: FirewallStatus,
    pub traffic: TrafficSnapshot,
    pub stats: Stats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
