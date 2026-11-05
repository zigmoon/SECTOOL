//! Journal d'événements et alertes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertLevel {
    Info,
    Warn,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    NewDevice,
    DeviceBack,
    DeviceOffline,
    MacChange,
    NewOutbound,
    NewListening,
    ScanError,
    FirewallApplied,
    FirewallError,
    RuleMatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub id: String,
    pub ts: i64,
    pub level: AlertLevel,
    pub kind: AlertKind,
    pub title: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default)]
    pub seen: bool,
}

impl Alert {
    pub fn new(
        level: AlertLevel,
        kind: AlertKind,
        title: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        let ts = crate::now_ms();
        Self {
            id: format!("a{ts}-{}", (ts as u64) % 9973),
            ts,
            level,
            kind,
            title: title.into(),
            detail: detail.into(),
            device_id: None,
            seen: false,
        }
    }

    pub fn with_device(mut self, id: impl Into<String>) -> Self {
        self.device_id = Some(id.into());
        self
    }

    pub fn level_str(&self) -> &'static str {
        match self.level {
            AlertLevel::Info => "info",
            AlertLevel::Warn => "warn",
            AlertLevel::Critical => "critical",
        }
    }
}
