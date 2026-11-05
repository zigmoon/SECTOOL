//! Pare-feu logiciel portable de SECTOOL.
//!
//! Le crate garde la liste des règles voulues par l'utilisateur (source de
//! vérité), sait produire un aperçu exact de ce qui sera exécuté, puis
//! l'applique au pare-feu du système avec élévation si nécessaire.

pub mod exec;
pub mod render;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use sectool_core::firewall::{ApplyPlan, FirewallRule, FirewallStatus};
use sectool_core::{now_ms, paths};

pub use exec::ApplyReport;
pub use render::Os;

const STORE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreFile {
    version: u32,
    rules: Vec<FirewallRule>,
    applied_at: Option<i64>,
    applied_names: Vec<String>,
    last_log: String,
}

impl Default for StoreFile {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            rules: Vec::new(),
            applied_at: None,
            applied_names: Vec::new(),
            last_log: String::new(),
        }
    }
}

/// Gestionnaire du pare-feu.
pub struct Firewall {
    os: Os,
    store_path: PathBuf,
    rules: Vec<FirewallRule>,
    applied_at: Option<i64>,
    applied_names: Vec<String>,
    last_log: String,
}

impl Firewall {
    /// Charge les règles enregistrées (ou un jeu vide).
    pub fn load() -> Self {
        let store_path = paths::config_file("firewall.json");
        let store = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|text| serde_json::from_str::<StoreFile>(&text).ok())
            .unwrap_or_default();
        Self {
            os: Os::current(),
            store_path,
            rules: store.rules,
            applied_at: store.applied_at,
            applied_names: store.applied_names,
            last_log: store.last_log,
        }
    }

    pub fn os(&self) -> Os {
        self.os
    }

    pub fn rules(&self) -> &[FirewallRule] {
        &self.rules
    }

    pub fn applied_names(&self) -> &[String] {
        &self.applied_names
    }

    pub fn last_log(&self) -> &str {
        &self.last_log
    }

    pub fn find(&self, id: &str) -> Option<&FirewallRule> {
        self.rules.iter().find(|r| r.id == id)
    }

    /// Ajoute ou remplace une règle.
    pub fn upsert(&mut self, rule: FirewallRule) {
        match self.rules.iter_mut().find(|r| r.id == rule.id) {
            Some(existing) => *existing = rule,
            None => self.rules.push(rule),
        }
        let _ = self.save();
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.id != id);
        let changed = self.rules.len() != before;
        if changed {
            let _ = self.save();
        }
        changed
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        match self.rules.iter_mut().find(|r| r.id == id) {
            Some(rule) => {
                rule.enabled = enabled;
                let _ = self.save();
                true
            }
            None => false,
        }
    }

    fn save(&self) -> std::io::Result<()> {
        let store = StoreFile {
            version: STORE_VERSION,
            rules: self.rules.clone(),
            applied_at: self.applied_at,
            applied_names: self.applied_names.clone(),
            last_log: self.last_log.clone(),
        };
        let text = serde_json::to_string_pretty(&store).unwrap_or_else(|_| "{}".into());
        std::fs::write(&self.store_path, text)
    }

    /// Lignes exactes qui seront exécutées (aperçu avant application).
    pub fn preview(&self) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        match self.os {
            Os::Windows => {
                for name in &self.applied_names {
                    lines.push(format!("netsh advfirewall firewall delete rule name=\"{name}\""));
                }
                lines.extend(render::windows_all(&self.rules));
            }
            Os::Linux => {
                lines.push("nft delete table inet sectool".to_string());
                lines.extend(
                    render::nft_script(&self.rules)
                        .lines()
                        .map(|l| l.to_string()),
                );
            }
            Os::Macos => {
                lines.push("pfctl -a sectool -f /etc/pf.anchors/sectool".to_string());
                lines.extend(render::pf_rules(&self.rules).lines().map(|l| l.to_string()));
            }
        }
        lines
    }

    pub fn plan(&self, elevated: bool) -> ApplyPlan {
        let mut warnings = Vec::new();
        for rule in &self.rules {
            for w in render::validate(rule, self.os) {
                warnings.push(format!("{} : {w}", rule.name));
            }
        }
        if !elevated {
            warnings.push(
                "Une autorisation administrateur sera demandée pour modifier le pare-feu du système."
                    .to_string(),
            );
        }
        ApplyPlan {
            backend: self.os.label().to_string(),
            elevated,
            commands: self.preview(),
            warnings,
            rule_count: render::active_count(&self.rules),
        }
    }

    pub fn status(&self, elevated: bool) -> FirewallStatus {
        let active = render::active_count(&self.rules);
        let message = if !elevated {
            "Mode lecture seule : lancez SECTOOL en administrateur pour appliquer les règles."
                .to_string()
        } else if self.applied_at.is_some() {
            format!("{active} règle(s) active(s) — moteur {} ", self.os.label())
        } else if active == 0 {
            "Aucune règle : le pare-feu du système n'est pas modifié.".to_string()
        } else {
            "Règles prêtes, en attente d'application.".to_string()
        };
        FirewallStatus {
            supported: cfg!(any(
                target_os = "windows",
                target_os = "linux",
                target_os = "macos"
            )),
            elevated,
            backend: self.os.backend().to_string(),
            active_rules: active,
            applied_at: self.applied_at,
            message,
            read_only: !elevated,
        }
    }

    /// Applique réellement les règles au système.
    pub async fn apply(&mut self, elevated: bool) -> ApplyReport {
        let report = match self.os {
            Os::Windows => {
                let mut names: Vec<String> = Vec::new();
                let mut commands: Vec<String> = Vec::new();
                // On retire d'abord ce que SECTOOL avait posé avant.
                if !self.applied_names.is_empty() {
                    commands.extend(
                        self.applied_names
                            .iter()
                            .map(|n| format!("netsh advfirewall firewall delete rule name=\"{n}\"")),
                    );
                }
                for rule in self.rules.iter().filter(|r| r.enabled) {
                    names.extend(render::windows_rule_names(rule));
                    commands.extend(render::windows_commands(rule));
                }
                let report = exec::apply_windows(&commands, elevated).await;
                if report.ok {
                    self.applied_names = names;
                }
                report
            }
            Os::Linux => {
                let script = render::nft_script(&self.rules);
                exec::apply_nft(&script, elevated).await
            }
            Os::Macos => {
                let rules = render::pf_rules(&self.rules);
                exec::apply_pf(&rules, elevated).await
            }
        };

        self.last_log = report.log.clone();
        if report.ok {
            self.applied_at = Some(now_ms());
            for rule in self.rules.iter_mut() {
                if rule.enabled {
                    rule.last_applied = self.applied_at;
                }
            }
        }
        let _ = self.save();
        report
    }

    /// Retire toutes les règles SECTOOL du pare-feu du système.
    pub async fn flush(&mut self, elevated: bool) -> ApplyReport {
        let report = match self.os {
            Os::Windows => {
                if self.applied_names.is_empty() {
                    ApplyReport {
                        ok: true,
                        message: "Aucune règle SECTOOL présente dans le pare-feu Windows.".into(),
                        log: String::new(),
                    }
                } else {
                    let commands: Vec<String> = self
                        .applied_names
                        .iter()
                        .map(|n| format!("netsh advfirewall firewall delete rule name=\"{n}\""))
                        .collect();
                    exec::apply_windows(&commands, elevated).await
                }
            }
            Os::Linux => exec::flush_nft(elevated).await,
            Os::Macos => exec::apply_pf("", elevated).await,
        };
        if report.ok {
            self.applied_names.clear();
            self.applied_at = None;
            for rule in self.rules.iter_mut() {
                rule.last_applied = None;
            }
            let _ = self.save();
        }
        self.last_log = report.log.clone();
        report
    }
}

/// Construit une règle prête à l'emploi.
pub fn new_rule(
    name: &str,
    action: sectool_core::firewall::RuleAction,
    direction: sectool_core::firewall::Direction,
    protocol: sectool_core::firewall::Proto,
) -> FirewallRule {
    let ts = now_ms();
    FirewallRule {
        id: format!("r{ts}"),
        name: name.to_string(),
        action,
        direction,
        protocol,
        remote: None,
        local_ports: None,
        remote_ports: None,
        app: None,
        note: None,
        enabled: true,
        auto: false,
        created: ts,
        last_applied: None,
    }
}
