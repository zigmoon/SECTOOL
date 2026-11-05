//! Modèle de règles du pare-feu logiciel.
//!
//! Une règle SECTOOL est volontairement simple : elle décrit un trafic à
//! autoriser ou à bloquer. Chaque système d'exploitation a ensuite son propre
//! moteur (netsh, nftables, pf) et le crate `sectool-firewall` traduit.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Block,
}

impl RuleAction {
    pub fn as_str(self) -> &'static str {
        match self {
            RuleAction::Allow => "allow",
            RuleAction::Block => "block",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    In,
    Out,
    Both,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::In => "in",
            Direction::Out => "out",
            Direction::Both => "both",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Proto {
    Tcp,
    Udp,
    Any,
}

impl Proto {
    pub fn as_str(self) -> &'static str {
        match self {
            Proto::Tcp => "tcp",
            Proto::Udp => "udp",
            Proto::Any => "any",
        }
    }
}

/// Une règle de pare-feu créée par l'utilisateur.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirewallRule {
    pub id: String,
    pub name: String,
    pub action: RuleAction,
    pub direction: Direction,
    pub protocol: Proto,
    /// CIDR ou adresse simple, ex. `192.168.1.42` ou `10.0.0.0/8`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// Liste de ports locaux, ex. `445,139` ou `8000-8100`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_ports: Option<String>,
    /// Liste de ports distants.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_ports: Option<String>,
    /// Chemin complet d'un exécutable (Windows / macOS surtout).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Règle créée automatiquement depuis l'interface (appareil bloqué).
    #[serde(default)]
    pub auto: bool,
    pub created: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_applied: Option<i64>,
}

fn default_true() -> bool {
    true
}

impl FirewallRule {
    /// Étiquette stable utilisée dans les pare-feu du système.
    /// Le préfixe permet de retrouver et supprimer nos règles sans toucher
    /// à celles de l'utilisateur ou d'un autre logiciel.
    pub fn system_name(&self) -> String {
        format!("SECTOOL|{}|{}", self.id, sanitize(&self.name))
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        parts.push(match self.action {
            RuleAction::Block => "Bloquer".to_string(),
            RuleAction::Allow => "Autoriser".to_string(),
        });
        parts.push(match self.direction {
            Direction::In => "entrant".into(),
            Direction::Out => "sortant".into(),
            Direction::Both => "2 sens".into(),
        });
        format!("{} ({})", parts.join(" "), self.protocol.as_str().to_uppercase())
    }
}

/// Retire les caractères gênants pour une ligne de commande.
pub fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .take(60)
        .collect::<String>()
        .trim()
        .to_string()
}

/// État du pare-feu côté machine.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirewallStatus {
    /// Le système est-il pris en charge ?
    pub supported: bool,
    /// Le processus tourne-t-il avec les droits nécessaires ?
    pub elevated: bool,
    /// `windows`, `nftables`, `pf`, `none`.
    pub backend: String,
    /// Nombre de règles actives côté système.
    pub active_rules: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_at: Option<i64>,
    /// Message lisible : pourquoi ça ne marche pas, ou rappel.
    pub message: String,
    /// Vrai si l'on ne peut pas écrire dans le pare-feu du système.
    pub read_only: bool,
}

/// Plan d'application, affiché avant toute modification réelle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPlan {
    pub backend: String,
    pub elevated: bool,
    pub commands: Vec<String>,
    pub warnings: Vec<String>,
    pub rule_count: usize,
}
