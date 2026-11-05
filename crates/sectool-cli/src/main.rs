//! SECTOOL en ligne de commande.
//!
//! Sert à vérifier le moteur réseau sans lancer l'interface graphique, et
//! permet d'appliquer les règles de pare-feu depuis un terminal ou un script.

use std::time::Duration;

use sectool_core::device::Device;
use sectool_core::{now_ms, Settings};
use sectool_firewall::Firewall;
use sectool_net::scan::Scanner;
use sectool_net::sockets;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(|s| s.as_str()).unwrap_or("help").to_string();
    let rest = if args.len() > 1 { args[1..].to_vec() } else { Vec::new() };

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Impossible de démarrer le moteur asynchrone : {e}");
            std::process::exit(1);
        }
    };

    let code = runtime.block_on(async move {
        match command.as_str() {
            "scan" => cmd_scan(&rest).await,
            "watch" => cmd_watch(&rest).await,
            "connections" | "conns" => cmd_connections().await,
            "rules" => cmd_rules(),
            "preview" => cmd_preview(),
            "apply" => cmd_apply(&rest).await,
            "flush" => cmd_flush(&rest).await,
            "help" | "--help" | "-h" => {
                print_help();
                0
            }
            "version" | "--version" | "-V" => {
                println!("sectool {}", sectool_core::VERSION);
                0
            }
            other => {
                eprintln!("Commande inconnue : {other}\n");
                print_help();
                2
            }
        }
    });
    std::process::exit(code);
}

fn print_help() {
    println!(
        r#"SECTOOL {version} — voir et contrôler son réseau local

UTILISATION
  sectool <commande> [options]

COMMANDES
  scan [--json]        Un balayage du réseau, puis le tableau des appareils
  watch [--interval N] Balayage en boucle (par défaut toutes les 5 s)
  connections [--json] Connexions TCP/UDP de la machine
  rules                Liste les règles de pare-feu enregistrées
  preview              Affiche les commandes exactes du pare-feu (sans agir)
  apply [--yes]        Applique les règles (élévation demandée si besoin)
  flush [--yes]        Retire toutes les règles SECTOOL du système
  version              Version
  help                 Cette aide

EXEMPLES
  sectool scan
  sectool watch --interval 3
  sectool scan --json > reseau.json
  sectool apply --yes
"#,
        version = sectool_core::VERSION
    );
}

async fn cmd_scan(args: &[String]) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    let mut scanner = Scanner::new();
    let settings = Settings::default();
    let started = now_ms();
    let outcome = scanner.run(&settings).await;
    let devices = outcome.devices;

    if json {
        let payload = serde_json::json!({
            "ts": now_ms(),
            "durationMs": outcome.duration_ms,
            "gateway": outcome.gateway,
            "localIp": outcome.local_ip,
            "interfaces": outcome.interfaces,
            "devices": devices,
            "alerts": outcome.alerts,
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_default());
    } else {
        println!(
            "Balayage terminé en {} ms — {} appareil(s)\n",
            outcome.duration_ms,
            devices.len()
        );
        print_devices(&devices);
        if !outcome.alerts.is_empty() {
            println!("\nÉvénements :");
            for alert in &outcome.alerts {
                println!("  [{}] {} — {}", alert.level_str(), alert.title, alert.detail);
            }
        }
        let _ = started;
    }
    0
}

async fn cmd_watch(args: &[String]) -> i32 {
    let interval: u64 = args
        .iter()
        .position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let mut scanner = Scanner::new();
    let mut settings = Settings::default();
    settings.scan_interval_secs = interval;
    loop {
        let outcome = scanner.run(&settings).await;
        println!(
            "\n=== {} — {} appareil(s) en {} ms ===",
            horodatage(),
            outcome.devices.len(),
            outcome.duration_ms
        );
        print_devices(&outcome.devices);
        for alert in &outcome.alerts {
            println!("  [{}] {} — {}", alert.level_str(), alert.title, alert.detail);
        }
        tokio::time::sleep(Duration::from_secs(interval.max(1))).await;
    }
}

async fn cmd_connections() -> i32 {
    let snap = sockets::snapshot().await;
    println!(
        "{:<5} {:<28} {:<28} {:<12} {:<8} {}",
        "PROTO", "LOCAL", "DISTANT", "ÉTAT", "PID", "PROCESSUS"
    );
    let mut list: Vec<_> = snap
        .connections
        .iter()
        .filter(|c| c.state.is_open())
        .collect();
    list.sort_by(|a, b| a.process.cmp(&b.process));
    for c in list {
        println!(
            "{:<5} {:<28} {:<28} {:<12} {:<8} {}",
            c.proto.to_uppercase(),
            c.local_label(),
            c.remote_label(),
            format!("{:?}", c.state).to_lowercase(),
            c.pid.map(|p| p.to_string()).unwrap_or_default(),
            c.process.clone().unwrap_or_default()
        );
    }
    0
}

fn cmd_rules() -> i32 {
    let firewall = Firewall::load();
    println!("Moteur : {}", firewall.os().label());
    if firewall.rules().is_empty() {
        println!("Aucune règle enregistrée.");
        return 0;
    }
    for rule in firewall.rules() {
        println!(
            "{} [{}] {} {} {} {}",
            if rule.enabled { "●" } else { "○" },
            rule.id,
            rule.name,
            rule.action.as_str(),
            rule.direction.as_str(),
            rule.protocol.as_str()
        );
        if let Some(remote) = &rule.remote {
            println!("    distant : {remote}");
        }
        if let Some(ports) = &rule.local_ports {
            println!("    ports locaux : {ports}");
        }
    }
    0
}

fn cmd_preview() -> i32 {
    let firewall = Firewall::load();
    let plan = firewall.plan(false);
    println!("Moteur   : {}", plan.backend);
    println!("Règles   : {}", plan.rule_count);
    for warning in &plan.warnings {
        println!("⚠ {warning}");
    }
    println!("--- commandes ---");
    for line in &plan.commands {
        println!("{line}");
    }
    0
}

async fn cmd_apply(args: &[String]) -> i32 {
    let confirmed = args.iter().any(|a| a == "--yes" || a == "-y");
    let mut firewall = Firewall::load();
    if !confirmed {
        println!("Aperçu (relancez avec --yes pour appliquer) :");
        for line in firewall.preview() {
            println!("{line}");
        }
        return 0;
    }
    let elevated = sectool_net::sys::is_elevated().await;
    let report = firewall.apply(elevated).await;
    println!("{}", report.message);
    if !report.ok {
        println!("--- journal ---\n{}", report.log);
        return 1;
    }
    0
}

async fn cmd_flush(args: &[String]) -> i32 {
    let confirmed = args.iter().any(|a| a == "--yes" || a == "-y");
    let mut firewall = Firewall::load();
    if !confirmed {
        println!("Retire toutes les règles SECTOOL du pare-feu système.");
        println!("Relancez avec --yes pour confirmer.");
        return 0;
    }
    let elevated = sectool_net::sys::is_elevated().await;
    let report = firewall.flush(elevated).await;
    println!("{}", report.message);
    if !report.ok {
        println!("--- journal ---\n{}", report.log);
        return 1;
    }
    0
}

fn print_devices(devices: &[Device]) {
    println!(
        "{:<16} {:<19} {:<22} {:<10} {:<18} {}",
        "IP", "MAC", "NOM", "TYPE", "CONSTRUCTEUR", "SERVICES"
    );
    for d in devices {
        let services = d
            .ports
            .iter()
            .map(|p| p.port.to_string())
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{:<16} {:<19} {:<22} {:<10} {:<18} {}",
            d.ip.clone().unwrap_or_else(|| "-".into()),
            d.mac.clone().unwrap_or_else(|| "-".into()),
            truncate(d.hostname.clone().or(d.model.clone()).unwrap_or_else(|| "-".into()), 22),
            d.kind.as_str(),
            truncate(d.vendor.clone().unwrap_or_else(|| "-".into()), 18),
            if d.is_gateway {
                format!("passerelle {services}")
            } else {
                services
            }
        );
    }
}

fn truncate(mut text: String, max: usize) -> String {
    if text.chars().count() > max {
        text = text.chars().take(max.saturating_sub(1)).collect::<String>() + "…";
    }
    text
}

/// Horodatage lisible sans dépendance externe.
fn horodatage() -> String {
    let secs = now_ms() / 1000;
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}
