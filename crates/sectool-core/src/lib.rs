//! SECTOOL — noyau partagé.
//!
//! Ce crate ne fait aucune entrée/sortie : il ne contient que des types, des
//! règles de décision et des utilitaires purs. Tout ce qui touche au réseau ou
//! au système d'exploitation vit dans `sectool-net` et `sectool-firewall`.

pub mod alert;
pub mod connection;
pub mod device;
pub mod firewall;
pub mod net_util;
pub mod oui;
pub mod paths;
pub mod settings;
pub mod snapshot;
pub mod traffic;

pub use alert::{Alert, AlertKind, AlertLevel};
pub use connection::{ConnState, Connection, Scope};
pub use device::{Device, DeviceKind, PortInfo};
pub use firewall::{ApplyPlan, Direction, FirewallRule, FirewallStatus, Proto, RuleAction};
pub use net_util::{is_private, ip_scope, normalize_mac, now_ms};
pub use settings::{Settings, UiSettings};
pub use snapshot::{InterfaceInfo, Snapshot, Stats};
pub use traffic::{IfaceTraffic, TrafficPoint, TrafficSample, TrafficSnapshot};

/// Version affichée dans l'interface et dans les journaux.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
