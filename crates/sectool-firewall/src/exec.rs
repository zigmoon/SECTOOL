//! Exécution des scripts d'application, avec élévation si nécessaire.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sectool_core::paths::work_dir;

use crate::render::Os;

/// Résultat d'une application de règles.
#[derive(Debug, Clone)]
pub struct ApplyReport {
    pub ok: bool,
    pub message: String,
    pub log: String,
}

impl ApplyReport {
    fn failure(message: impl Into<String>, log: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            log: log.into(),
        }
    }
}

/// Écrit un fichier de travail et renvoie son chemin.
fn write_work_file(name: &str, content: &str) -> std::io::Result<PathBuf> {
    let path = work_dir().join(name);
    std::fs::write(&path, content)?;
    Ok(path)
}

fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(path, perms);
    }
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

/// Lance un programme sans fenêtre et attend la fin.
async fn run_program(program: &str, args: &[String], timeout: Duration) -> (bool, String) {
    let mut cmd = tokio::process::Command::new(program);
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

/// Applique un script Windows (`.cmd`).
pub async fn apply_windows(commands: &[String], elevated: bool) -> ApplyReport {
    if commands.is_empty() {
        return ApplyReport {
            ok: true,
            message: "Aucune règle active : rien à appliquer.".into(),
            log: String::new(),
        };
    }
    let log_path = work_dir().join("netsh.log");
    let result_path = work_dir().join("netsh.result");
    let _ = std::fs::remove_file(&log_path);
    let _ = std::fs::remove_file(&result_path);

    let mut script = String::from("@echo off\r\nchcp 65001 >nul\r\n");
    for cmd in commands {
        script.push_str(&format!("({cmd}) >> \"{}\" 2>&1\r\n", log_path.display()));
    }
    script.push_str(&format!("echo done> \"{}\"\r\n", result_path.display()));

    let script_path = match write_work_file("apply.cmd", &script) {
        Ok(p) => p,
        Err(e) => return ApplyReport::failure("Impossible d'écrire le script", e.to_string()),
    };

    let (ok, out) = if elevated {
        run_program(
            "cmd.exe",
            &[
                "/c".to_string(),
                script_path.display().to_string(),
            ],
            Duration::from_secs(60),
        )
        .await
    } else {
        // Demande l'élévation via UAC.
        let ps = format!(
            "Start-Process -FilePath cmd.exe -ArgumentList @('/c','\"{}\"') -Verb RunAs -Wait -WindowStyle Hidden",
            script_path.display()
        );
        run_program(
            "powershell",
            &[
                "-NoProfile".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-Command".into(),
                ps,
            ],
            Duration::from_secs(120),
        )
        .await
    };

    let log = read_text(&log_path);
    let finished = result_path.exists();
    if finished {
        ApplyReport {
            ok: true,
            message: format!("{} règle(s) appliquée(s) au pare-feu Windows.", commands.len()),
            log: format!("{out}\n{log}"),
        }
    } else {
        ApplyReport::failure(
            "Application refusée ou interrompue (autorisation administrateur nécessaire).",
            format!("{out}\n{log}"),
        )
    }
}

/// Supprime les règles SECTOOL déjà présentes sous Windows.
pub async fn delete_windows(names: &[String], elevated: bool) -> ApplyReport {
    let commands: Vec<String> = names
        .iter()
        .map(|n| format!("netsh advfirewall firewall delete rule name=\"{n}\""))
        .collect();
    apply_windows(&commands, elevated).await
}

/// Applique un script nftables sous Linux.
pub async fn apply_nft(script: &str, elevated: bool) -> ApplyReport {
    let rules_path = match write_work_file("sectool.nft", script) {
        Ok(p) => p,
        Err(e) => return ApplyReport::failure("Impossible d'écrire le script nft", e.to_string()),
    };
    let log_path = work_dir().join("nft.log");
    let result_path = work_dir().join("nft.result");
    let _ = std::fs::remove_file(&log_path);
    let _ = std::fs::remove_file(&result_path);

    let shell = format!(
        "#!/bin/sh\n: > '{log}'\nnft delete table inet sectool >>'{log}' 2>&1\nnft -f '{rules}' >>'{log}' 2>&1\necho $? > '{result}'\n",
        log = log_path.display(),
        rules = rules_path.display(),
        result = result_path.display()
    );
    let script_path = match write_work_file("apply.sh", &shell) {
        Ok(p) => p,
        Err(e) => return ApplyReport::failure("Impossible d'écrire le script", e.to_string()),
    };
    make_executable(&script_path);

    let (out, _ok) = if elevated {
        let (ok, out) = run_program(
            "sh",
            &[script_path.display().to_string()],
            Duration::from_secs(30),
        )
        .await;
        (out, ok)
    } else {
        let (mut ok, mut out) = run_program(
            "pkexec",
            &["sh".into(), script_path.display().to_string()],
            Duration::from_secs(90),
        )
        .await;
        if !ok {
            let (ok2, out2) = run_program(
                "sudo",
                &["-n".into(), "sh".into(), script_path.display().to_string()],
                Duration::from_secs(30),
            )
            .await;
            ok = ok2;
            out.push_str(&out2);
        }
        (out, ok)
    };

    let log = read_text(&log_path);
    if result_path.exists() {
        ApplyReport {
            ok: true,
            message: "Règles chargées dans la table nftables « sectool ».".into(),
            log: format!("{out}\n{log}"),
        }
    } else {
        ApplyReport::failure(
            "Application impossible (droits root nécessaires : pkexec ou sudo).",
            format!("{out}\n{log}"),
        )
    }
}

/// Supprime complètement la table nftables de SECTOOL.
pub async fn flush_nft(elevated: bool) -> ApplyReport {
    let log_path = work_dir().join("nft.log");
    let result_path = work_dir().join("nft.result");
    let _ = std::fs::remove_file(&log_path);
    let _ = std::fs::remove_file(&result_path);
    let shell = format!(
        "#!/bin/sh\n: > '{log}'\nnft delete table inet sectool >>'{log}' 2>&1\necho $? > '{result}'\n",
        log = log_path.display(),
        result = result_path.display()
    );
    let script_path = match write_work_file("flush.sh", &shell) {
        Ok(p) => p,
        Err(e) => return ApplyReport::failure("Impossible d'écrire le script", e.to_string()),
    };
    make_executable(&script_path);

    let (out, _) = if elevated {
        let (ok, out) = run_program("sh", &[script_path.display().to_string()], Duration::from_secs(20)).await;
        (out, ok)
    } else {
        let (ok, out) = run_program(
            "pkexec",
            &["sh".into(), script_path.display().to_string()],
            Duration::from_secs(90),
        )
        .await;
        if !ok {
            let (_, out2) = run_program(
                "sudo",
                &["-n".into(), "sh".into(), script_path.display().to_string()],
                Duration::from_secs(30),
            )
            .await;
            (format!("{out}{out2}"), false)
        } else {
            (out, ok)
        }
    };
    let log = read_text(&log_path);
    ApplyReport {
        ok: true,
        message: "Table nftables de SECTOOL supprimée.".into(),
        log: format!("{out}\n{log}"),
    }
}

/// Applique une configuration pf sous macOS.
pub async fn apply_pf(rules: &str, elevated: bool) -> ApplyReport {
    let rules_path = match write_work_file("sectool.pf", rules) {
        Ok(p) => p,
        Err(e) => return ApplyReport::failure("Impossible d'écrire les règles pf", e.to_string()),
    };
    let log_path = work_dir().join("pf.log");
    let shell = format!(
        "#!/bin/sh\n: > '{log}'\nmkdir -p /etc/pf.anchors\ncp '{rules}' /etc/pf.anchors/sectool\npfctl -a sectool -f /etc/pf.anchors/sectool >>'{log}' 2>&1\npfctl -E >>'{log}' 2>&1\necho done >>'{log}'\n",
        log = log_path.display(),
        rules = rules_path.display()
    );
    let script_path = match write_work_file("apply-pf.sh", &shell) {
        Ok(p) => p,
        Err(e) => return ApplyReport::failure("Impossible d'écrire le script", e.to_string()),
    };

    let command = format!("/bin/sh '{}'", script_path.display());
    let (ok, out) = if elevated {
        run_program("sh", &[script_path.display().to_string()], Duration::from_secs(30)).await
    } else {
        run_program(
            "osascript",
            &[
                "-e".into(),
                format!("do shell script \"{command}\" with administrator privileges"),
            ],
            Duration::from_secs(120),
        )
        .await
    };
    let log = read_text(&log_path);
    let applied = log.contains("done");
    if ok && applied {
        ApplyReport {
            ok: true,
            message: "Règles chargées dans l'ancre pf « sectool ».".into(),
            log: format!("{out}\n{log}"),
        }
    } else {
        ApplyReport::failure(
            "Application impossible (mot de passe administrateur nécessaire).",
            format!("{out}\n{log}"),
        )
    }
}

/// Point d'entrée unique : exécute le plan pour le système courant.
pub async fn apply_os(os: Os, text: &str, elevated: bool) -> ApplyReport {
    match os {
        Os::Windows => {
            let commands: Vec<String> = text
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            apply_windows(&commands, elevated).await
        }
        Os::Linux => apply_nft(text, elevated).await,
        Os::Macos => apply_pf(text, elevated).await,
    }
}
