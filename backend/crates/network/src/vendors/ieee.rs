//! O registro público de fabricantes do IEEE (Registration Authority).
//!
//! Três arquivos CSV, um por tamanho de bloco: MA-L (o OUI de 24 bits que
//! quase todo fabricante tem), MA-M (28 bits) e MA-S (36 bits, comum em IoT e
//! equipamento industrial, que compra blocos pequenos). O formato é
//! `Registry,Assignment,Organization Name,Organization Address`, com aspas
//! onde o nome ou o endereço têm vírgula.

/// Onde o servidor busca o registro. `OUI_SOURCES` (URLs separadas por
/// vírgula) troca a origem — um espelho interno numa rede sem internet.
pub const DEFAULT_SOURCES: &[&str] = &[
    "https://standards-oui.ieee.org/oui/oui.csv",
    "https://standards-oui.ieee.org/oui28/mam.csv",
    "https://standards-oui.ieee.org/oui36/oui36.csv",
];

/// Uma linha útil do registro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    /// Prefixo em hexadecimal minúsculo: 6, 7 ou 9 dígitos.
    pub prefix: String,
    pub organization: String,
    /// `MA-L`, `MA-M` ou `MA-S`.
    pub registry: String,
}

/// Maior nome guardado — a coluna é `varchar(255)`.
const MAX_ORGANIZATION: usize = 255;

/// Registros do CSV com aspas, inclusive campo com quebra de linha dentro.
fn records(text: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(char) = chars.next() {
        match (char, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => record.push(std::mem::take(&mut field)),
            ('\n', false) => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            ('\r', false) => {}
            (other, _) => field.push(other),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    records
}

/// As atribuições válidas de um CSV do IEEE. Cabeçalho, linha quebrada e
/// bloco "Private" (o dono pediu para não ser listado) ficam de fora.
#[must_use]
pub fn parse(text: &str) -> Vec<Assignment> {
    records(text)
        .into_iter()
        .filter_map(|fields| {
            let registry = fields.first()?.trim();
            let prefix = fields.get(1)?.trim().to_ascii_lowercase();
            let organization = fields
                .get(2)?
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let valid = registry.starts_with("MA-")
                && matches!(prefix.len(), 6 | 7 | 9)
                && prefix.chars().all(|char| char.is_ascii_hexdigit())
                && !organization.is_empty()
                && !organization.eq_ignore_ascii_case("private");
            valid.then(|| Assignment {
                prefix,
                organization: organization.chars().take(MAX_ORGANIZATION).collect(),
                registry: registry.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Registry,Assignment,Organization Name,Organization Address\r\n\
MA-L,5CCF7F,Espressif Inc.,\"Room 204, Building 2, Shanghai CN 201203\"\r\n\
MA-L,F8E43B,\"ASIX Electronics Corporation\",\"4F, No. 8, Hsin Ann Rd. Hsinchu TW 300\"\r\n\
MA-L,FCFFAA,Private,\r\n\
MA-M,70B3D51,\"Fabricante \"\"Aspas\"\" Ltda\",\"Rua A,\nlinha 2\"\r\n\
MA-S,70B3D5123,Sensor   Industrial,\r\n\
lixo sem campos\r\n";

    #[test]
    fn le_os_tres_tamanhos_de_bloco() {
        let parsed = parse(SAMPLE);
        let prefixes: Vec<_> = parsed.iter().map(|item| item.prefix.as_str()).collect();
        assert_eq!(prefixes, ["5ccf7f", "f8e43b", "70b3d51", "70b3d5123"]);
        assert_eq!(parsed[0].organization, "Espressif Inc.");
        assert_eq!(parsed[1].organization, "ASIX Electronics Corporation");
        assert_eq!(parsed[2].organization, "Fabricante \"Aspas\" Ltda");
        assert_eq!(parsed[2].registry, "MA-M");
        assert_eq!(parsed[3].organization, "Sensor Industrial");
    }

    #[test]
    fn bloco_privado_e_cabecalho_ficam_de_fora() {
        assert!(parse(SAMPLE).iter().all(|item| item.prefix != "fcffaa"));
        assert!(parse("Registry,Assignment,Organization Name\n").is_empty());
    }
}
