//! Accès au système : commandes, passerelle, nom de machine, élévation.

use std::net::IpAddr;
use std::time::Duration;

use tokio::process::Command;

/// Nom de la machine.
pub fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "poste-local".to_string())
}

/// Identifiant de plateforme utilisé par l'interface.
pub fn platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// Exécute une commande et renvoie sa sortie standard.
///
/// Ne bloque pas le runtime : le processus est lancé par tokio et une limite
/// de temps évite qu'une commande figée bloque la boucle de surveillance.
pub async fn run(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.stdin(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.output();
    match tokio::time::timeout(timeout, child).await {
        Ok(Ok(out)) if out.status.success() => {
            Some(String::from_utf8_lossy(&out.stdout).to_string())
        }
        _ => None,
    }
}

/// Exécute une commande et renvoie (succès, stdout + stderr).
pub async fn run_full(program: &str, args: &[&str], timeout: Duration) -> (bool, String) {
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    match tokio::time::timeout(timeout, cmd.output()).await {
        Ok(Ok(out)) => {
            let mut text = String::from_utf8_lossy(&out.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            (out.status.success(), text)
        }
        Ok(Err(e)) => (false, e.to_string()),
        Err(_) => (false, "délai dépassé".to_string()),
    }
}

/// Passerelle par défaut détectée auprès du système.
pub async fn default_gateway() -> Option<IpAddr> {
    let timeout = Duration::from_secs(3);
    if cfg!(target_os = "windows") {
        let out = run("route", &["print", "-4"], timeout).await?;
        for line in out.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 3 && cols[0] == "0.0.0.0" && cols[1] == "0.0.0.0" {
                if let Ok(ip) = cols[2].parse::<IpAddr>() {
                    if !ip.is_unspecified() {
                        return Some(ip);
                    }
                }
            }
        }
        None
    } else if cfg!(target_os = "macos") {
        let out = run("netstat", &["-rn", "-f", "inet"], timeout).await?;
        for line in out.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 2 && cols[0] == "default" {
                if let Ok(ip) = cols[1].parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
        None
    } else {
        let out = run("ip", &["route", "show", "default"], timeout).await?;
        for line in out.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if let Some(pos) = cols.iter().position(|c| *c == "via") {
                if let Some(ip) = cols.get(pos + 1).and_then(|s| s.parse::<IpAddr>().ok()) {
                    return Some(ip);
                }
            }
        }
        None
    }
}

/// Le processus tourne-t-il avec les droits administrateur ?
pub async fn is_elevated() -> bool {
    if cfg!(target_os = "windows") {
        let script = "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)";
        match run(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", script],
            Duration::from_secs(6),
        )
        .await
        {
            Some(out) => out.trim().eq_ignore_ascii_case("true"),
            None => false,
        }
    } else {
        match run("id", &["-u"], Duration::from_secs(2)).await {
            Some(out) => out.trim() == "0",
            None => false,
        }
    }
}
