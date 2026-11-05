//! Commandes exposées à l'interface Astro.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use sectool_core::firewall::{ApplyPlan, Direction, FirewallRule, Proto, RuleAction};
use sectool_core::{now_ms, Alert, Device, FirewallStatus, Settings, Snapshot};
use sectool_firewall::Firewall;

use crate::state::{save_settings, AppState};

/// Résultat d'une action sur le pare-feu.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub ok: bool,
    pub message: String,
    pub log: String,
    pub rules: Vec<FirewallRule>,
    pub status: FirewallStatus,
    pub at: i64,
}

/// Ce que l'interface envoie pour créer ou modifier une règle.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default = "default_action")]
    pub action: RuleAction,
    #[serde(default = "default_direction")]
    pub direction: Direction,
    #[serde(default = "default_proto")]
    pub protocol: Proto,
    #[serde(default)]
    pub remote: Option<String>,
    #[serde(default)]
    pub local_ports: Option<String>,
    #[serde(default)]
    pub remote_ports: Option<String>,
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub auto: bool,
}

fn default_action() -> RuleAction {
    RuleAction::Block
}
fn default_direction() -> Direction {
    Direction::In
}
fn default_proto() -> Proto {
    Proto::Tcp
}
fn default_true() -> bool {
    true
}

fn status_of(firewall: &Firewall, elevated: bool) -> FirewallStatus {
    firewall.status(elevated)
}

// ───────────────────────── Lecture ─────────────────────────────────────────

#[tauri::command]
pub async fn get_snapshot(state: State<'_, Arc<AppState>>) -> Result<Snapshot, String> {
    let snapshot = state
        .snapshot
        .read()
        .map(|s| s.clone())
        .map_err(|_| "état indisponible".to_string())?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn get_devices(state: State<'_, Arc<AppState>>) -> Result<Vec<Device>, String> {
    Ok(state
        .snapshot
        .read()
        .map(|s| s.devices.clone())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn get_alerts(
    state: State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<Alert>, String> {
    let mut list = state.alerts_list();
    if let Some(limit) = limit {
        list.truncate(limit);
    }
    Ok(list)
}

#[tauri::command]
pub async fn clear_alerts(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if let Ok(mut queue) = state.alerts.lock() {
        queue.clear();
    }
    Ok(())
}

#[tauri::command]
pub async fn scan_now(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.scan_requested.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, Arc<AppState>>) -> Result<Settings, String> {
    Ok(state.settings_snapshot())
}

#[tauri::command]
pub async fn save_settings_cmd(
    state: State<'_, Arc<AppState>>,
    settings: Settings,
) -> Result<Settings, String> {
    let clean = Settings {
        scan_interval_secs: settings.scan_interval_secs.clamp(2, 300),
        probe_timeout_ms: settings.probe_timeout_ms.clamp(80, 3000),
        max_hosts: settings.max_hosts.clamp(16, 4096),
        probe_ports: settings.probe_ports.into_iter().take(64).collect(),
        ..settings
    };
    save_settings(&clean)?;
    if let Ok(mut slot) = state.settings.write() {
        *slot = clean.clone();
    }
    state.scan_requested.store(true, Ordering::SeqCst);
    Ok(clean)
}

// ───────────────────────── Pare-feu ────────────────────────────────────────

#[tauri::command]
pub async fn get_rules(state: State<'_, Arc<AppState>>) -> Result<Vec<FirewallRule>, String> {
    let firewall = state.firewall.lock().await;
    Ok(firewall.rules().to_vec())
}

#[tauri::command]
pub async fn get_firewall_status(
    state: State<'_, Arc<AppState>>,
) -> Result<FirewallStatus, String> {
    let firewall = state.firewall.lock().await;
    Ok(status_of(&firewall, state.elevated.load(Ordering::SeqCst)))
}

#[tauri::command]
pub async fn preview_rules(state: State<'_, Arc<AppState>>) -> Result<ApplyPlan, String> {
    let firewall = state.firewall.lock().await;
    Ok(firewall.plan(state.elevated.load(Ordering::SeqCst)))
}

#[tauri::command]
pub async fn save_rule(
    state: State<'_, Arc<AppState>>,
    rule: RuleInput,
) -> Result<Vec<FirewallRule>, String> {
    if rule.name.trim().is_empty() {
        return Err("Donnez un nom à la règle.".into());
    }
    let ts = now_ms();
    let id = rule
        .id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("r{ts}"));
    let created = {
        let firewall = state.firewall.lock().await;
        firewall.find(&id).map(|r| r.created).unwrap_or(ts)
    };
    let entry = FirewallRule {
        id,
        name: rule.name.trim().to_string(),
        action: rule.action,
        direction: rule.direction,
        protocol: rule.protocol,
        remote: clean_opt(rule.remote),
        local_ports: clean_opt(rule.local_ports),
        remote_ports: clean_opt(rule.remote_ports),
        app: clean_opt(rule.app),
        note: clean_opt(rule.note),
        enabled: rule.enabled,
        auto: rule.auto,
        created,
        last_applied: None,
    };
    let mut firewall = state.firewall.lock().await;
    firewall.upsert(entry);
    Ok(firewall.rules().to_vec())
}

fn clean_opt(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[tauri::command]
pub async fn delete_rule(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Vec<FirewallRule>, String> {
    let mut firewall = state.firewall.lock().await;
    firewall.delete(&id);
    Ok(firewall.rules().to_vec())
}

#[tauri::command]
pub async fn toggle_rule(
    state: State<'_, Arc<AppState>>,
    id: String,
    enabled: bool,
) -> Result<Vec<FirewallRule>, String> {
    let mut firewall = state.firewall.lock().await;
    firewall.set_enabled(&id, enabled);
    Ok(firewall.rules().to_vec())
}

#[tauri::command]
pub async fn apply_rules(state: State<'_, Arc<AppState>>) -> Result<ActionResult, String> {
    let elevated = sectool_net::sys::is_elevated().await;
    state.elevated.store(elevated, Ordering::SeqCst);
    let mut firewall = state.firewall.lock().await;
    let report = firewall.apply(elevated).await;
    let status = status_of(&firewall, elevated);
    let rules = firewall.rules().to_vec();
    drop(firewall);
    if !report.ok {
        state.push_alerts(vec![Alert::new(
            sectool_core::AlertLevel::Critical,
            sectool_core::AlertKind::FirewallError,
            "Application du pare-feu impossible",
            report.message.clone(),
        )]);
    } else {
        state.push_alerts(vec![Alert::new(
            sectool_core::AlertLevel::Info,
            sectool_core::AlertKind::FirewallApplied,
            "Règles appliquées",
            report.message.clone(),
        )]);
    }
    Ok(ActionResult {
        ok: report.ok,
        message: report.message,
        log: report.log,
        rules,
        status,
        at: now_ms(),
    })
}

#[tauri::command]
pub async fn flush_rules(state: State<'_, Arc<AppState>>) -> Result<ActionResult, String> {
    let elevated = sectool_net::sys::is_elevated().await;
    state.elevated.store(elevated, Ordering::SeqCst);
    let mut firewall = state.firewall.lock().await;
    let report = firewall.flush(elevated).await;
    let status = status_of(&firewall, elevated);
    let rules = firewall.rules().to_vec();
    drop(firewall);
    state.push_alerts(vec![Alert::new(
        if report.ok {
            sectool_core::AlertLevel::Info
        } else {
            sectool_core::AlertLevel::Critical
        },
        if report.ok {
            sectool_core::AlertKind::FirewallApplied
        } else {
            sectool_core::AlertKind::FirewallError
        },
        "Pare-feu remis à zéro",
        report.message.clone(),
    )]);
    Ok(ActionResult {
        ok: report.ok,
        message: report.message,
        log: report.log,
        rules,
        status,
        at: now_ms(),
    })
}

/// Bloque (ou débloque) un appareil en une seule action.
#[tauri::command]
pub async fn set_device_blocked(
    state: State<'_, Arc<AppState>>,
    ip: String,
    label: String,
    blocked: bool,
) -> Result<Vec<FirewallRule>, String> {
    if ip.trim().is_empty() {
        return Err("Adresse IP inconnue pour cet appareil.".into());
    }
    let ip = ip.trim().to_string();
    let mut firewall = state.firewall.lock().await;
    let existing: Vec<String> = firewall
        .rules()
        .iter()
        .filter(|r| r.remote.as_deref() == Some(ip.as_str()) && r.auto)
        .map(|r| r.id.clone())
        .collect();
    for id in existing {
        firewall.delete(&id);
    }
    if blocked {
        let ts = now_ms();
        let rule = FirewallRule {
            id: format!("d{ts}"),
            name: format!("Bloquer {label}"),
            action: RuleAction::Block,
            direction: Direction::Both,
            protocol: Proto::Any,
            remote: Some(ip.clone()),
            local_ports: None,
            remote_ports: None,
            app: None,
            note: Some("Créé depuis la carte du réseau".into()),
            enabled: true,
            auto: true,
            created: ts,
            last_applied: None,
        };
        firewall.upsert(rule);
    }
    Ok(firewall.rules().to_vec())
}

// ───────────────────────── Divers ──────────────────────────────────────────

#[tauri::command]
pub async fn is_elevated(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let elevated = sectool_net::sys::is_elevated().await;
    state.elevated.store(elevated, Ordering::SeqCst);
    Ok(elevated)
}

#[tauri::command]
pub async fn export_report(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let snapshot = state
        .snapshot
        .read()
        .map(|s| s.clone())
        .unwrap_or_default();
    let rules = {
        let firewall = state.firewall.lock().await;
        firewall.rules().to_vec()
    };
    let payload = serde_json::json!({
        "application": "SECTOOL",
        "version": sectool_core::VERSION,
        "generatedAt": now_ms(),
        "host": snapshot.hostname,
        "platform": snapshot.platform,
        "gateway": snapshot.gateway,
        "interfaces": snapshot.interfaces,
        "devices": snapshot.devices,
        "connections": snapshot.connections,
        "alerts": snapshot.alerts,
        "rules": rules,
        "firewall": snapshot.firewall,
    });
    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}
