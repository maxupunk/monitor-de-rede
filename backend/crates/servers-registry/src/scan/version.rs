//! Comparação de versões pela regra de cada ecossistema (RF-25).
//!
//! Comparar `1.10` com `1.9` como texto erra; comparar `1:2.3-1` (Debian)
//! como semver também. Cada família tem o algoritmo dela: `dpkg` (Debian,
//! Ubuntu), `apk` (Alpine) e a comparação por segmentos numéricos que serve a
//! semver, PEP 440 e Maven no caso comum (o que sobra cai no mesmo caminho,
//! sem nunca entrar em pânico).

use std::cmp::Ordering;

/// A família de comparação de um ecossistema OSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Dpkg,
    Apk,
    Generic,
}

impl Scheme {
    #[must_use]
    pub fn of(ecosystem: &str) -> Self {
        let base = ecosystem.split(':').next().unwrap_or(ecosystem);
        match base {
            "Debian" | "Ubuntu" => Self::Dpkg,
            "Alpine" => Self::Apk,
            _ => Self::Generic,
        }
    }

    #[must_use]
    pub fn compare(self, a: &str, b: &str) -> Ordering {
        match self {
            Self::Dpkg => dpkg(a, b),
            Self::Apk => apk(a, b),
            Self::Generic => generic(a, b),
        }
    }
}

// ── dpkg ─────────────────────────────────────────────────────────────────

fn split_dpkg(version: &str) -> (u64, &str, &str) {
    let (epoch, rest) = match version.split_once(':') {
        Some((epoch, rest)) if epoch.chars().all(|c| c.is_ascii_digit()) => {
            (epoch.parse().unwrap_or(0), rest)
        }
        _ => (0, version),
    };
    match rest.rsplit_once('-') {
        Some((upstream, revision)) => (epoch, upstream, revision),
        None => (epoch, rest, ""),
    }
}

/// Ordem de um caractere no algoritmo do dpkg: `~` antes de tudo (até do
/// fim), letras antes dos outros símbolos.
fn dpkg_order(c: Option<char>) -> i32 {
    match c {
        Some('~') => -1,
        None => 0,
        Some(c) if c.is_ascii_digit() => 0,
        Some(c) if c.is_ascii_alphabetic() => c as i32,
        Some(c) => c as i32 + 256,
    }
}

fn dpkg_part(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        // Parte não numérica.
        loop {
            let (ca, cb) = (a.peek().copied(), b.peek().copied());
            let a_digit = ca.is_none_or(|c| c.is_ascii_digit());
            let b_digit = cb.is_none_or(|c| c.is_ascii_digit());
            if a_digit && b_digit {
                break;
            }
            let (oa, ob) = (
                if a_digit { 0 } else { dpkg_order(ca) },
                if b_digit { 0 } else { dpkg_order(cb) },
            );
            if oa != ob {
                return oa.cmp(&ob);
            }
            if !a_digit {
                a.next();
            }
            if !b_digit {
                b.next();
            }
        }
        // Parte numérica.
        let take = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
            let mut number = String::new();
            while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                number.push(c);
                it.next();
            }
            number
        };
        let (na, nb) = (take(&mut a), take(&mut b));
        let ordering = numeric(&na, &nb);
        if ordering != Ordering::Equal {
            return ordering;
        }
        if a.peek().is_none() && b.peek().is_none() {
            return Ordering::Equal;
        }
    }
}

/// Compara números em texto de qualquer tamanho.
fn numeric(a: &str, b: &str) -> Ordering {
    let a = a.trim_start_matches('0');
    let b = b.trim_start_matches('0');
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn dpkg(a: &str, b: &str) -> Ordering {
    let (ea, ua, ra) = split_dpkg(a);
    let (eb, ub, rb) = split_dpkg(b);
    ea.cmp(&eb)
        .then_with(|| dpkg_part(ua, ub))
        .then_with(|| dpkg_part(ra, rb))
}

// ── apk ──────────────────────────────────────────────────────────────────

fn apk(a: &str, b: &str) -> Ordering {
    // `1.2.3_rc1-r4`: o `-rN` é a revisão do pacote.
    let split = |v: &str| -> (String, u64) {
        match v.rsplit_once("-r") {
            Some((base, rev)) if rev.chars().all(|c| c.is_ascii_digit()) => {
                (base.to_string(), rev.parse().unwrap_or(0))
            }
            _ => (v.to_string(), 0),
        }
    };
    let (ba, ra) = split(a);
    let (bb, rb) = split(b);
    // Sufixos de pré-lançamento vêm antes do lançamento.
    let weight = |s: &str| -> (String, i32) {
        for (suffix, rank) in [("_alpha", -4), ("_beta", -3), ("_pre", -2), ("_rc", -1)] {
            if let Some((main, _)) = s.split_once(suffix) {
                return (main.to_string(), rank);
            }
        }
        (s.to_string(), 0)
    };
    let (ma, wa) = weight(&ba);
    let (mb, wb) = weight(&bb);
    generic(&ma, &mb)
        .then_with(|| wa.cmp(&wb))
        .then_with(|| generic(&ba, &bb))
        .then_with(|| ra.cmp(&rb))
}

// ── genérica (semver, PEP 440, Maven, Go) ────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Number(String),
    Text(String),
}

fn tokens(version: &str) -> (Vec<Token>, bool) {
    let version = version.trim().trim_start_matches(['v', 'V']);
    // Pré-lançamento: `-alpha`, `rc1`, `.dev0` contam antes da versão final.
    let (main, pre) = match version.find(['-', '+']) {
        Some(at) if version[at..].starts_with('+') => (&version[..at], false),
        Some(at) => (&version[..at], true),
        None => (version, false),
    };
    let mut out = Vec::new();
    let mut current = String::new();
    let mut digit = None;
    let flush = |current: &mut String, digit: Option<bool>, out: &mut Vec<Token>| {
        if current.is_empty() {
            return;
        }
        let text = std::mem::take(current);
        out.push(if digit == Some(true) {
            Token::Number(text)
        } else {
            Token::Text(text.to_ascii_lowercase())
        });
    };
    let mut pre_text = pre;
    for c in main.chars() {
        if c == '.' || c == '_' {
            flush(&mut current, digit, &mut out);
            digit = None;
            continue;
        }
        let is_digit = c.is_ascii_digit();
        if digit.is_some_and(|d| d != is_digit) {
            flush(&mut current, digit, &mut out);
        }
        if !is_digit {
            pre_text = true;
        }
        digit = Some(is_digit);
        current.push(c);
    }
    flush(&mut current, digit, &mut out);
    (out, pre_text)
}

fn generic(a: &str, b: &str) -> Ordering {
    let (ta, pa) = tokens(a);
    let (tb, pb) = tokens(b);
    for index in 0..ta.len().max(tb.len()) {
        let ordering = match (ta.get(index), tb.get(index)) {
            (Some(Token::Number(x)), Some(Token::Number(y))) => numeric(x, y),
            (Some(Token::Text(x)), Some(Token::Text(y))) => x.cmp(y),
            // Número depois de texto: `1.0.1` > `1.0.rc1`.
            (Some(Token::Number(_)), Some(Token::Text(_))) => Ordering::Greater,
            (Some(Token::Text(_)), Some(Token::Number(_))) => Ordering::Less,
            // `1.0` == `1.0.0`; `1.0rc1` < `1.0`.
            (Some(Token::Number(x)), None) if x.trim_start_matches('0').is_empty() => {
                Ordering::Equal
            }
            (None, Some(Token::Number(y))) if y.trim_start_matches('0').is_empty() => {
                Ordering::Equal
            }
            (Some(Token::Number(_)), None) => Ordering::Greater,
            (None, Some(Token::Number(_))) => Ordering::Less,
            (Some(Token::Text(_)), None) => Ordering::Less,
            (None, Some(Token::Text(_))) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    // Mesma base: o pré-lançamento vem antes.
    match (pa, pb) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpkg_como_o_debian() {
        let lt = |a: &str, b: &str| assert_eq!(dpkg(a, b), Ordering::Less, "{a} < {b}");
        lt("1.0~rc1", "1.0");
        lt("1.0", "1.0-1");
        lt("2.36-9+deb12u3", "2.36-9+deb12u4");
        lt("3.0.11-1~deb12u1", "3.0.11-1~deb12u2");
        lt("1:1.0", "2:0.1");
        lt("1.9", "1.10");
        assert_eq!(dpkg("1.0-1", "1.0-1"), Ordering::Equal);
        assert_eq!(dpkg("0:1.0", "1.0"), Ordering::Equal);
    }

    #[test]
    fn apk_como_o_alpine() {
        let lt = |a: &str, b: &str| assert_eq!(apk(a, b), Ordering::Less, "{a} < {b}");
        lt("3.1.4-r0", "3.1.4-r1");
        lt("1.2.4_rc1-r0", "1.2.4-r0");
        lt("1.2.9-r0", "1.2.10-r0");
    }

    #[test]
    fn generica_para_semver_pep440_e_go() {
        let lt = |a: &str, b: &str| assert_eq!(generic(a, b), Ordering::Less, "{a} < {b}");
        lt("1.9.0", "1.10.0");
        lt("1.0.0-alpha", "1.0.0");
        lt("v0.17.0", "v0.18.1");
        lt("4.2rc1", "4.2");
        lt("2.0", "2.0.1");
        assert_eq!(generic("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(generic("v1.2.3", "1.2.3"), Ordering::Equal);
        assert_eq!(generic("1.2.3+build5", "1.2.3"), Ordering::Equal);
    }

    #[test]
    fn esquema_pelo_ecossistema() {
        assert_eq!(Scheme::of("Debian:12"), Scheme::Dpkg);
        assert_eq!(Scheme::of("Alpine:v3.19"), Scheme::Apk);
        assert_eq!(Scheme::of("npm"), Scheme::Generic);
    }
}
