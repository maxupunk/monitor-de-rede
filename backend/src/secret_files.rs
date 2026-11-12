//! Segredos lidos de arquivo, na convenção `<VAR>_FILE` das imagens Docker.
//!
//! O compose não põe segredo no ambiente: o serviço `netmonitor-init` os
//! sorteia no primeiro boot (ou grava os que o `.env` trouxer) no volume
//! `netmonitor-secrets`, e a API recebe só o caminho — `JWT_SECRET_FILE`,
//! `ENCRYPTION_KEY_FILE`. Este módulo roda no começo do `main`, antes do
//! runtime e do Loco: o `production.yaml` e o `shared::crypto` continuam
//! lendo a variável de sempre, e um `docker compose exec netmonitor
//! backend-cli task ...` — que não passa pelo entrypoint — enxerga o mesmo
//! valor.
//!
//! A variável definida (não vazia) vence o arquivo. A senha do PostgreSQL não
//! passa por aqui: as URLs vão sem senha e o `sqlx` a lê do `PGPASSFILE`.

use std::path::Path;

/// As variáveis que aceitam `<VAR>_FILE`.
pub const SECRET_VARS: [&str; 2] = ["JWT_SECRET", "ENCRYPTION_KEY"];

/// O valor do segredo: a variável, se definida; senão o conteúdo do arquivo,
/// sem a quebra de linha do fim.
///
/// # Errors
///
/// O motivo, quando há `_FILE` mas o arquivo não pode ser lido.
pub fn resolve(
    value: Option<String>,
    file: Option<String>,
    read: impl FnOnce(&Path) -> std::io::Result<String>,
) -> Result<Option<String>, String> {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        return Ok(Some(value));
    }
    let Some(file) = file.filter(|file| !file.trim().is_empty()) else {
        return Ok(None);
    };
    let content = read(Path::new(&file)).map_err(|error| format!("{file}: {error}"))?;
    let content = content.trim_end_matches(['\r', '\n']).to_string();
    Ok((!content.is_empty()).then_some(content))
}

/// Carrega no ambiente os segredos de [`SECRET_VARS`] que vierem por arquivo.
/// Chamar antes de criar qualquer thread.
///
/// # Panics
///
/// Quando `<VAR>_FILE` aponta para um arquivo ilegível: subir sem o segredo
/// pedido só trocaria um erro claro por um 401 ou um dado ilegível adiante.
pub fn load() {
    for var in SECRET_VARS {
        let resolved = resolve(
            std::env::var(var).ok(),
            std::env::var(format!("{var}_FILE")).ok(),
            |path| std::fs::read_to_string(path),
        )
        .unwrap_or_else(|reason| panic!("{var}_FILE ilegível — {reason}"));
        if let Some(value) = resolved {
            std::env::set_var(var, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn never(_: &Path) -> std::io::Result<String> {
        panic!("não deveria ler o arquivo")
    }

    #[test]
    fn variavel_definida_vence_o_arquivo() {
        let value = resolve(Some("env".into()), Some("/x".into()), never);
        assert_eq!(value, Ok(Some("env".into())));
    }

    #[test]
    fn variavel_vazia_cai_no_arquivo_sem_a_quebra_de_linha() {
        let value = resolve(Some(" ".into()), Some("/s".into()), |_| Ok("abc=\n".into()));
        assert_eq!(value, Ok(Some("abc=".into())));
    }

    #[test]
    fn sem_variavel_nem_arquivo_nao_ha_segredo() {
        assert_eq!(resolve(None, None, never), Ok(None));
        assert_eq!(resolve(None, Some(String::new()), never), Ok(None));
    }

    #[test]
    fn arquivo_ilegivel_e_erro() {
        let value = resolve(None, Some("/nao/existe".into()), |path| {
            std::fs::read_to_string(path)
        });
        assert!(value.expect_err("ilegível").starts_with("/nao/existe"));
    }
}
