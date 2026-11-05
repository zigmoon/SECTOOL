//! Traduction des règles SECTOOL vers le pare-feu du système.
//!
//! Trois moteurs sont pris en charge :
//! * Windows — `netsh advfirewall` (règles nommées `SECTOOL|…`).
//! * Linux  — `nftables`, dans une table dédiée `inet sectool`.
//! * macOS  — `pf`, dans une ancre dédiée `sectool`.
//!
//! Dans les trois cas, SECTOOL ne touche **jamais** aux règles qui ne lui
//! appartiennent pas.

use sectool_core::firewall::{Direction, FirewallRule, Proto, RuleAction};

/// Système d'exploitation ciblé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    Macos,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Macos
        } else {
            Os::Linux
        }
    }

    pub fn backend(self) -> &'static str {
        match self {
            Os::Windows => "windows",
            Os::Linux => "nftables",
            Os::Macos => "pf",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Os::Windows => "Pare-feu Windows (netsh)",
            Os::Linux => "nftables",
            Os::Macos => "pf (packet filter)",
        }
    }
}

/// Découpe une liste de ports `445,139,8000-8100` en ports individuels bornés.
pub fn expand_ports(spec: &str) -> Vec<u16> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let start = a.trim().parse::<u16>().unwrap_or(0);
            let end = b.trim().parse::<u16>().unwrap_or(0);
            if start == 0 || end < start {
                continue;
            }
            for p in start..=end {
                out.push(p);
                if out.len() >= 256 {
                    return out;
                }
            }
        } else if let Ok(p) = part.parse::<u16>() {
            out.push(p);
        }
        if out.len() >= 256 {
            break;
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Vérifie qu'une règle est appliquable ; renvoie les avertissements.
pub fn validate(rule: &FirewallRule, os: Os) -> Vec<String> {
    let mut warnings = Vec::new();
    if rule.name.trim().is_empty() {
        warnings.push("Le nom de la règle est vide.".to_string());
    }
    if let Some(remote) = rule
        .remote
        .as_deref()
        .map(|r| r.trim())
        .filter(|r| !r.is_empty())
    {
        if remote.parse::<ipnet::IpNet>().is_err() && remote.parse::<std::net::IpAddr>().is_err() {
            warnings.push(format!("Adresse ou CIDR invalide : {remote}"));
        }
    }
    for (label, spec) in [
        ("ports locaux", rule.local_ports.as_deref()),
        ("ports distants", rule.remote_ports.as_deref()),
    ] {
        if let Some(spec) = spec.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            if expand_ports(spec).is_empty() {
                warnings.push(format!("Liste de {label} invalide : {spec}"));
            }
        }
    }
    if rule.app.is_some() && os != Os::Windows {
        warnings
            .push("Le filtrage par application n'est pris en charge que sous Windows.".to_string());
    }
    if rule.remote.is_none() && rule.local_ports.is_none() && rule.remote_ports.is_none() {
        warnings.push(
            "Règle très large : sans adresse ni port, elle vise tout le trafic.".to_string(),
        );
    }
    warnings
}

/// Nom système, échappé pour une ligne de commande.
fn quoted_system_name(rule: &FirewallRule, index: usize) -> String {
    let base = rule.system_name().replace('"', "'");
    if index == 0 {
        base
    } else {
        format!("{base}#{index}")
    }
}

fn windows_dirs(rule: &FirewallRule) -> Vec<&'static str> {
    match rule.direction {
        Direction::In => vec!["in"],
        Direction::Out => vec!["out"],
        Direction::Both => vec!["in", "out"],
    }
}

fn windows_proto(rule: &FirewallRule) -> &'static str {
    match rule.protocol {
        Proto::Tcp => "TCP",
        Proto::Udp => "UDP",
        Proto::Any => "any",
    }
}

/// Commandes `netsh` pour une règle (une par sens de circulation).
pub fn windows_commands(rule: &FirewallRule) -> Vec<String> {
    let action = match rule.action {
        RuleAction::Allow => "allow",
        RuleAction::Block => "block",
    };
    let mut out = Vec::new();
    for (index, dir) in windows_dirs(rule).into_iter().enumerate() {
        let mut cmd = format!(
            "netsh advfirewall firewall add rule name=\"{}\" dir={dir} action={action} protocol={} profile=any enable=yes",
            quoted_system_name(rule, index),
            windows_proto(rule)
        );        if let Some(ports) = rule.local_ports.as_deref().map(|s| s.replace(' ', "")) {
            if !ports.is_empty() {
                cmd.push_str(&format!(" localport={ports}"));
            }
        }
        if let Some(ports) = rule.remote_ports.as_deref().map(|s| s.replace(' ', "")) {
            if !ports.is_empty() {
                cmd.push_str(&format!(" remoteport={ports}"));
            }
        }
        if let Some(remote) = rule.remote.as_deref().map(|s| s.trim()) {
            if !remote.is_empty() {
                cmd.push_str(&format!(" remoteip={remote}"));
            }
        }
        if let Some(app) = rule.app.as_deref().map(|s| s.trim()) {
            if !app.is_empty() {
                cmd.push_str(&format!(" program=\"{app}\""));
            }
        }
        out.push(cmd);
    }
    out
}

/// Commandes de suppression d'une règle.
pub fn windows_delete_commands(rule: &FirewallRule) -> Vec<String> {
    windows_rule_names(rule)
        .into_iter()
        .map(|name| format!("netsh advfirewall firewall delete rule name=\"{name}\""))
        .collect()
}

/// Noms exacts des règles Windows créées pour une règle SECTOOL.
pub fn windows_rule_names(rule: &FirewallRule) -> Vec<String> {
    (0..windows_dirs(rule).len())
        .map(|index| quoted_system_name(rule, index))
        .collect()
}

/// Toutes les commandes Windows pour un jeu de règles.
pub fn windows_all(rules: &[FirewallRule]) -> Vec<String> {
    let mut out = Vec::new();
    for rule in rules.iter().filter(|r| r.enabled) {
        out.extend(windows_commands(rule));
    }
    out
}

/// Script nftables complet (remplace la table `inet sectool`).
pub fn nft_script(rules: &[FirewallRule]) -> String {
    let mut input = String::new();
    let mut output = String::new();

    for rule in rules.iter().filter(|r| r.enabled) {
        let verdict = match rule.action {
            RuleAction::Allow => "accept",
            RuleAction::Block => "drop",
        };
        let proto = match rule.protocol {
            Proto::Tcp => "tcp",
            Proto::Udp => "udp",
            Proto::Any => "th",
        };
        let remote = rule
            .remote
            .as_deref()
            .map(|r| r.trim())
            .filter(|r| !r.is_empty());
        let ports = rule
            .local_ports
            .as_deref()
            .map(expand_ports)
            .unwrap_or_default();
        let dports = rule
            .remote_ports
            .as_deref()
            .map(expand_ports)
            .unwrap_or_default();

        let mut line = String::new();
        if let Some(remote) = remote {
            line.push_str(&format!("ip saddr {remote} "));
        }
        let ports = if ports.is_empty() { dports } else { ports };
        if ports.is_empty() {
            line.push_str(&format!("{proto} counter {verdict}"));
        } else {
            let list = ports
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            line.push_str(&format!("{proto} dport {{ {list} }} counter {verdict}"));
        }
        line.push_str(&format!(" comment \"{}\"", safe_label(&rule.name)));

        let padded = format!("        {line}\n");
        match rule.direction {
            Direction::In => input.push_str(&padded),
            Direction::Out => output.push_str(&padded),
            Direction::Both => {
                input.push_str(&padded);
                output.push_str(&padded);
            }
        }
    }

    format!(
        "table inet sectool {{\n    chain input {{\n        type filter hook input priority 0; policy accept;\n{input}    }}\n    chain output {{\n        type filter hook output priority 0; policy accept;\n{output}    }}\n}}\n"
    )
}

fn safe_label(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .take(48)
        .collect()
}

/// Règles pf pour l'ancre `sectool`.
pub fn pf_rules(rules: &[FirewallRule]) -> String {
    let mut out = String::new();
    for rule in rules.iter().filter(|r| r.enabled) {
        let action = match rule.action {
            RuleAction::Allow => "pass",
            RuleAction::Block => "block drop",
        };
        let mut parts: Vec<String> = vec![action.to_string()];
        match rule.direction {
            Direction::In => parts.push("in".into()),
            Direction::Out => parts.push("out".into()),
            Direction::Both => {}
        }
        match rule.protocol {
            Proto::Tcp => parts.push("proto tcp".into()),
            Proto::Udp => parts.push("proto udp".into()),
            Proto::Any => {}
        }
        if let Some(remote) = rule
            .remote
            .as_deref()
            .map(|r| r.trim())
            .filter(|r| !r.is_empty())
        {
            parts.push(format!("from {remote} to any"));
        }
        let ports = rule
            .local_ports
            .as_deref()
            .map(expand_ports)
            .unwrap_or_default();
        if !ports.is_empty() {
            let list = ports
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("port {{ {list} }}"));
        }
        parts.push(format!("label \"{}\"", safe_label(&rule.name)));
        out.push_str(&parts.join(" "));
        out.push('\n');
    }
    out
}

/// Nombre de règles réellement appliquées par le système.
pub fn active_count(rules: &[FirewallRule]) -> usize {
    rules.iter().filter(|r| r.enabled).count()
}
