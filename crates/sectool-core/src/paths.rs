//! Emplacements de configuration et de données, sans dépendance externe.

use std::path::PathBuf;

/// Dossier de configuration de l'utilisateur.
pub fn config_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var("APPDATA").ok().map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            })
    };
    let dir = base
        .unwrap_or_else(|| PathBuf::from("."))
        .join(if cfg!(target_os = "windows") {
            "SECTOOL"
        } else {
            "sectool"
        });
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Dossier de travail temporaire de l'application.
pub fn work_dir() -> PathBuf {
    let dir = config_dir().join("work");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Chemin d'un fichier de configuration nommé.
pub fn config_file(name: &str) -> PathBuf {
    config_dir().join(name)
}
