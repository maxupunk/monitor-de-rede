//! Digest de conteúdo (`sha256:<64 hex>`), a identidade de tudo no registry.
//!
//! Só `sha256`: é o que Docker, BuildKit e a especificação usam por padrão, e
//! aceitar outro algoritmo seria só mais superfície. Validado pela regex da
//! especificação — um digest nunca vira caminho de arquivo sem passar aqui.

use std::{fmt, str::FromStr};

use sha2::{Digest as _, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Digest(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("digest inválido: use sha256:<64 hexadecimais minúsculos>")]
pub struct InvalidDigest;

impl Digest {
    /// Os 64 hexadecimais.
    #[must_use]
    pub fn hex(&self) -> &str {
        &self.0["sha256:".len()..]
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// O digest de um conteúdo inteiro em memória (manifests).
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
    }
}

impl FromStr for Digest {
    type Err = InvalidDigest;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let hex = raw.strip_prefix("sha256:").ok_or(InvalidDigest)?;
        if hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(raw.to_string()))
        } else {
            Err(InvalidDigest)
        }
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Hash incremental: o conteúdo é conferido enquanto chega, sem segunda
/// leitura.
#[derive(Default, Clone)]
pub struct Hasher(Sha256);

impl Hasher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    #[must_use]
    pub fn finish(self) -> Digest {
        Digest(format!("sha256:{}", hex::encode(self.0.finalize())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_sha256_minusculo() {
        let good = format!("sha256:{}", "a".repeat(64));
        assert_eq!(good.parse::<Digest>().unwrap().hex(), "a".repeat(64));
        for bad in [
            "sha256:".to_string(),
            format!("sha256:{}", "A".repeat(64)),
            format!("sha512:{}", "a".repeat(64)),
            format!("sha256:{}/../x", "a".repeat(60)),
            "a".repeat(64),
        ] {
            assert!(bad.parse::<Digest>().is_err(), "aceitou {bad}");
        }
    }

    #[test]
    fn incremental_bate_com_o_inteiro() {
        let mut hasher = Hasher::new();
        hasher.update(b"hello ");
        hasher.update(b"world");
        assert_eq!(hasher.finish(), Digest::of(b"hello world"));
        assert_eq!(
            Digest::of(b"").as_str(),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
