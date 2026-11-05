//! Boucle de surveillance : balayage, sockets, trafic, événements.

use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use sectool_core::traffic::{TrafficPoint, TrafficSnapshot};
use sectool_core::{now_ms, Snapshot, Stats, VERSION};
use sectool_firewall::render::active_count;
use sectool_net::sockets;
use sectool_net::scan::Scanner;
use sectool_net::traffic::TrafficReader;

use crate::state::AppState;

/// Nombre de points conservés pour le graphique (≈ 3 minutes à 1 s).
const HISTORY: usize = 180;

/// Démarre la boucle de surveillance en tâche de fond.
pub fn spawn(app: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut scanner = Scanner::new();
        let mut traffic = TrafficReader::new();
        let mut history: VecDeque<TrafficPoint> = VecDeque::with_capacity(HISTORY);
        let mut tick: u64 = 0;
        let mut devices = Vec::new();
        let mut interfaces = Vec::new();
        let mut gateway: Option<String> = None;
        let mut local_ip: Option<String> = None;
        let mut connections = Vec::new();

        // Première détection des droits (une seule fois, c'est coûteux).
        let elevated = sectool_net::sys::is_elevated().await;
        state.elevated.store(elevated, Ordering::SeqCst);

        loop {
            let settings = state.settings_snapshot();
            let scan_interval = settings.scan_interval_secs.clamp(2, 300);
            let wanted = tick % scan_interval == 0
                || state.scan_requested.swap(false, Ordering::SeqCst);

            if wanted && !state.scanning.swap(true, Ordering::SeqCst) {
                let outcome = scanner.run(&settings).await;
                devices = outcome.devices;
                interfaces = outcome.interfaces;
                gateway = outcome.gateway;
                local_ip = outcome.local_ip;
                state
                    .last_scan_ms
                    .store(outcome.duration_ms, Ordering::SeqCst);
                state
                    .last_scan_at
                    .store(now_ms() as u64, Ordering::SeqCst);
                if let Some(err) = outcome.error {
                    state.push_alerts(vec![sectool_core::Alert::new(
                        sectool_core::AlertLevel::Warn,
                        sectool_core::AlertKind::ScanError,
                        "Balayage incomplet",
                        err,
                    )]);
                }
                state.push_alerts(outcome.alerts);
                state.scanning.store(false, Ordering::SeqCst);
            }

            // Le trafic se rafraîchit à chaque seconde.
            let ifaces = traffic.sample().await;
            let rx_bps: u64 = ifaces.iter().map(|i| i.rx_bps).sum();
            let tx_bps: u64 = ifaces.iter().map(|i| i.tx_bps).sum();
            history.push_back(TrafficPoint {
                ts: now_ms(),
                rx_bps,
                tx_bps,
            });
            while history.len() > HISTORY {
                history.pop_front();
            }

            // Les sockets sont plus coûteuses : un passage sur deux.
            if tick % 2 == 0 {
                connections = sockets::snapshot().await.connections;
            }

            let (firewall_status, rules_active) = {
                let firewall = state.firewall.lock().await;
                (
                    firewall.status(state.elevated.load(Ordering::SeqCst)),
                    active_count(firewall.rules()),
                )
            };

            // Un appareil est marqué « bloqué » s'il est visé par une règle.
            let blocked_ips: std::collections::HashSet<String> = {
                let firewall = state.firewall.lock().await;
                firewall
                    .rules()
                    .iter()
                    .filter(|r| r.enabled && r.action == sectool_core::RuleAction::Block)
                    .filter_map(|r| r.remote.clone())
                    .collect()
            };
            for device in devices.iter_mut() {
                device.blocked = device
                    .ip
                    .as_ref()
                    .map(|ip| blocked_ips.contains(ip))
                    .unwrap_or(false);
            }

            let alert_list = state.alerts_list();
            let stats = Stats {
                devices: devices.len(),
                devices_online: devices.iter().filter(|d| d.online).count(),
                connections: connections.iter().filter(|c| c.state.is_open()).count(),
                connections_internet: connections
                    .iter()
                    .filter(|c| c.scope == sectool_core::Scope::Internet)
                    .count(),
                listening: connections
                    .iter()
                    .filter(|c| c.state == sectool_core::ConnState::Listen)
                    .count(),
                alerts: alert_list.len(),
                alerts_critical: alert_list
                    .iter()
                    .filter(|a| a.level == sectool_core::AlertLevel::Critical)
                    .count(),
                blocked_devices: devices.iter().filter(|d| d.blocked).count(),
                active_rules: rules_active,
            };

            let snapshot = Snapshot {
                ts: now_ms(),
                version: VERSION.to_string(),
                platform: sectool_net::sys::platform().to_string(),
                hostname: sectool_net::sys::hostname(),
                is_elevated: state.elevated.load(Ordering::SeqCst),
                scanning: state.scanning.load(Ordering::SeqCst),
                last_scan_ms: Some(state.last_scan_ms.load(Ordering::SeqCst)),
                last_scan_at: Some(state.last_scan_at.load(Ordering::SeqCst) as i64),
                gateway: gateway.clone(),
                local_ip: local_ip.clone(),
                interfaces: interfaces.clone(),
                devices: devices.clone(),
                connections: connections.clone(),
                alerts: alert_list.clone(),
                firewall: firewall_status,
                traffic: TrafficSnapshot {
                    ts: now_ms(),
                    interfaces: ifaces,
                    history: history.iter().copied().collect(),
                    rx_bps,
                    tx_bps,
                    rx_total: 0,
                    tx_total: 0,
                },
                stats,
                error: None,
            };

            if let Ok(mut slot) = state.snapshot.write() {
                *slot = snapshot.clone();
            }
            let _ = app.emit("sectool://snapshot", &snapshot);

            tick = tick.wrapping_add(1);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}
