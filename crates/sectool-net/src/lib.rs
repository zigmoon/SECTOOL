//! Découverte et surveillance réseau pour SECTOOL.
//!
//! Tout est en lecture seule côté système : ce crate n'applique aucune règle de
//! pare-feu (c'est le rôle de `sectool-firewall`).

pub mod ifaces;
pub mod mdns;
pub mod neighbors;
pub mod probe;
pub mod scan;
pub mod sockets;
pub mod sys;
pub mod traffic;

pub use ifaces::{local_interfaces, LocalIface};
pub use scan::{ScanOutcome, Scanner};
pub use sockets::{snapshot as socket_snapshot, SocketSnapshot};
pub use traffic::TrafficReader;
