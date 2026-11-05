//! Compteurs de trafic par interface.

use std::collections::HashMap;
use std::time::Duration;

use sectool_core::traffic::IfaceTraffic;
use sectool_core::now_ms;

use crate::sys;

/// Garde les compteurs précédents pour calculer un débit.
#[derive(Default)]
pub struct TrafficReader {
    last: HashMap<String, (u64, u64)>,
    last_ts: i64,
}

impl TrafficReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Relève les compteurs et renvoie le débit depuis le dernier relevé.
    pub async fn sample(&mut self) -> Vec<IfaceTraffic> {
        let raw = read_counters().await;
        let ts = now_ms();
        let elapsed = if self.last_ts == 0 {
            0.0
        } else {
            ((ts - self.last_ts) as f64 / 1000.0).max(0.001)
        };
        let mut out = Vec::new();
        for (name, rx, tx) in raw {
            let (rx_bps, tx_bps) = match self.last.get(&name) {
                Some((prx, ptx)) if elapsed > 0.0 => (
                    per_second(rx.saturating_sub(*prx), elapsed),
                    per_second(tx.saturating_sub(*ptx), elapsed),
                ),
                _ => (0, 0),
            };
            out.push(IfaceTraffic {
                name: name.clone(),
                rx_bps,
                tx_bps,
                rx_total: rx,
                tx_total: tx,
            });
            self.last.insert(name, (rx, tx));
        }
        self.last_ts = ts;
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

fn per_second(delta: u64, seconds: f64) -> u64 {
    if seconds <= 0.0 {
        return 0;
    }
    ((delta as f64) / seconds) as u64
}

async fn read_counters() -> Vec<(String, u64, u64)> {
    if cfg!(target_os = "windows") {
        windows_counters().await
    } else if cfg!(target_os = "macos") {
        macos_counters().await
    } else {
        linux_counters()
    }
}

fn linux_counters() -> Vec<(String, u64, u64)> {
    let mut out = Vec::new();
    if let Ok(text) = std::fs::read_to_string("/proc/net/dev") {
        for line in text.lines().skip(2) {
            let (name, rest) = match line.split_once(':') {
                Some(v) => v,
                None => continue,
            };
            let nums: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|n| n.parse::<u64>().ok())
                .collect();
            if nums.len() >= 9 {
                out.push((name.trim().to_string(), nums[0], nums[8]));
            }
        }
    }
    out
}

async fn macos_counters() -> Vec<(String, u64, u64)> {
    let mut out: HashMap<String, (u64, u64)> = HashMap::new();
    let text = match sys::run("netstat", &["-ibn"], Duration::from_secs(4)).await {
        Some(t) => t,
        None => return Vec::new(),
    };
    let mut ib_idx = None;
    let mut ob_idx = None;
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.is_empty() {
            continue;
        }
        if line.starts_with("Name") {
            ib_idx = cols.iter().position(|c| *c == "Ibytes");
            ob_idx = cols.iter().position(|c| *c == "Obytes");
            continue;
        }
        let (Some(ib), Some(ob)) = (ib_idx, ob_idx) else {
            continue;
        };
        if cols.len() <= ob {
            continue;
        }
        let name = cols[0].to_string();
        let rx = cols[ib].parse::<u64>().unwrap_or(0);
        let tx = cols[ob].parse::<u64>().unwrap_or(0);
        if rx == 0 && tx == 0 {
            continue;
        }
        let entry = out.entry(name).or_insert((0, 0));
        // `netstat -ib` répète une ligne par famille : on garde le maximum.
        entry.0 = entry.0.max(rx);
        entry.1 = entry.1.max(tx);
    }
    out.into_iter().map(|(n, (rx, tx))| (n, rx, tx)).collect()
}

async fn windows_counters() -> Vec<(String, u64, u64)> {
    let text = match sys::run("netstat", &["-e"], Duration::from_secs(4)).await {
        Some(t) => t,
        None => return Vec::new(),
    };
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() == 3 {
            if let (Ok(rx), Ok(tx)) = (cols[1].parse::<u64>(), cols[2].parse::<u64>()) {
                return vec![("global".to_string(), rx, tx)];
            }
        }
    }
    Vec::new()
}
