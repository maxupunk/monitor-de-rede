//! Tabela OUI estática. `phf` mantém a consulta O(1) sem heap.

use phf::phf_map;

static VENDORS: phf::Map<&'static str, &'static str> = phf_map! {
    "00005e" => "ICANN / Virtualization",
    "000142" => "Cisco Systems",
    "00037f" => "Intel",
    "00044d" => "Avaya",
    "00085c" => "Huawei",
    "000acd" => "IBM",
    "000b86" => "Ubiquiti Networks",
    "000c29" => "VMware",
    "000c41" => "Huawei",
    "000ecf" => "Dell",
    "000f34" => "Hewlett-Packard",
    "001018" => "Intel",
    "001132" => "Synology",
    "001185" => "Hewlett-Packard",
    "001320" => "MikroTik",
    "001372" => "MikroTik",
    "0013ce" => "Siemens",
    "001422" => "Dell",
    "001517" => "Hewlett-Packard",
    "00155d" => "Microsoft",
    "0015c5" => "MikroTik",
    "001617" => "Hewlett-Packard",
    "00188b" => "MikroTik",
    "0018de" => "Hewlett-Packard",
    "001906" => "MikroTik",
    "001999" => "Hewlett-Packard",
    "0019bb" => "MikroTik",
    "0019d1" => "Hewlett-Packard",
    "001a3f" => "MikroTik",
    "001a4d" => "Hewlett-Packard",
    "001a70" => "Hewlett-Packard",
    "001b11" => "Hewlett-Packard",
    "001b21" => "Intel",
    "001b78" => "Hewlett-Packard",
    "001cc4" => "Hewlett-Packard",
    "001d60" => "Hewlett-Packard",
    "001e0b" => "Hewlett-Packard",
    "001e37" => "Hewlett-Packard",
    "001ec1" => "Realtek",
    "001f29" => "Hewlett-Packard",
    "001ff3" => "McAfee",
    "00215a" => "Hewlett-Packard",
    "00219b" => "Hewlett-Packard",
    "002219" => "Hewlett-Packard",
    "002264" => "Hewlett-Packard",
    "00237d" => "Hewlett-Packard",
    "002481" => "Hewlett-Packard",
    "0025b3" => "Hewlett-Packard",
    "002655" => "Hewlett-Packard",
    "005056" => "VMware",
    "0060b0" => "Hewlett-Packard",
    "008048" => "Hewlett-Packard",
    "00900b" => "Hewlett-Packard",
    "00907a" => "Hewlett-Packard",
    "00a0c9" => "Intel",
    "00a0d1" => "Hewlett-Packard",
    "00aa02" => "Hewlett-Packard",
    "00bb3a" => "Hewlett-Packard",
    "00c04f" => "Dell",
    "00e04c" => "Realtek",
    "00e098" => "MikroTik",
    "0418d6" => "Ubiquiti Networks",
    "080007" => "Apple",
    "18e8dd" => "Hewlett-Packard",
    "244bfe" => "Hewlett-Packard",
    "288023" => "Hewlett-Packard",
    "2c598a" => "Hewlett-Packard",
    "2c768a" => "Hewlett-Packard",
    "30e171" => "Hewlett-Packard",
    "3c5a37" => "Hewlett-Packard",
    "3c6104" => "Espressif",
    "44a842" => "Hewlett-Packard",
    "484d7e" => "Hewlett-Packard",
    "4ccc6a" => "Hewlett-Packard",
    "541310" => "Hewlett-Packard",
    "5820b1" => "Hewlett-Packard",
    "60e327" => "Hewlett-Packard",
    "645106" => "Hewlett-Packard",
    "68b599" => "Hewlett-Packard",
    "6c3be5" => "Hewlett-Packard",
    "7054d2" => "Hewlett-Packard",
    "7446a0" => "Hewlett-Packard",
    "78acc0" => "Hewlett-Packard",
    "7c6193" => "Hewlett-Packard",
    "80c16e" => "Hewlett-Packard",
    "843497" => "Hewlett-Packard",
    "8851fb" => "Hewlett-Packard",
    "8cdcd4" => "Hewlett-Packard",
    "90e2ba" => "Hewlett-Packard",
    "9457a5" => "Hewlett-Packard",
    "984be1" => "Hewlett-Packard",
    "9cdc71" => "Hewlett-Packard",
    "a0481c" => "Hewlett-Packard",
    "a41242" => "Hewlett-Packard",
    "a45d36" => "Hewlett-Packard",
    "a8667f" => "Hewlett-Packard",
    "ac162d" => "Hewlett-Packard",
    "aca31e" => "Hewlett-Packard",
    "b00bd5" => "Hewlett-Packard",
    "b48c9d" => "Hewlett-Packard",
    "b88303" => "Hewlett-Packard",
    "bc83a7" => "Hewlett-Packard",
    "c02e25" => "Hewlett-Packard",
    "c09134" => "Hewlett-Packard",
    "c4346b" => "Hewlett-Packard",
    "c80873" => "Hewlett-Packard",
    "c8cbb8" => "Hewlett-Packard",
    "cc3e5f" => "Hewlett-Packard",
    "d067e5" => "Hewlett-Packard",
    "d48564" => "Hewlett-Packard",
    "d89d67" => "Hewlett-Packard",
    "dc4a3e" => "Hewlett-Packard",
    "e0071b" => "Hewlett-Packard",
    "e070ea" => "Hewlett-Packard",
    "e4115b" => "Hewlett-Packard",
    "e83935" => "Hewlett-Packard",
    "ecb1d7" => "Hewlett-Packard",
    "f0921c" => "Hewlett-Packard",
    "f430b9" => "Hewlett-Packard",
    "f80de0" => "Hewlett-Packard",
    "fc15b4" => "Hewlett-Packard",
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

#[cfg(test)]
mod tests {
    use super::lookup_vendor;

    #[test]
    fn identifica_fabricantes_de_rede_com_mac_formatado_ou_nao() {
        assert_eq!(
            lookup_vendor("00:0B:86:12:34:56"),
            Some("Ubiquiti Networks")
        );
        assert_eq!(lookup_vendor("001320abcdef"), Some("MikroTik"));
    }

    #[test]
    fn mac_incompleto_nao_e_consultado() {
        assert_eq!(lookup_vendor("00:0b"), None);
    }
}
