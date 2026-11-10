//! Texto de fora (URL, purl): decodificado num lugar só.

/// `%3A` → `:`. Sequência inválida fica como veio.
#[must_use]
pub fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&raw[index + 1..index + 3], 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodifica_e_tolera_lixo() {
        assert_eq!(percent_decode("sha256%3Aab"), "sha256:ab");
        assert_eq!(percent_decode("%40vue%2Freactivity"), "@vue/reactivity");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("fim%3A"), "fim:");
    }
}
