//! Transação de escrita sempre por `shared::transaction::begin_write`.
//!
//! O `BEGIN` deferido do SQLite falha com `database is locked` quando outra
//! conexão escreve entre a leitura e a escrita da transação — era o que fazia
//! um monitor perder o resultado a cada minuto. Um `.begin()` novo em código de
//! produção traria o problema de volta sem nenhum teste de unidade perceber.

use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("diretório do código") {
        let path = entry.expect("entrada do diretório").path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn codigo_de_producao_nao_abre_transacao_deferida() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let offenders: Vec<String> = files
        .iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(path).ok()?;
            let production = text.split("#[cfg(test)]").next().unwrap_or_default();
            (production.contains(".begin().await") || production.contains(".begin()\n")).then(
                || {
                    path.strip_prefix(&src)
                        .unwrap_or(path)
                        .display()
                        .to_string()
                },
            )
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "use shared::transaction::begin_write no lugar de .begin(): {offenders:?}"
    );
}
