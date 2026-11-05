//! Orchestration : un balayage complet du réseau local en une passe.

use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sectool_core::alert::{Alert, AlertKind, AlertLevel};
use sectool_core::device::{Device, DeviceKind, PortInfo};
use sectool_core::snapshot::InterfaceInfo;
use sectool_core::{now_ms, oui, Settings};
use tokio::net::UdpSocket;
use tokio::sync::Semaphore;

use crate::ifaces::{self, LocalIface};
use crate::{mdns, neighbors, probe, sys};

/// Résultat d'un balayage.
pub struct ScanOutcome {
    pub devices: Vec<Device>,
    pub alerts: Vec<Alert>,
    pub interfaces: Vec<InterfaceInfo>,
    pub gateway: Option<String>,
    pub local_ip: Option<String>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Ce qu'on a appris sur un hôte pendant la passe.
struct Found {
    ip: Ipv4Addr,
    mac: Option<String>,
    iface: Option<String>,
    rtt: Option<u32>,
    ports: Vec<PortInfo>,
    hostname: Option<String>,
    services: Vec<String>,
    model: Option<String>,
    sources: Vec<String>,
}

/// Scanner avec mémoire : il conserve les appareils connus d'un passage à
/// l'autre pour pouvoir dire « nouveau », « revenu » ou « hors ligne ».
#[derive(Default)]
pub struct Scanner {
    known: HashMap<String, Device>,
    last_full_scan: HashMap<String, i64>,
    outbound_seen: HashSet<String>,
    rounds: u64,
}

impl Scanner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn device_count(&self) -> usize {
        self.known.len()
    }

    /// Numéro du passage en cours (1, 2, 3…).
    pub fn round(&self) -> u64 {
        self.rounds
    }

    /// Effectue un balayage complet.
    pub async fn run(&mut self, settings: &Settings) -> ScanOutcome {
        let started = Instant::now();
        self.rounds += 1;
        let first_round = self.rounds == 1;
        let mut alerts: Vec<Alert> = Vec::new();
        let mut error: Option<String> = None;

        // 1. Interfaces locales et passerelle ---------------------------------
        let local_ifaces: Vec<LocalIface> = ifaces::local_interfaces();
        let macs = ifaces::local_macs().await;
        let gateway = sys::default_gateway().await;
        let local_v4 = ifaces::local_v4_addresses(&local_ifaces);

        let mut interfaces: Vec<InterfaceInfo> = Vec::new();
        for i in &local_ifaces {
            let best = i.best_v4();
            interfaces.push(InterfaceInfo {
                name: i.name.clone(),
                mac: macs.get(&i.name).cloned(),
                ipv4: i.ipv4.iter().map(|(ip, _)| ip.to_string()).collect(),
                ipv6: i.ipv6.iter().map(|ip| ip.to_string()).collect(),
                subnet: best.map(|(ip, p)| ifaces::network_cidr(ip, p)),
                is_up: i.is_up,
                is_default: match (gateway, best) {
                    (Some(gw), Some((ip, prefix))) => {
                        match (gw, ip) {
                            (std::net::IpAddr::V4(gw4), _) => {
                                let mask = if prefix == 0 {
                                    0u32
                                } else {
                                    u32::MAX << (32 - prefix.min(32))
                                };
                                (u32::from(gw4) & mask) == (u32::from(ip) & mask)
                            }
                            _ => false,
                        }
                    }
                    _ => false,
                },
            });
        }

        // 2. Liste des adresses à balayer -------------------------------------
        let mut targets: Vec<Ipv4Addr> = Vec::new();
        let mut budget = settings.max_hosts.max(16);
        if let Some(override_cidr) = settings
            .subnet_override
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            match override_cidr.trim().parse::<ipnet::Ipv4Net>() {
                Ok(net) => {
                    let hosts = probe::hosts_in_subnet(net.network(), net.prefix_len(), budget);
                    budget = budget.saturating_sub(hosts.len());
                    targets.extend(hosts);
                }
                Err(_) => {
                    alerts.push(Alert::new(
                        AlertLevel::Warn,
                        AlertKind::ScanError,
                        "Sous-réseau invalide",
                        format!("« {override_cidr} » n'est pas un CIDR valide (ex. 192.168.1.0/24)."),
                    ));
                }
            }
        }
        if targets.is_empty() {
            for i in &local_ifaces {
                if let Some((ip, prefix)) = i.best_v4() {
                    if ip.is_loopback() || ip.is_link_local() {
                        continue;
                    }
                    let network = ifaces::network_cidr(ip, prefix);
                    if let Ok(net) = network.parse::<ipnet::Ipv4Net>() {
                        let hosts =
                            probe::hosts_in_subnet(net.network(), net.prefix_len(), budget);
                        budget = budget.saturating_sub(hosts.len());
                        targets.extend(hosts);
                    }
                    if budget == 0 {
                        break;
                    }
                }
            }
        }
        targets.sort_unstable();
        targets.dedup();

        // 3. Réveil ARP -------------------------------------------------------
        let mut neighbors = neighbors::read_neighbors().await;
        if settings.enable_arp_sweep && !targets.is_empty() {
            if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await {
                let _ = socket.set_broadcast(true);
                let sweep = async {
                    for ip in &targets {
                        probe::udp_poke(&socket, *ip).await;
                    }
                };
                let _ = tokio::time::timeout(Duration::from_secs(2), sweep).await;
            }
            tokio::time::sleep(Duration::from_millis(220)).await;
            for n in neighbors::read_neighbors().await {
                match neighbors.iter_mut().find(|x| x.ip == n.ip) {
                    Some(existing) => {
                        if existing.mac.is_none() {
                            existing.mac = n.mac;
                        }
                        if !n.state.is_empty() {
                            existing.state = n.state;
                        }
                    }
                    None => neighbors.push(n),
                }
            }
        }

        // 4. Hôtes candidats : la table ARP est la source de vérité -----------
        let mut hosts: HashMap<Ipv4Addr, Found> = HashMap::new();
        for n in &neighbors {
            if n.ip.is_loopback() || n.ip.is_link_local() || !is_usable(n.ip) {
                continue;
            }
            if n.mac.is_none() && !local_v4.contains(&n.ip) {
                continue;
            }
            if n.state.eq_ignore_ascii_case("incomplete") {
                continue;
            }
            hosts.insert(
                n.ip,
                Found {
                    ip: n.ip,
                    mac: n.mac.clone(),
                    iface: n.iface.clone(),
                    rtt: None,
                    ports: Vec::new(),
                    hostname: None,
                    services: Vec::new(),
                    model: None,
                    sources: vec!["arp".into()],
                },
            );
        }
        if let Some(gw) = gateway {
            if let std::net::IpAddr::V4(gw4) = gw {
                if is_usable(gw4) {
                    hosts.entry(gw4).or_insert_with(|| Found {
                        ip: gw4,
                        mac: None,
                        iface: None,
                        rtt: None,
                        ports: Vec::new(),
                        hostname: Some("passerelle".into()),
                        services: Vec::new(),
                        model: None,
                        sources: vec!["gateway".into()],
                    });
                }
            }
        }
        for ip in &local_v4 {
            hosts.entry(*ip).or_insert_with(|| Found {
                ip: *ip,
                mac: None,
                iface: None,
                rtt: None,
                ports: Vec::new(),
                hostname: Some(sys::hostname()),
                services: Vec::new(),
                model: None,
                sources: vec!["self".into()],
            });
        }

        // 5. Sondes TCP (ports ouverts + temps de réponse) --------------------
        let sem = Arc::new(Semaphore::new(64));
        let timeout = Duration::from_millis(settings.probe_timeout_ms.clamp(80, 3000));
        let mut tasks = Vec::new();
        for (ip, found) in &hosts {
            if local_v4.contains(ip) {
                continue;
            }
            let key = Device::key(found.mac.as_deref(), Some(&ip.to_string()));
            let now = now_ms();
            let last = self.last_full_scan.get(&key).copied().unwrap_or(0);
            let full = now - last > 120_000;
            if full {
                self.last_full_scan.insert(key, now);
            }
            let ports: Vec<u16> = if full {
                settings.probe_ports.clone()
            } else {
                vec![80, 443, 445, 62078]
            };
            let sem = sem.clone();
            let ip = *ip;
            tasks.push(tokio::spawn(async move {
                let open = probe::scan_ports(std::net::IpAddr::V4(ip), &ports, timeout, sem).await;
                (ip, open)
            }));
        }
        let probe_deadline = Duration::from_millis(settings.probe_timeout_ms.max(300) * 12 + 1500);
        if let Ok(results) = tokio::time::timeout(probe_deadline, futures_join(tasks)).await {
            for (ip, ports) in results {
                if let Some(found) = hosts.get_mut(&ip) {
                    if let Some(best) = ports.iter().filter_map(|p| p.rtt_ms).min() {
                        found.rtt = Some(best);
                    }
                    if !ports.is_empty() && !found.sources.contains(&"tcp".to_string()) {
                        found.sources.push("tcp".into());
                    }
                    found.ports = ports;
                }
            }
        }

        // 6. mDNS (un passage sur trois pour rester léger) --------------------
        if settings.enable_mdns && (first_round || self.rounds % 3 == 0) {
            let hosts_mdns = mdns::probe(Duration::from_millis(1100)).await;
            for host in hosts_mdns {
                if let Some(found) = hosts.get_mut(&host.ip) {
                    if found.hostname.is_none() {
                        found.hostname = host.name.clone();
                    }
                    if found.model.is_none() {
                        found.model = host.model.clone();
                    }
                    for s in host.services {
                        if !found.services.contains(&s) {
                            found.services.push(s);
                        }
                    }
                    if !found.sources.contains(&"mdns".to_string()) {
                        found.sources.push("mdns".into());
                    }
                }
            }
        }

        // 7. DNS inverse pour les appareils encore anonymes -------------------
        if settings.enable_reverse_dns {
            let mut anonymous: Vec<Ipv4Addr> = hosts
                .values()
                .filter(|h| h.hostname.is_none())
                .map(|h| h.ip)
                .collect();
            anonymous.sort_unstable();
            for ip in anonymous.into_iter().take(24) {
                if let Some(name) = probe::reverse_hostname(ip).await {
                    if let Some(found) = hosts.get_mut(&ip) {
                        found.hostname = Some(name);
                        if !found.sources.contains(&"dns".to_string()) {
                            found.sources.push("dns".into());
                        }
                    }
                }
            }
        }

        // 8. Fusion avec la mémoire du scanner --------------------------------
        let mut seen_keys: HashSet<String> = HashSet::new();
        let mut ip_to_key: HashMap<String, String> = HashMap::new();
        for device in self.known.values() {
            if let Some(ip) = &device.ip {
                ip_to_key.insert(ip.clone(), device.id.clone());
            }
        }

        for found in hosts.values() {
            let ip_text = found.ip.to_string();
            let mut key = Device::key(found.mac.as_deref(), Some(&ip_text));

            // La même IP avec une autre MAC : c'est un autre appareil (bail
            // DHCP réattribué) ou une usurpation — on le signale.
            if let Some(previous_key) = ip_to_key.get(&ip_text) {
                if previous_key != &key && found.mac.is_some() {
                    if let Some(previous) = self.known.get(previous_key) {
                        if previous.mac.is_some() {
                            alerts.push(
                                Alert::new(
                                    AlertLevel::Warn,
                                    AlertKind::MacChange,
                                    "Adresse MAC modifiée",
                                    format!(
                                        "{ip_text} était {} ({}), c'est maintenant {} ({}).",
                                        previous.mac.clone().unwrap_or_default(),
                                        previous.hostname.clone().unwrap_or_else(|| "inconnu".into()),
                                        found.mac.clone().unwrap_or_default(),
                                        found.hostname.clone().unwrap_or_else(|| "inconnu".into())
                                    ),
                                )
                                .with_device(previous.id.clone()),
                            );
                        }
                    }
                    // L'ancienne entrée basée sur la MAC n'a plus d'IP : elle
                    // reste dans l'historique mais repasse hors ligne.
                } else if previous_key != &key {
                    // Même IP mais pas de MAC connue : on garde l'identité.
                    key = previous_key.clone();
                }
            }

            seen_keys.insert(key.clone());
            let now = now_ms();
            let is_gateway = match gateway {
                Some(std::net::IpAddr::V4(gw)) => gw == found.ip,
                _ => false,
            };
            let is_self = local_v4.contains(&found.ip);
            let vendor = found
                .mac
                .as_deref()
                .and_then(oui::lookup)
                .map(|m| m.vendor.to_string())
                .or_else(|| {
                    found
                        .hostname
                        .as_deref()
                        .and_then(oui::vendor_from_hostname)
                        .map(|v| v.to_string())
                });

            match self.known.get_mut(&key) {
                Some(existing) => {
                    let was_offline = !existing.online;
                    existing.ip = Some(ip_text.clone());
                    if found.mac.is_some() {
                        existing.mac = found.mac.clone();
                    }
                    if found.hostname.is_some() {
                        existing.hostname = found.hostname.clone();
                    }
                    if found.model.is_some() {
                        existing.model = found.model.clone();
                    }
                    if vendor.is_some() {
                        existing.vendor = vendor.clone();
                    }
                    if found.rtt.is_some() {
                        existing.rtt_ms = found.rtt;
                    }
                    if !found.ports.is_empty() {
                        existing.ports = found.ports.clone();
                    }
                    for s in &found.services {
                        if !existing.services.contains(s) {
                            existing.services.push(s.clone());
                        }
                    }
                    for s in &found.sources {
                        if !existing.sources.contains(s) {
                            existing.sources.push(s.clone());
                        }
                    }
                    existing.iface = found.iface.clone().or(existing.iface.clone());
                    existing.last_seen = now;
                    existing.online = true;
                    existing.is_gateway = is_gateway;
                    existing.is_self = is_self;
                    existing.kind = existing.infer_kind();
                    if was_offline {
                        alerts.push(
                            Alert::new(
                                AlertLevel::Info,
                                AlertKind::DeviceBack,
                                "Appareil de retour",
                                format!(
                                    "{} est de nouveau en ligne ({ip_text}).",
                                    existing
                                        .hostname
                                        .clone()
                                        .or(existing.vendor.clone())
                                        .unwrap_or_else(|| ip_text.clone())
                                ),
                            )
                            .with_device(existing.id.clone()),
                        );
                    }
                }
                None => {
                    let mut device = Device {
                        id: key.clone(),
                        mac: found.mac.clone(),
                        ip: Some(ip_text.clone()),
                        ipv6: None,
                        hostname: found.hostname.clone(),
                        vendor: vendor.clone(),
                        model: found.model.clone(),
                        kind: DeviceKind::Unknown,
                        online: true,
                        is_gateway,
                        is_self,
                        first_seen: now,
                        last_seen: now,
                        rtt_ms: found.rtt,
                        ports: found.ports.clone(),
                        services: found.services.clone(),
                        sources: found.sources.clone(),
                        iface: found.iface.clone(),
                        blocked: false,
                        note: None,
                    };
                    device.kind = device.infer_kind();
                    if !is_self {
                        let label = device
                            .hostname
                            .clone()
                            .or(device.vendor.clone())
                            .unwrap_or_else(|| ip_text.clone());
                        alerts.push(
                            Alert::new(
                                AlertLevel::Info,
                                AlertKind::NewDevice,
                                "Nouvel appareil détecté",
                                format!(
                                    "{label} — {ip_text}{}",
                                    device
                                        .mac
                                        .as_deref()
                                        .map(|m| format!(" · {m}"))
                                        .unwrap_or_default()
                                ),
                            )
                            .with_device(device.id.clone()),
                        );
                    }
                    self.known.insert(key, device);
                }
            }
        }

        // 9. Appareils absents → hors ligne -----------------------------------
        for (key, device) in self.known.iter_mut() {
            if !seen_keys.contains(key) && device.online {
                device.online = false;
                let label = device
                    .hostname
                    .clone()
                    .or(device.vendor.clone())
                    .unwrap_or_else(|| device.ip.clone().unwrap_or_default());
                if !device.is_self {
                    alerts.push(
                        Alert::new(
                            AlertLevel::Warn,
                            AlertKind::DeviceOffline,
                            "Appareil hors ligne",
                            format!("{label} ne répond plus."),
                        )
                        .with_device(device.id.clone()),
                    );
                }
            }
        }

        // 10. Nettoyage des appareils fantômes --------------------------------
        let ttl = 7 * 24 * 3600 * 1000i64;
        let stale: Vec<String> = self
            .known
            .iter()
            .filter(|(_, d)| !d.online && now_ms() - d.last_seen > ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for key in stale {
            self.known.remove(&key);
        }

        let mut devices: Vec<Device> = self.known.values().cloned().collect();
        devices.sort_by(|a, b| {
            b.is_gateway
                .cmp(&a.is_gateway)
                .then(b.online.cmp(&a.online))
                .then(ip_sort_key(&a.ip).cmp(&ip_sort_key(&b.ip)))
        });

        if devices.is_empty() {
            error = Some(
                "Aucun appareil détecté. Vérifiez que la machine est bien connectée au réseau local."
                    .to_string(),
            );
        }

        ScanOutcome {
            devices,
            alerts,
            interfaces,
            gateway: gateway.map(|g| g.to_string()),
            local_ip: local_v4.first().map(|ip| ip.to_string()),
            duration_ms: started.elapsed().as_millis() as u64,
            error,
        }
    }

    /// Note les connexions sortantes déjà vues, pour ne pas répéter les alertes.
    pub fn mark_outbound(&mut self, key: String) -> bool {
        self.outbound_seen.insert(key)
    }
}

fn is_usable(ip: Ipv4Addr) -> bool {
    !ip.is_unspecified() && !ip.is_broadcast() && !ip.is_multicast() && !ip.is_loopback()
}

fn ip_sort_key(ip: &Option<String>) -> u32 {
    ip.as_deref()
        .and_then(|s| s.parse::<Ipv4Addr>().ok())
        .map(u32::from)
        .unwrap_or(u32::MAX)
}

/// Attend toutes les tâches sans dépendance supplémentaire.
async fn futures_join<T>(tasks: Vec<tokio::task::JoinHandle<T>>) -> Vec<T> {
    let mut out = Vec::new();
    for task in tasks {
        if let Ok(value) = task.await {
            out.push(value);
        }
    }
    out
}
