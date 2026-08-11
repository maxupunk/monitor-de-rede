//! Tabela OUI estática. `phf` mantém a consulta O(1) sem heap.

use phf::phf_map;

static VENDORS: phf::Map<&'static str, &'static str> = phf_map! {
    "00163e" => "Xensource", "001a2b" => "Cisco", "001b54" => "Cisco", "001c42" => "Cisco",
    "002590" => "Cisco", "001c23" => "Juniper", "001f9e" => "Cisco", "0020d8" => "Cisco",
    "001018" => "Broadcom", "001122" => "Cimsys", "001e8c" => "ASUSTek",
    "001a11" => "Google", "0017f2" => "Apple", "a4c361" => "Apple", "f0d1a9" => "Apple",
    "3c5a37" => "Google", "f4f5e8" => "Google", "d850e6" => "ASUSTek", "001f3f" => "AVM",
    "e48d8c" => "TP-Link", "14cc20" => "TP-Link", "60e327" => "TP-Link", "50c7bf" => "TP-Link",
    "b0487a" => "TP-Link", "001e58" => "D-Link", "001cf0" => "Samsung", "001dd8" => "Cisco",
    "0024a8" => "Cisco", "000c29" => "VMware", "005056" => "VMware", "001c14" => "VMware",
    "080027" => "PCS Systemtechnik", "525400" => "QEMU", "00155d" => "Microsoft", "001c7c" => "Pericom",
    "001b21" => "Intel", "3cecef" => "Apple", "b827eb" => "Raspberry Pi", "dca632" => "Raspberry Pi",
    "e45f01" => "Raspberry Pi", "0013ef" => "Cisco", "0016b6" => "Cisco", "0019e8" => "Cisco",
};

#[must_use]
pub fn lookup_vendor(mac_address: &str) -> Option<&'static str> {
    let normalized: String = mac_address
        .chars()
        .filter(|char| char.is_ascii_hexdigit())
        .take(6)
        .map(|char| char.to_ascii_lowercase())
        .collect();
    (normalized.len() == 6)
        .then(|| VENDORS.get(normalized.as_str()).copied())
        .flatten()
}
