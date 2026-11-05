//! État partagé entre les commandes et la boucle de surveillance.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Mutex, RwLock};

use sectool_core::{Alert, Settings, Snapshot};
use sectool_firewall::Firewall;

/// Nombre d'événements conservés en mémoire.
pub const ALERT_HISTORY: usize = 300;

pub struct AppState {
    pub settings: RwLock<Settings>,
    /// Le pare-feu est protégé par un mutex asynchrone : l'application des
    /// règles peut attendre une autorisation système pendant plusieurs secondes.
    pub firewall: tokio::sync::Mutex<Firewall>,
    pub alerts: Mutex<VecDeque<Alert>>,
    pub snapshot: RwLock<Snapshot>,
    pub elevated: AtomicBool,
    pub scanning: AtomicBool,
    pub scan_requested: AtomicBool,
    pub last_scan_at: AtomicU64,
    pub last_scan_ms: AtomicU64,
}

impl AppState {
    pub fn new() -> Self {
        let settings = load_settings();
        Self {
            settings: RwLock::new(settings),
            firewall: tokio::sync::Mutex::new(Firewall::load()),
            alerts: Mutex::new(VecDeque::with_capacity(ALERT_HISTORY)),
            snapshot: RwLock::new(Snapshot {
                version: sectool_core::VERSION.to_string(),
                platform: sectool_net::sys::platform().to_string(),
                hostname: sectool_net::sys::hostname(),
                ..Default::default()
            }),
            elevated: AtomicBool::new(false),
            scanning: AtomicBool::new(false),
            scan_requested: AtomicBool::new(true),
            last_scan_at: AtomicU64::new(0),
            last_scan_ms: AtomicU64::new(0),
        }
    }

    pub fn settings_snapshot(&self) -> Settings {
        self.settings
            .read()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    pub fn push_alerts(&self, incoming: Vec<Alert>) {
        if incoming.is_empty() {
            return;
        }
        if let Ok(mut queue) = self.alerts.lock() {
            for alert in incoming {
                queue.push_front(alert);
            }
            while queue.len() > ALERT_HISTORY {
                queue.pop_back();
            }
        }
    }

    pub fn alerts_list(&self) -> Vec<Alert> {
        self.alerts
            .lock()
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Charge les réglages depuis `settings.json`.
pub fn load_settings() -> Settings {
    let path = sectool_core::paths::config_file("settings.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
        .unwrap_or_default()
}

/// Enregistre les réglages.
pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let path = sectool_core::paths::config_file("settings.json");
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}
