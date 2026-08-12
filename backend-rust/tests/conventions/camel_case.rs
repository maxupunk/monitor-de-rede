//! Guarda da convenção §5.1: **todo** struct que atravessa a fronteira HTTP
//! leva `#[serde(rename_all = "camelCase")]`.
//!
//! Por que um teste que lê o código-fonte em vez de um teste de runtime: o
//! esquecimento acontece na declaração de um DTO novo, e não há como enumerar
//! em runtime tipos que ninguém instanciou. Um `grep` estruturado pega o erro
//! no momento em que o DTO nasce — que é quando custa barato — em vez de na
//! tela, quando o Vue lê `undefined` de `perPage`.
//!
//! Escopo: `src/dtos/`, `src/views/` e os DTOs de paginação em
//! `src/services/shared/pagination.rs`. Structs de request/response do Loco que
//! o frontend não consome (fixtures, seeds) ficam de fora por não serem
//! contrato.

use std::path::{Path, PathBuf};

const CAMEL_CASE_ATTR: &str = r#"rename_all = "camelCase""#;

/// Arquivos que definem contrato HTTP.
fn arquivos_de_contrato() -> Vec<PathBuf> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut arquivos = Vec::new();
    for dir in ["src/dtos", "src/views"] {
        colete_rs(&raiz.join(dir), &mut arquivos);
    }
    arquivos.push(raiz.join("src/services/shared/pagination.rs"));
    arquivos
}

fn colete_rs(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir() {
            colete_rs(&caminho, saida);
        } else if caminho.extension().is_some_and(|e| e == "rs") {
            saida.push(caminho);
        }
    }
}

/// Um struct declarado no fonte, com os atributos que o precedem.
struct StructDeclarado {
    nome: String,
    arquivo: String,
    linha: usize,
    atributos: Vec<String>,
}

/// Varre um arquivo juntando cada `struct` aos atributos imediatamente acima.
///
/// O parser é deliberadamente burro (linha a linha): a convenção só precisa
/// enxergar `#[derive(...)]` e `#[serde(...)]`, que por estilo do projeto ficam
/// coladas na declaração. Um `rustfmt` já garante esse formato.
fn structs_de(caminho: &Path) -> Vec<StructDeclarado> {
    let conteudo = std::fs::read_to_string(caminho).expect("ler arquivo de contrato");
    let arquivo = caminho
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut encontrados = Vec::new();
    let mut atributos: Vec<String> = Vec::new();
    let mut dentro_de_teste = false;

    for (indice, linha_bruta) in conteudo.lines().enumerate() {
        let linha = linha_bruta.trim();

        // Structs auxiliares de `mod tests` não são contrato.
        if linha.starts_with("mod tests") || linha.starts_with("#[cfg(test)]") {
            dentro_de_teste = true;
        }
        if dentro_de_teste {
            continue;
        }

        if linha.starts_with("#[") {
            atributos.push(linha.to_string());
            continue;
        }

        if let Some(resto) = linha
            .strip_prefix("pub struct ")
            .or_else(|| linha.strip_prefix("struct "))
        {
            let nome = resto
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .unwrap_or_default()
                .to_string();
            encontrados.push(StructDeclarado {
                nome,
                arquivo: arquivo.clone(),
                linha: indice + 1,
                atributos: std::mem::take(&mut atributos),
            });
            continue;
        }

        // Qualquer outra linha quebra a sequência de atributos.
        if !linha.is_empty() && !linha.starts_with("///") && !linha.starts_with("//") {
            atributos.clear();
        }
    }

    encontrados
}

fn e_serializavel(s: &StructDeclarado) -> bool {
    s.atributos.iter().any(|attr| {
        attr.contains("derive") && (attr.contains("Serialize") || attr.contains("Deserialize"))
    })
}

fn tem_camel_case(s: &StructDeclarado) -> bool {
    s.atributos
        .iter()
        .any(|attr| attr.contains(CAMEL_CASE_ATTR))
}

/// Exceções deliberadas: `POST /api/topology/links` e
/// `GET /api/topology?site_id=` recebem snake_case porque é assim que o
/// frontend envia hoje. Trocar para camelCase quebraria a tela sem aviso.
/// Qualquer struct acrescentado aqui precisa vir com a justificativa escrita.
const EXCECOES_SNAKE_CASE: &[&str] = &["TopologyLinkRequest", "TopologyQuery"];

#[test]
fn todo_dto_serializavel_declara_camel_case() {
    let mut faltando = Vec::new();
    let mut inspecionados = 0;

    for arquivo in arquivos_de_contrato() {
        for s in structs_de(&arquivo) {
            if !e_serializavel(&s) || EXCECOES_SNAKE_CASE.contains(&s.nome.as_str()) {
                continue;
            }
            inspecionados += 1;
            if !tem_camel_case(&s) {
                faltando.push(format!("{}:{} — {}", s.arquivo, s.linha, s.nome));
            }
        }
    }

    assert!(
        inspecionados > 0,
        "nenhum DTO foi inspecionado — a varredura quebrou e o teste virou decoração"
    );
    assert!(
        faltando.is_empty(),
        "DTO(s) sem `#[serde(rename_all = \"camelCase\")]` (§5.1). O frontend leria \
         `undefined` nesses campos:\n  {}",
        faltando.join("\n  ")
    );
}

#[test]
fn a_varredura_realmente_detecta_a_ausencia() {
    // Sem este teste, um bug no parser faria o anterior passar sempre.
    let temp = std::env::temp_dir().join("netmonitor_convencao_camel_case.rs");
    std::fs::write(
        &temp,
        "#[derive(Serialize)]\npub struct Esquecido {\n    pub per_page: u64,\n}\n",
    )
    .unwrap();

    let structs = structs_de(&temp);
    std::fs::remove_file(&temp).ok();

    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].nome, "Esquecido");
    assert!(e_serializavel(&structs[0]));
    assert!(!tem_camel_case(&structs[0]));
}
