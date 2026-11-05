//! Connexions réseau actives (sockets).

use serde::{Deserialize, Serialize};

/// Zone atteinte par une connexion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Boucle locale ou socket en écoute.
    Local,
    /// Adresse privée du réseau local.
    Lan,
    /// Internet.
    Internet,
    /// Multicast / broadcast.
    Multicast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnState {
    Listen,
    Established,
    SynSent,
    SynRecv,
    FinWait,
    TimeWait,
    CloseWait,
    Closed,
    Udp,
    Unknown,
}

impl ConnState {
    pub fn from_raw(raw: &str) -> Self {
        let s = raw.trim().to_ascii_uppercase();
        match s.as_str() {
            "LISTEN" | "LISTENING" | "ECOUTE" => ConnState::Listen,
            "ESTAB" | "ESTABLISHED" | "ETABLIE" => ConnState::Established,
            "SYN_SENT" | "SYN-SENT" => ConnState::SynSent,
            "SYN_RECV" | "SYN-RECEIVED" => ConnState::SynRecv,
            "FIN_WAIT1" | "FIN_WAIT2" | "FIN-WAIT-1" | "FIN-WAIT-2" => ConnState::FinWait,
            "TIME_WAIT" | "TIME-WAIT" => ConnState::TimeWait,
            "CLOSE_WAIT" | "CLOSE-WAIT" => ConnState::CloseWait,
            "CLOSED" | "CLOSE" => ConnState::Closed,
            "UDP" | "UNCONN" | "UNCONNECTED" => ConnState::Udp,
            _ => ConnState::Unknown,
        }
    }

    pub fn is_open(self) -> bool {
        matches!(
            self,
            ConnState::Listen | ConnState::Established | ConnState::Udp
        )
    }
}

/// Une ligne de socket vue sur la machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    /// `tcp` ou `udp`.
    pub proto: String,
    pub local_addr: String,
    pub local_port: u16,
    pub remote_addr: String,
    pub remote_port: u16,
    pub state: ConnState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    pub scope: Scope,
    /// Premier instant où cette connexion a été vue.
    pub first_seen: i64,
    pub last_seen: i64,
}

impl Connection {
    pub fn remote_label(&self) -> String {
        if self.remote_port == 0 {
            self.remote_addr.clone()
        } else if self.remote_addr.contains(':') {
            format!("[{}]:{}", self.remote_addr, self.remote_port)
        } else {
            format!("{}:{}", self.remote_addr, self.remote_port)
        }
    }

    pub fn local_label(&self) -> String {
        if self.local_addr.contains(':') {
            format!("[{}]:{}", self.local_addr, self.local_port)
        } else {
            format!("{}:{}", self.local_addr, self.local_port)
        }
    }
}
