//! Appareils présents sur le réseau local.

use serde::{Deserialize, Serialize};

/// Catégorie d'appareil, utilisée pour choisir l'icône et la couleur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    Router,
    Computer,
    Laptop,
    Phone,
    Tablet,
    Tv,
    Printer,
    Camera,
    Speaker,
    Console,
    Nas,
    Iot,
    Server,
    Unknown,
}

impl DeviceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceKind::Router => "router",
            DeviceKind::Computer => "computer",
            DeviceKind::Laptop => "laptop",
            DeviceKind::Phone => "phone",
            DeviceKind::Tablet => "tablet",
            DeviceKind::Tv => "tv",
            DeviceKind::Printer => "printer",
            DeviceKind::Camera => "camera",
            DeviceKind::Speaker => "speaker",
            DeviceKind::Console => "console",
            DeviceKind::Nas => "nas",
            DeviceKind::Iot => "iot",
            DeviceKind::Server => "server",
            DeviceKind::Unknown => "unknown",
        }
    }

    /// Devine une catégorie à partir d'une chaîne libre (nom d'hôte, modèle,
    /// type mDNS…).
    pub fn from_hint(hint: &str) -> Option<Self> {
        let h = hint.to_ascii_lowercase();
        let table: &[(&str, DeviceKind)] = &[
            ("freebox", DeviceKind::Router),
            ("livebox", DeviceKind::Router),
            ("bbox", DeviceKind::Router),
            ("sfr-box", DeviceKind::Router),
            ("sfr_box", DeviceKind::Router),
            ("routeur", DeviceKind::Router),
            ("router", DeviceKind::Router),
            ("gateway", DeviceKind::Router),
            ("box", DeviceKind::Router),
            ("iphone", DeviceKind::Phone),
            ("ipad", DeviceKind::Tablet),
            ("ipod", DeviceKind::Phone),
            ("macbook", DeviceKind::Laptop),
            ("imac", DeviceKind::Computer),
            ("mac-mini", DeviceKind::Computer),
            ("macmini", DeviceKind::Computer),
            ("mac-pro", DeviceKind::Computer),
            ("mac-", DeviceKind::Computer),
            ("galaxy", DeviceKind::Phone),
            ("pixel", DeviceKind::Phone),
            ("redmi", DeviceKind::Phone),
            ("oneplus", DeviceKind::Phone),
            ("android", DeviceKind::Phone),
            ("phone", DeviceKind::Phone),
            ("tablet", DeviceKind::Tablet),
            ("tab-", DeviceKind::Tablet),
            ("bravia", DeviceKind::Tv),
            ("tv", DeviceKind::Tv),
            ("television", DeviceKind::Tv),
            ("chromecast", DeviceKind::Tv),
            ("appletv", DeviceKind::Tv),
            ("apple-tv", DeviceKind::Tv),
            ("firetv", DeviceKind::Tv),
            ("fire-tv", DeviceKind::Tv),
            ("shield", DeviceKind::Tv),
            ("roku", DeviceKind::Tv),
            ("printer", DeviceKind::Printer),
            ("imprimante", DeviceKind::Printer),
            ("epson", DeviceKind::Printer),
            ("canon", DeviceKind::Printer),
            ("brother", DeviceKind::Printer),
            ("laserjet", DeviceKind::Printer),
            ("deskjet", DeviceKind::Printer),
            ("officejet", DeviceKind::Printer),
            ("cam", DeviceKind::Camera),
            ("ipcam", DeviceKind::Camera),
            ("ipc", DeviceKind::Camera),
            ("hikvision", DeviceKind::Camera),
            ("reolink", DeviceKind::Camera),
            ("dahua", DeviceKind::Camera),
            ("doorbell", DeviceKind::Camera),
            ("sonos", DeviceKind::Speaker),
            ("homepod", DeviceKind::Speaker),
            ("echo", DeviceKind::Speaker),
            ("speaker", DeviceKind::Speaker),
            ("enceinte", DeviceKind::Speaker),
            ("xbox", DeviceKind::Console),
            ("playstation", DeviceKind::Console),
            ("ps4", DeviceKind::Console),
            ("ps5", DeviceKind::Console),
            ("nintendo", DeviceKind::Console),
            ("switch", DeviceKind::Console),
            ("nas", DeviceKind::Nas),
            ("synology", DeviceKind::Nas),
            ("diskstation", DeviceKind::Nas),
            ("qnap", DeviceKind::Nas),
            ("truenas", DeviceKind::Nas),
            ("raspberry", DeviceKind::Server),
            ("esp32", DeviceKind::Iot),
            ("esp8266", DeviceKind::Iot),
            ("shelly", DeviceKind::Iot),
            ("sonoff", DeviceKind::Iot),
            ("tuya", DeviceKind::Iot),
            ("tasmota", DeviceKind::Iot),
            ("lamp", DeviceKind::Iot),
            ("hue", DeviceKind::Iot),
            ("server", DeviceKind::Server),
            ("serveur", DeviceKind::Server),
            ("pc-", DeviceKind::Computer),
            ("desktop", DeviceKind::Computer),
            ("laptop", DeviceKind::Laptop),
            ("lenovo", DeviceKind::Laptop),
            ("thinkpad", DeviceKind::Laptop),
            ("dell", DeviceKind::Laptop),
            ("hp-", DeviceKind::Laptop),
        ];
        for (needle, kind) in table {
            if h.contains(needle) {
                return Some(*kind);
            }
        }
        None
    }
}

impl Default for DeviceKind {
    fn default() -> Self {
        DeviceKind::Unknown
    }
}

/// Un port ouvert détecté sur un appareil.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u32>,
}

/// Nom lisible d'un port TCP bien connu.
pub fn service_for_port(port: u16) -> Option<&'static str> {
    let name = match port {
        21 => "FTP",
        22 => "SSH",
        23 => "Telnet",
        53 => "DNS",
        80 => "HTTP",
        139 => "NetBIOS",
        443 => "HTTPS",
        445 => "SMB",
        515 => "LPD",
        548 => "AFP",
        554 => "RTSP",
        631 => "IPP",
        1883 => "MQTT",
        2049 => "NFS",
        3000 => "Dev",
        3306 => "MySQL",
        3389 => "RDP",
        5000 => "UPnP",
        5001 => "NAS TLS",
        5060 => "SIP",
        5353 => "mDNS",
        5432 => "PostgreSQL",
        5555 => "ADB",
        7000 => "AirPlay",
        8000 => "HTTP alt",
        8008 => "Cast",
        8009 => "Cast",
        8080 => "HTTP proxy",
        8443 => "HTTPS alt",
        8554 => "RTSP alt",
        9000 => "Portail",
        9100 => "Impression",
        32400 => "Plex",
        62078 => "iPhone sync",
        _ => return None,
    };
    Some(name)
}

/// Un appareil du réseau local.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub kind: DeviceKind,
    pub online: bool,
    pub is_gateway: bool,
    pub is_self: bool,
    pub first_seen: i64,
    pub last_seen: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<PortInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<String>,
    /// Origines de l'information : `arp`, `tcp`, `mdns`, `dns`, `gateway`.
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iface: Option<String>,
    /// L'appareil est-il visé par une règle de pare-feu ?
    #[serde(default)]
    pub blocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Device {
    /// Clé stable : la MAC quand on l'a, sinon l'IP.
    pub fn key(mac: Option<&str>, ip: Option<&str>) -> String {
        match (mac, ip) {
            (Some(m), _) => format!("mac:{m}"),
            (None, Some(i)) => format!("ip:{i}"),
            _ => "unknown".to_string(),
        }
    }

    /// Devine la catégorie à partir de tout ce qu'on sait.
    pub fn infer_kind(&self) -> DeviceKind {
        if self.is_gateway {
            return DeviceKind::Router;
        }
        // 1. Nom d'hôte / modèle
        for hint in [self.hostname.as_deref(), self.model.as_deref()] {
            if let Some(h) = hint {
                if let Some(k) = DeviceKind::from_hint(h) {
                    return k;
                }
            }
        }
        // 2. Services annoncés
        for s in &self.services {
            if let Some(k) = DeviceKind::from_hint(s) {
                return k;
            }
        }
        // 3. Ports typiques
        let has = |p: u16| self.ports.iter().any(|x| x.port == p);
        if has(9100) || has(631) || has(515) {
            return DeviceKind::Printer;
        }
        if has(554) || has(8554) {
            return DeviceKind::Camera;
        }
        if has(62078) {
            return DeviceKind::Phone;
        }
        if has(32400) || has(8009) || has(8008) {
            return DeviceKind::Tv;
        }
        if has(5000) || has(5001) || has(2049) || has(548) {
            return DeviceKind::Nas;
        }
        if has(22) && has(445) {
            return DeviceKind::Server;
        }
        // 4. Constructeur
        if let Some(v) = &self.vendor {
            if let Some(k) = DeviceKind::from_hint(v) {
                return k;
            }
        }
        DeviceKind::Unknown
    }
}
