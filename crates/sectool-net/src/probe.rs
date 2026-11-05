//! Sondes réseau sans privilège particulier.
//!
//! On n'utilise ni ICMP brut ni capture de paquets : tout passe par des
//! sockets UDP/TCP normaux, ce qui fonctionne sans administrateur et sans
//! pilote noyau — sur Windows, macOS et Linux.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::Semaphore;

use sectool_core::device::{service_for_port, PortInfo};

/// Énumère les hôtes utilisables d'un sous-réseau, en excluant le réseau et le
/// broadcast. Le nombre d'hôtes est plafonné.
pub fn hosts_in_subnet(network: Ipv4Addr, prefix: u8, max_hosts: usize) -> Vec<Ipv4Addr> {
    let prefix = prefix.min(32);
    let host_bits = 32 - prefix as u32;
    if host_bits == 0 {
        return vec![network];
    }
    let total = 1u64 << host_bits;
    if total <= 2 {
        return Vec::new();
    }
    let base = u32::from(network);
    let first = base.saturating_add(1);
    let last = base.saturating_add(total as u32).saturating_sub(2);
    let mut out = Vec::new();
    let mut ip = first;
    while ip <= last {
        out.push(Ipv4Addr::from(ip));
        if out.len() >= max_hosts {
            break;
        }
        ip += 1;
    }
    out
}

/// Envoie un datagramme UDP anodin pour forcer le système à résoudre l'adresse
/// matérielle : c'est ce qui remplit la table ARP sans privilège.
pub async fn udp_poke(socket: &UdpSocket, ip: Ipv4Addr) {
    let payload = [0u8; 1];
    for port in [9u16, 5353, 137] {
        let target = SocketAddr::new(IpAddr::V4(ip), port);
        // `try_send_to` sur UDP ne bloque pas : l'erreur éventuelle (hôte
        // injoignable) est sans importance ici.
        if socket.try_send_to(&payload, target).is_err() {
            // Tampon plein : on laisse le système respirer.
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
    tokio::task::yield_now().await;
}

/// Teste un port TCP et renvoie le temps de réponse en millisecondes.
pub async fn tcp_probe(ip: IpAddr, port: u16, timeout: Duration) -> Option<u32> {
    let started = Instant::now();
    let addr = SocketAddr::new(ip, port);
    match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
        Ok(Ok(stream)) => {
            drop(stream);
            Some(started.elapsed().as_millis().min(u32::MAX as u128) as u32)
        }
        _ => None,
    }
}

/// Teste une liste de ports en parallèle et renvoie ceux qui répondent.
///
/// Le sémaphore est partagé entre tous les hôtes pour ne pas saturer la
/// connexion de l'utilisateur.
pub async fn scan_ports(
    ip: IpAddr,
    ports: &[u16],
    timeout: Duration,
    sem: Arc<Semaphore>,
) -> Vec<PortInfo> {
    let mut tasks = Vec::new();
    for &port in ports {
        let sem = sem.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.ok()?;
            let rtt = tcp_probe(ip, port, timeout).await?;
            Some(PortInfo {
                port,
                service: service_for_port(port).map(|s| s.to_string()),
                rtt_ms: Some(rtt),
            })
        }));
    }
    let mut open = Vec::new();
    for task in tasks {
        if let Ok(Some(info)) = task.await {
            open.push(info);
        }
    }
    open.sort_by_key(|p| p.port);
    open
}

/// Résolution DNS inverse du nom d'hôte, tolérante à l'absence d'outil.
pub async fn reverse_hostname(ip: Ipv4Addr) -> Option<String> {
    // Linux/macOS : `getent hosts` est rapide et non localisé.
    if !cfg!(target_os = "windows") {
        if let Some(text) = crate::sys::run(
            "getent",
            &["hosts", &ip.to_string()],
            Duration::from_millis(800),
        )
        .await
        {
            if let Some(name) = text.split_whitespace().last() {
                let name = name.trim_end_matches('.');
                if !name.is_empty() && name != ip.to_string() {
                    return Some(name.to_string());
                }
            }
        }
    }
    // Windows et secours : `nslookup <ip>` puis lecture de la ligne « Name: ».
    let text =
        crate::sys::run("nslookup", &[&ip.to_string()], Duration::from_millis(1200)).await?;
    for line in text.lines() {
        if let Some((label, value)) = line.split_once(':') {
            let label = label.trim().to_ascii_lowercase();
            if !(label.starts_with("name") || label.starts_with("nom")) {
                continue;
            }
            let value = value.trim().trim_end_matches('.');
            if !value.is_empty()
                && value != ip.to_string()
                && value.parse::<IpAddr>().is_err()
                && !value.contains(' ')
            {
                return Some(value.to_string());
            }
        }
    }
    None
}
