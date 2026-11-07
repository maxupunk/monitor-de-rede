//! Comparação de versão de firmware.
//!
//! Firmware não segue semver: `23.05.2`, `21.02.0-rc4`, `7.14.1`, `v6.49.10`,
//! `2.0.9-hotfix`. A comparação é numérica por segmento e ignora o sufixo —
//! `21.02.0-rc4` conta como `21.02.0`. Para compatibilidade isso basta: quem
//! precisa distinguir um release candidate declara a versão exata.

use std::cmp::Ordering;

/// Os segmentos numéricos da versão (`"v23.05.2-rc1"` → `[23, 5, 2]`).
#[must_use]
pub fn segments(version: &str) -> Vec<u64> {
    let trimmed = version.trim().trim_start_matches(['v', 'V']);
    let core = trimmed
        .split(['-', '+', ' ', '_'])
        .next()
        .unwrap_or_default();
    core.split('.')
        .map_while(|part| {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u64>().ok()
        })
        .collect()
}

/// Compara duas versões, completando com zeros (`21.02` == `21.02.0`).
#[must_use]
pub fn compare(a: &str, b: &str) -> Ordering {
    let (a, b) = (segments(a), segments(b));
    let len = a.len().max(b.len());
    for index in 0..len {
        let (x, y) = (
            a.get(index).copied().unwrap_or(0),
            b.get(index).copied().unwrap_or(0),
        );
        match x.cmp(&y) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

/// Verifica `version` contra uma restrição como `">=21.02, <24"`.
///
/// Termos separados por vírgula valem juntos (E). Operadores: `>=`, `>`,
/// `<=`, `<`, `=`/`==` e prefixo com `*` (`23.*`). Sem operador é igualdade.
///
/// # Errors
///
/// Restrição vazia ou termo sem versão.
pub fn satisfies(version: &str, constraint: &str) -> Result<bool, String> {
    let terms: Vec<&str> = constraint
        .split(',')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .collect();
    if terms.is_empty() {
        return Err("restrição de versão vazia".into());
    }
    for term in terms {
        if !term_matches(version, term)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn term_matches(version: &str, term: &str) -> Result<bool, String> {
    let (op, wanted) = ["<=", ">=", "==", "<", ">", "="]
        .iter()
        .find_map(|op| term.strip_prefix(op).map(|rest| (*op, rest.trim())))
        .unwrap_or(("=", term));
    if wanted.is_empty() || segments(wanted.trim_end_matches(".*").trim_end_matches('*')).is_empty()
    {
        return Err(format!("termo de versão inválido: `{term}`"));
    }
    if let Some(prefix) = wanted
        .strip_suffix(".*")
        .or_else(|| wanted.strip_suffix('*'))
    {
        let wanted = segments(prefix);
        let actual = segments(version);
        return Ok(actual.len() >= wanted.len() && actual[..wanted.len()] == wanted[..]);
    }
    let ordering = compare(version, wanted);
    Ok(match op {
        ">=" => ordering != Ordering::Less,
        ">" => ordering == Ordering::Greater,
        "<=" => ordering != Ordering::Greater,
        "<" => ordering == Ordering::Less,
        _ => ordering == Ordering::Equal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmentos_ignoram_prefixo_e_sufixo() {
        assert_eq!(segments("v23.05.2-rc1"), vec![23, 5, 2]);
        assert_eq!(segments("7.14"), vec![7, 14]);
        assert!(segments("desconhecida").is_empty());
    }

    #[test]
    fn comparacao_completa_com_zeros() {
        assert_eq!(compare("21.02", "21.02.0"), Ordering::Equal);
        assert_eq!(compare("23.05.2", "21.02.7"), Ordering::Greater);
        assert_eq!(compare("6.49.10", "7.1"), Ordering::Less);
    }

    #[test]
    fn restricoes_compostas() {
        assert!(satisfies("23.05.2", ">=21.02, <24").unwrap());
        assert!(!satisfies("24.10.0", ">=21.02, <24").unwrap());
        assert!(satisfies("23.05.2", "23.*").unwrap());
        assert!(!satisfies("22.03.5", "23.*").unwrap());
        assert!(satisfies("7.14.1", "7.14.1").unwrap());
        assert!(satisfies("21.02.0-rc4", ">=21.02").unwrap());
    }

    #[test]
    fn restricao_invalida_e_erro() {
        assert!(satisfies("1.0", "").is_err());
        assert!(satisfies("1.0", ">=abc").is_err());
    }
}
