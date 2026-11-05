//! Débit réseau (échantillons et historique).

use serde::{Deserialize, Serialize};

/// Débit instantané d'une interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IfaceTraffic {
    pub name: String,
    pub rx_bps: u64,
    pub tx_bps: u64,
    pub rx_total: u64,
    pub tx_total: u64,
}

/// Un point du graphique temps réel.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficPoint {
    pub ts: i64,
    pub rx_bps: u64,
    pub tx_bps: u64,
}

/// Vue complète du trafic à un instant donné.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficSnapshot {
    pub ts: i64,
    pub interfaces: Vec<IfaceTraffic>,
    pub history: Vec<TrafficPoint>,
    pub rx_bps: u64,
    pub tx_bps: u64,
    pub rx_total: u64,
    pub tx_total: u64,
}

/// Alias conservé pour l'API publique.
pub type TrafficSample = TrafficPoint;
