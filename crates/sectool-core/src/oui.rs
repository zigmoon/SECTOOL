//! Base locale de préfixes MAC (OUI) → constructeur + type probable.
//!
//! Volontairement embarquée et compacte : couvre le matériel que l'on croise
//! vraiment dans une maison ou un petit bureau (box, PC, téléphones, TV,
//! imprimantes, objets connectés). Aucun appel réseau n'est nécessaire.

use crate::device::DeviceKind;
use crate::net_util::oui_prefix;

struct Entry {
    prefix: &'static str,
    vendor: &'static str,
    kind: Option<DeviceKind>,
}

const fn e(prefix: &'static str, vendor: &'static str, kind: Option<DeviceKind>) -> Entry {
    Entry {
        prefix,
        vendor,
        kind,
    }
}

/// Table triée à la main. L'ordre importe peu : la recherche est linéaire et
/// la table reste petite (≈ 230 entrées).
static TABLE: &[Entry] = &[
    // ── Box / routeurs français ──────────────────────────────────────────────
    e("f4cae5", "Free SAS", Some(DeviceKind::Router)),
    e("140c76", "Freebox SAS", Some(DeviceKind::Router)),
    e("001ca0", "Freebox", Some(DeviceKind::Router)),
    e("001f9f", "Sagemcom", Some(DeviceKind::Router)),
    e("346bd3", "Sagemcom", Some(DeviceKind::Router)),
    e("7c03d8", "Technicolor", Some(DeviceKind::Router)),
    e("001a2b", "Sagemcom", Some(DeviceKind::Router)),
    e("d4a26c", "Arcadyan", Some(DeviceKind::Router)),
    // ── Routeurs / Wi-Fi grand public ───────────────────────────────────────
    e("001d0f", "TP-Link", Some(DeviceKind::Router)),
    e("14cc20", "TP-Link", Some(DeviceKind::Router)),
    e("50c7bf", "TP-Link", Some(DeviceKind::Router)),
    e("a42bb0", "TP-Link", Some(DeviceKind::Router)),
    e("c46e1f", "TP-Link", Some(DeviceKind::Router)),
    e("ec086b", "TP-Link", Some(DeviceKind::Router)),
    e("00146c", "Netgear", Some(DeviceKind::Router)),
    e("204e7f", "Netgear", Some(DeviceKind::Router)),
    e("a040a0", "Netgear", Some(DeviceKind::Router)),
    e("c03f0e", "Netgear", Some(DeviceKind::Router)),
    e("4494fc", "Netgear", Some(DeviceKind::Router)),
    e("00156d", "Ubiquiti", Some(DeviceKind::Router)),
    e("24a43c", "Ubiquiti", Some(DeviceKind::Router)),
    e("74acb9", "Ubiquiti", Some(DeviceKind::Router)),
    e("788a20", "Ubiquiti", Some(DeviceKind::Router)),
    e("dc9fdb", "Ubiquiti", Some(DeviceKind::Router)),
    e("001b11", "D-Link", Some(DeviceKind::Router)),
    e("1c7ee5", "D-Link", Some(DeviceKind::Router)),
    e("78a528", "D-Link", Some(DeviceKind::Router)),
    e("001fc6", "ASUSTek", Some(DeviceKind::Router)),
    e("2c56dc", "ASUSTek", Some(DeviceKind::Router)),
    e("ac9e17", "ASUSTek", Some(DeviceKind::Router)),
    e("d850e6", "ASUSTek", Some(DeviceKind::Router)),
    e("00000c", "Cisco", Some(DeviceKind::Router)),
    e("001b54", "Cisco", Some(DeviceKind::Router)),
    e("2c3f38", "Cisco", Some(DeviceKind::Router)),
    // ── Apple ──────────────────────────────────────────────────────────────
    e("001b63", "Apple", None),
    e("001ec2", "Apple", None),
    e("002500", "Apple", None),
    e("3c0754", "Apple", None),
    e("8c8590", "Apple", None),
    e("a483e7", "Apple", None),
    e("f01898", "Apple", None),
    e("d0817a", "Apple", None),
    e("147dda", "Apple", None),
    e("6c19c0", "Apple", None),
    e("90b0ed", "Apple", None),
    e("f0dbe2", "Apple", None),
    e("bc926b", "Apple", None),
    e("04489a", "Apple", None),
    // ── Téléphones / tablettes ─────────────────────────────────────────────
    e("0012fb", "Samsung", Some(DeviceKind::Phone)),
    e("001d25", "Samsung", Some(DeviceKind::Phone)),
    e("3423ba", "Samsung", Some(DeviceKind::Phone)),
    e("5c0a5b", "Samsung", Some(DeviceKind::Phone)),
    e("781fdb", "Samsung", Some(DeviceKind::Phone)),
    e("8c7712", "Samsung", Some(DeviceKind::Phone)),
    e("e8508b", "Samsung", Some(DeviceKind::Phone)),
    e("009ec8", "Xiaomi", Some(DeviceKind::Phone)),
    e("286c07", "Xiaomi", Some(DeviceKind::Phone)),
    e("34ce00", "Xiaomi", Some(DeviceKind::Phone)),
    e("64b473", "Xiaomi", Some(DeviceKind::Phone)),
    e("7811dc", "Xiaomi", Some(DeviceKind::Phone)),
    e("8cbebe", "Xiaomi", Some(DeviceKind::Phone)),
    e("f0b429", "Xiaomi", Some(DeviceKind::Phone)),
    e("001882", "Huawei", Some(DeviceKind::Phone)),
    e("00e0fc", "Huawei", Some(DeviceKind::Phone)),
    e("04bd70", "Huawei", Some(DeviceKind::Phone)),
    e("20f3a3", "Huawei", Some(DeviceKind::Phone)),
    e("48db50", "Huawei", Some(DeviceKind::Phone)),
    e("70723c", "Huawei", Some(DeviceKind::Phone)),
    e("8853d4", "Huawei", Some(DeviceKind::Phone)),
    e("0012fe", "Lenovo", Some(DeviceKind::Laptop)),
    e("60d819", "Lenovo", Some(DeviceKind::Laptop)),
    e("e86a64", "Lenovo", Some(DeviceKind::Laptop)),
    e("001b21", "Intel", None),
    e("3c970e", "Intel", None),
    e("7c7a91", "Intel", None),
    e("a4bb6d", "Intel", None),
    e("e4a471", "Intel", None),
    e("00e04c", "Realtek", None),
    e("001018", "Broadcom", None),
    e("001422", "Dell", Some(DeviceKind::Laptop)),
    e("180373", "Dell", Some(DeviceKind::Laptop)),
    e("b82a72", "Dell", Some(DeviceKind::Laptop)),
    e("f8bc12", "Dell", Some(DeviceKind::Laptop)),
    e("001b78", "HP", Some(DeviceKind::Printer)),
    e("3cd92b", "HP", None),
    e("9cb654", "HP", None),
    e("d48564", "HP", None),
    e("00125a", "Microsoft", Some(DeviceKind::Console)),
    e("281878", "Microsoft", None),
    e("501ac5", "Microsoft", None),
    e("7c1e52", "Microsoft", Some(DeviceKind::Console)),
    e("985fd3", "Microsoft", None),
    e("0016d3", "Foxconn", None),
    e("902b34", "Foxconn", None),
    // ── TV / consoles / enceintes ──────────────────────────────────────────
    e("00041f", "Sony", Some(DeviceKind::Tv)),
    e("00248c", "Sony", Some(DeviceKind::Console)),
    e("544249", "Sony", Some(DeviceKind::Tv)),
    e("78843c", "Sony", Some(DeviceKind::Tv)),
    e("fc0fe6", "Sony", Some(DeviceKind::Tv)),
    e("000d4b", "Roku", Some(DeviceKind::Tv)),
    e("b0a737", "Roku", Some(DeviceKind::Tv)),
    e("cc6da0", "Roku", Some(DeviceKind::Tv)),
    e("d83134", "Roku", Some(DeviceKind::Tv)),
    e("00fc8b", "Amazon", Some(DeviceKind::Tv)),
    e("44650d", "Amazon", Some(DeviceKind::Tv)),
    e("50dce7", "Amazon", Some(DeviceKind::Speaker)),
    e("6837e9", "Amazon", Some(DeviceKind::Speaker)),
    e("74c246", "Amazon", Some(DeviceKind::Speaker)),
    e("f0272d", "Amazon", Some(DeviceKind::Tv)),
    e("3c5ab4", "Google", Some(DeviceKind::Tv)),
    e("546009", "Google", Some(DeviceKind::Tv)),
    e("6cadf8", "Google", Some(DeviceKind::Tv)),
    e("94eb2c", "Google", Some(DeviceKind::Tv)),
    e("a47733", "Google", Some(DeviceKind::Tv)),
    e("f4f5d8", "Google", Some(DeviceKind::Tv)),
    e("0009bf", "Nintendo", Some(DeviceKind::Console)),
    e("58bda3", "Nintendo", Some(DeviceKind::Console)),
    e("98b6e9", "Nintendo", Some(DeviceKind::Console)),
    e("e0e751", "Nintendo", Some(DeviceKind::Console)),
    e("000e58", "Sonos", Some(DeviceKind::Speaker)),
    e("347e5c", "Sonos", Some(DeviceKind::Speaker)),
    e("5caafd", "Sonos", Some(DeviceKind::Speaker)),
    e("7828ca", "Sonos", Some(DeviceKind::Speaker)),
    e("b8e937", "Sonos", Some(DeviceKind::Speaker)),
    // ── Imprimantes ────────────────────────────────────────────────────────
    e("000085", "Canon", Some(DeviceKind::Printer)),
    e("2c9efc", "Canon", Some(DeviceKind::Printer)),
    e("888717", "Canon", Some(DeviceKind::Printer)),
    e("000048", "Epson", Some(DeviceKind::Printer)),
    e("389d92", "Epson", Some(DeviceKind::Printer)),
    e("a4ee57", "Epson", Some(DeviceKind::Printer)),
    e("008077", "Brother", Some(DeviceKind::Printer)),
    e("30055c", "Brother", Some(DeviceKind::Printer)),
    e("b42200", "Brother", Some(DeviceKind::Printer)),
    // ── NAS / serveurs ─────────────────────────────────────────────────────
    e("001132", "Synology", Some(DeviceKind::Nas)),
    e("9009d0", "Synology", Some(DeviceKind::Nas)),
    e("245ebe", "QNAP", Some(DeviceKind::Nas)),
    e("00089b", "QNAP", Some(DeviceKind::Nas)),
    e("0090a9", "Western Digital", Some(DeviceKind::Nas)),
    e("b827eb", "Raspberry Pi", Some(DeviceKind::Server)),
    e("dca632", "Raspberry Pi", Some(DeviceKind::Server)),
    e("e45f01", "Raspberry Pi", Some(DeviceKind::Server)),
    e("28cdc1", "Raspberry Pi", Some(DeviceKind::Server)),
    // ── Caméras ────────────────────────────────────────────────────────────
    e("4419b6", "Hikvision", Some(DeviceKind::Camera)),
    e("c056e3", "Hikvision", Some(DeviceKind::Camera)),
    e("bcad28", "Hikvision", Some(DeviceKind::Camera)),
    e("ec71db", "Reolink", Some(DeviceKind::Camera)),
    e("3cef8c", "Dahua", Some(DeviceKind::Camera)),
    // ── Objets connectés ───────────────────────────────────────────────────
    e("240ac4", "Espressif", Some(DeviceKind::Iot)),
    e("3c71bf", "Espressif", Some(DeviceKind::Iot)),
    e("84f3eb", "Espressif", Some(DeviceKind::Iot)),
    e("a4cf12", "Espressif", Some(DeviceKind::Iot)),
    e("cc50e3", "Espressif", Some(DeviceKind::Iot)),
    e("10d561", "Tuya", Some(DeviceKind::Iot)),
    e("68572d", "Tuya", Some(DeviceKind::Iot)),
    e("d81f12", "Tuya", Some(DeviceKind::Iot)),
    e("001788", "Philips Hue", Some(DeviceKind::Iot)),
    e("dc4f22", "Sonoff", Some(DeviceKind::Iot)),
    e("84cca8", "Shelly", Some(DeviceKind::Iot)),
    // ── Virtualisation (utile pour se repérer) ─────────────────────────────
    e("000c29", "VMware", Some(DeviceKind::Server)),
    e("005056", "VMware", Some(DeviceKind::Server)),
    e("080027", "VirtualBox", Some(DeviceKind::Server)),
    e("001c42", "Parallels", Some(DeviceKind::Server)),
    e("0242ac", "Docker", Some(DeviceKind::Server)),
    e("525400", "QEMU", Some(DeviceKind::Server)),
];

/// Résultat d'une recherche OUI.
pub struct OuiMatch {
    pub vendor: &'static str,
    pub kind: Option<DeviceKind>,
}

/// Cherche le constructeur d'une adresse MAC.
pub fn lookup(mac: &str) -> Option<OuiMatch> {
    let prefix = oui_prefix(mac)?;
    TABLE
        .iter()
        .find(|entry| entry.prefix == prefix)
        .map(|entry| OuiMatch {
            vendor: entry.vendor,
            kind: entry.kind,
        })
}

/// Nombre d'entrées connues (affiché dans les réglages).
pub fn known_vendors() -> usize {
    TABLE.len()
}

/// Devine un constructeur à partir d'un nom d'hôte quand la MAC est inconnue.
pub fn vendor_from_hostname(host: &str) -> Option<&'static str> {
    let h = format!(" {}", host.to_ascii_lowercase());
    let pairs: &[(&str, &str)] = &[
        ("iphone", "Apple"),
        ("ipad", "Apple"),
        ("macbook", "Apple"),
        ("imac", "Apple"),
        ("appletv", "Apple"),
        ("homepod", "Apple"),
        ("freebox", "Free"),
        ("livebox", "Orange"),
        ("bbox", "Bouygues"),
        ("sfr", "SFR"),
        ("android", "Android"),
        ("galaxy", "Samsung"),
        ("redmi", "Xiaomi"),
        ("pixel", "Google"),
        ("chromecast", "Google"),
        ("firetv", "Amazon"),
        ("echo", "Amazon"),
        ("bravia", "Sony"),
        ("playstation", "Sony"),
        ("xbox", "Microsoft"),
        ("switch", "Nintendo"),
        ("synology", "Synology"),
        ("qnap", "QNAP"),
        ("raspberry", "Raspberry Pi"),
        ("espressif", "Espressif"),
        ("shelly", "Shelly"),
        ("hikvision", "Hikvision"),
        ("reolink", "Reolink"),
        ("printer", "Imprimante"),
    ];
    for (needle, vendor) in pairs {
        if h.contains(needle) {
            return Some(vendor);
        }
    }
    None
}
