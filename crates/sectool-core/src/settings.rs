//! Réglages persistants.

use serde::{Deserialize, Serialize};

fn d_interval() -> u64 {
    5
}
fn d_timeout() -> u64 {
    350
}
fn d_true() -> bool {
    true
}
fn d_ports() -> Vec<u16> {
    vec![
        22, 53, 80, 443, 445, 554, 631, 1883, 3389, 5000, 5001, 8008, 8009, 8080, 9100, 32400,
        62078,
    ]
}
fn d_max_hosts() -> usize {
    512
}

/// Préférences de l'application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Période entre deux balayages du réseau, en secondes.
    #[serde(default = "d_interval")]
    pub scan_interval_secs: u64,
    /// Délai d'attente d'une sonde TCP, en millisecondes.
    #[serde(default = "d_timeout")]
    pub probe_timeout_ms: u64,
    /// Ports testés sur chaque appareil.
    #[serde(default = "d_ports")]
    pub probe_ports: Vec<u16>,
    #[serde(default = "d_true")]
    pub enable_mdns: bool,
    #[serde(default = "d_true")]
    pub enable_reverse_dns: bool,
    #[serde(default = "d_true")]
    pub enable_arp_sweep: bool,
    /// Nombre maximal d'hôtes balayés par sous-réseau.
    #[serde(default = "d_max_hosts")]
    pub max_hosts: usize,
    /// Prévenir à chaque nouvelle connexion sortante.
    #[serde(default)]
    pub alert_new_outbound: bool,
    /// Forcer un sous-réseau (sinon détection automatique).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subnet_override: Option<String>,
    #[serde(default)]
    pub ui: UiSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scan_interval_secs: d_interval(),
            probe_timeout_ms: d_timeout(),
            probe_ports: d_ports(),
            enable_mdns: true,
            enable_reverse_dns: true,
            enable_arp_sweep: true,
            max_hosts: d_max_hosts(),
            alert_new_outbound: false,
            subnet_override: None,
            ui: UiSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSettings {
    /// Réduire les animations (accessibilité).
    #[serde(default)]
    pub reduce_motion: bool,
    /// Afficher les adresses IPv6 dans les tableaux.
    #[serde(default)]
    pub show_ipv6: bool,
    /// Carte du réseau ou liste compacte au démarrage.
    #[serde(default = "d_true")]
    pub show_map: bool,
    /// Thème de l'interface : `dark` ou `light`.
    #[serde(default = "d_theme")]
    pub theme: String,
    /// Bip sonore à chaque alerte importante.
    #[serde(default = "d_true")]
    pub sound_enabled: bool,
}

fn d_theme() -> String {
    "dark".to_string()
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            reduce_motion: false,
            show_ipv6: false,
            show_map: true,
            theme: d_theme(),
            sound_enabled: true,
        }
    }
}
