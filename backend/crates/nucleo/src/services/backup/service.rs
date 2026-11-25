//! Backup e restauração das **configurações** do sistema.
//!
//! O que entra no arquivo é o grafo de configuração — o que um operador
//! cadastrou e não quer digitar de novo: sites, probes, redes, dispositivos e
//! suas interfaces/enlaces, monitores, regras de alerta, servidores DNS, o
//! servidor VPN com seus peers e o `system_settings`.
//!
//! O que **não** entra, de propósito:
//!
//! * **Telemetria** (`monitor_results`, `metrics`, `alert_events`,
//!   `discovery_runs`, `discovery_results`, `event_outbox`, `probe_tasks`).
//!   É histórico, cresce em milhões de linhas e volta a ser produzido sozinho
//!   no ciclo seguinte. Um backup de configuração que carregasse isso junto
//!   deixaria de caber em um arquivo.
//! * **`users`.** Conta de acesso não é configuração. Restaurar usuários
//!   trocaria as credenciais de quem está logado no meio da operação — e
//!   deixaria o operador para fora do próprio sistema se o backup viesse de
//!   outra instalação.
//!
//! **Os ids são preservados.** Não é detalhe de implementação: `monitors`,
//! `alert_rules`, `device_links` e `vpn_peers` guardam FKs, e remapear os ids
//! na restauração significaria reescrever cada referência — inclusive as que
//! vivem dentro de JSON (`monitors.configuration`). Manter o id é o que torna
//! a restauração fiel. Em PostgreSQL isso exige realinhar a sequência de cada
//! tabela depois da carga, senão o próximo `INSERT` colide com um id já usado;
//! ver [`realign_sequences`].
//!
//! **Segredos.** `vpn_servers.private_key_encrypted` e
//! `vpn_peers.preshared_key_encrypted` viajam como estão no banco: cifrados
//! com a `ENCRYPTION_KEY` da instalação. Um backup restaurado em outra
//! instalação só devolve VPN funcional se a mesma `ENCRYPTION_KEY` estiver lá.
//! O arquivo também carrega `probes.token_hash` e `devices.snmp_community`,
//! então é material sensível — quem o guarda, guarda o acesso.

use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait};
use serde::Serialize;

use super::{
    archive::begin_snapshot,
    catalog::{BackupScope, Store},
    restore::{Loader, RestoreGuard},
};
use crate::{
    models::_entities::{
        alert_rules, device_interfaces, device_links, devices, dns_servers, monitors, networks,
        probes, sites, system_settings, vpn_peers, vpn_servers,
    },
    services::shared::errors::{AppError, AppResult},
};

pub use super::restore::TableCounts;

/// Versão do formato do arquivo.
///
/// Só muda quando a estrutura do envelope muda — acrescentar tabela ou coluna
/// não conta, porque a leitura é por nome. Um arquivo de versão desconhecida é
/// recusado na entrada, e não no meio da transação.
pub const FORMAT_VERSION: u32 = 1;

/// As tabelas do backup, na ordem de criação (pai antes de filho).
///
/// A ordem é a mesma de [`crate::models::tables::CREATION_ORDER`] e é o que permite inserir
/// sem violar FK. Um teste garante que nenhuma tabela aqui esteja fora daquela
/// lista nem fora de ordem.
pub const BACKED_UP_TABLES: [&str; 12] = [
    "sites",
    "probes",
    "networks",
    "devices",
    "device_interfaces",
    "device_links",
    "monitors",
    "alert_rules",
    "vpn_servers",
    "vpn_peers",
    "dns_servers",
    "system_settings",
];

/// Tabelas de histórico apagadas junto na restauração.
///
/// Elas não estão no arquivo, mas apontam para `devices`/`monitors`/`probes`,
/// que são substituídos. Deixá-las para trás produziria histórico pendurado em
/// id que passou a ser de outro equipamento — um gráfico de tráfego com os
/// dados do vizinho. As FKs `CASCADE` resolveriam isso no PostgreSQL, mas o
/// SQLite só as aplica com `foreign_keys=ON`; limpar à mão vale nos dois.
pub(super) const DEPENDENT_HISTORY: [&str; 19] = [
    // Teste de enlace: pernas, testes e agendamentos citam ids de
    // equipamentos e de enlaces que a restauração substitui.
    "link_test_legs",
    "link_tests",
    "link_test_schedules",
    // Tarefas agendadas: os argumentos citam ids de equipamentos, monitores
    // e alertas que a restauração substitui — rodar a de antes no
    // equipamento de outro id seria pior que recriá-la.
    "scheduled_task_runs",
    "scheduled_tasks",
    // Plugins de dispositivo: credenciais, execuções e aceites do modo
    // automático pertencem a um equipamento que a restauração substitui.
    "plugin_batches",
    "device_plugin_installs",
    "plugin_auto_accept",
    "plugin_runs",
    "device_credentials",
    "alert_events",
    "notification_outbox",
    "monitor_results_hourly",
    "monitor_results",
    "metrics",
    "discovery_results",
    "discovery_runs",
    "probe_tasks",
    "event_outbox",
];

/// Envelope do arquivo de backup.
///
/// `tables` é um mapa `nome da tabela → array de linhas`, cada linha no formato
/// em que a entidade do `sea-orm` serializa (`snake_case`, o mesmo do banco).
/// É deliberadamente cru: o arquivo é um despejo do estado, não um contrato de
/// tela, e qualquer renomeação estética aqui teria de ser desfeita na volta.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub format_version: u32,
    pub app_version: String,
    pub generated_at: String,
    /// Arquivo sem o campo é de antes do backup completo: configuração.
    #[serde(default)]
    pub scope: BackupScope,
    #[serde(serialize_with = "tables_in_creation_order")]
    pub tables: serde_json::Map<String, serde_json::Value>,
}

/// Escreve `tables` na ordem de [`BACKED_UP_TABLES`] (pai antes de filho), e
/// só depois o que não estiver nela.
///
/// O `serde_json::Map` sem `preserve_order` é um `BTreeMap`: serializado como
/// está, sai em ordem **alfabética** — `alert_rules` antes de `devices`,
/// `devices` antes de `sites` —, e a restauração em fluxo teria de segurar na
/// memória quase todas as tabelas até a primeira da ordem chegar. Ligar o
/// `preserve_order` no workspace trocaria o mapa de todo JSON do sistema por
/// um `IndexMap`; ordenar só aqui custa um laço.
fn tables_in_creation_order<S>(
    tables: &serde_json::Map<String, serde_json::Value>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;
    let mut map = serializer.serialize_map(Some(tables.len()))?;
    for table in BACKED_UP_TABLES {
        if let Some(rows) = tables.get(table) {
            map.serialize_entry(table, rows)?;
        }
    }
    for (table, rows) in tables {
        if !BACKED_UP_TABLES.contains(&table.as_str()) {
            map.serialize_entry(table, rows)?;
        }
    }
    map.end()
}

/// Monta o arquivo de backup a partir do banco.
///
/// # Errors
///
/// Propaga falha de leitura do banco ou de serialização.
pub async fn export(db: &DatabaseConnection, app_version: String) -> AppResult<BackupFile> {
    let txn = begin_snapshot(db).await?;
    let mut tables = serde_json::Map::new();
    dump::<sites::Entity, _>(&txn, &mut tables, "sites").await?;
    dump::<probes::Entity, _>(&txn, &mut tables, "probes").await?;
    dump::<networks::Entity, _>(&txn, &mut tables, "networks").await?;
    dump::<devices::Entity, _>(&txn, &mut tables, "devices").await?;
    dump::<device_interfaces::Entity, _>(&txn, &mut tables, "device_interfaces").await?;
    dump::<device_links::Entity, _>(&txn, &mut tables, "device_links").await?;
    dump::<monitors::Entity, _>(&txn, &mut tables, "monitors").await?;
    dump::<alert_rules::Entity, _>(&txn, &mut tables, "alert_rules").await?;
    dump::<vpn_servers::Entity, _>(&txn, &mut tables, "vpn_servers").await?;
    dump::<vpn_peers::Entity, _>(&txn, &mut tables, "vpn_peers").await?;
    dump::<dns_servers::Entity, _>(&txn, &mut tables, "dns_servers").await?;
    dump::<system_settings::Entity, _>(&txn, &mut tables, "system_settings").await?;
    txn.rollback().await?;

    Ok(BackupFile {
        format_version: FORMAT_VERSION,
        app_version,
        generated_at: chrono::Utc::now().to_rfc3339(),
        scope: BackupScope::Config,
        tables,
    })
}

/// Contagem de linhas por tabela de um arquivo já carregado, sem tocar no banco.
///
/// É o que alimenta a pré-visualização: o operador vê o que vai entrar antes de
/// mandar apagar o que está lá.
///
/// # Errors
///
/// Falha quando a versão do formato é desconhecida ou alguma entrada de
/// `tables` não é um array.
pub fn inspect(file: &BackupFile) -> AppResult<TableCounts> {
    validate_version(file)?;
    let mut counts = Vec::new();
    for table in BACKED_UP_TABLES {
        let rows = match file.tables.get(table) {
            Some(value) => value.as_array().ok_or_else(|| {
                AppError::validation(format!("A tabela '{table}' do backup não é uma lista"))
            })?,
            // Tabela ausente é backup mais antigo, não arquivo corrompido: a
            // restauração simplesmente a deixa vazia.
            None => continue,
        };
        counts.push((table.to_string(), rows.len()));
    }
    Ok(counts)
}

/// Substitui a configuração atual pela do arquivo.
///
/// Tudo acontece em **uma transação**: ou o sistema fica inteiro com a
/// configuração do backup, ou continua exatamente como estava. Uma restauração
/// que falhasse no meio deixaria o monitoramento com metade dos dispositivos —
/// pior do que não ter restaurado.
///
/// # Errors
///
/// Falha quando o formato é desconhecido, quando alguma linha do arquivo não
/// casa com o esquema atual, ou em qualquer erro do banco. Em todos os casos o
/// banco fica intocado.
pub async fn restore(db: &DatabaseConnection, file: &BackupFile) -> AppResult<TableCounts> {
    let guard = RestoreGuard::acquire()?;
    validate_version(file)?;
    // Falha cedo, fora da transação: um arquivo ilegível não merece o custo de
    // apagar tudo primeiro para só então descobrir que não dava.
    inspect(file)?;

    let mut loader = Loader::begin(db, None, BackupScope::Config).await?;
    for table in BACKED_UP_TABLES {
        let Some(value) = file.tables.get(table) else {
            continue;
        };
        let rows = value.as_array().cloned().unwrap_or_default();
        loader.rows(Store::Main, table, rows).await?;
        loader.table_end(Store::Main, table).await?;
    }
    Ok(loader.finish(db, None, None, guard).await?.counts)
}

fn validate_version(file: &BackupFile) -> AppResult<()> {
    if file.format_version == FORMAT_VERSION {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "Arquivo de backup na versão {} — este sistema lê a versão {FORMAT_VERSION}",
            file.format_version
        )))
    }
}

async fn dump<E, C>(
    db: &C,
    out: &mut serde_json::Map<String, serde_json::Value>,
    table: &str,
) -> AppResult<()>
where
    E: EntityTrait,
    E::Model: Serialize,
    C: ConnectionTrait,
{
    let rows = E::find().all(db).await?;
    let value = serde_json::to_value(rows)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("serializar '{table}': {err}")))?;
    out.insert(table.to_string(), value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::tables;
    use std::collections::HashSet;

    #[test]
    fn so_faz_backup_de_tabela_que_existe_no_esquema() {
        let esquema: HashSet<&str> = tables::CREATION_ORDER.iter().copied().collect();
        for table in BACKED_UP_TABLES {
            assert!(esquema.contains(table), "tabela fora do esquema: {table}");
        }
        for table in DEPENDENT_HISTORY {
            assert!(esquema.contains(table), "tabela fora do esquema: {table}");
        }
    }

    #[test]
    fn a_ordem_do_backup_segue_a_ordem_de_criacao() {
        let posicao = |nome: &str| {
            tables::CREATION_ORDER
                .iter()
                .position(|t| *t == nome)
                .expect("tabela no esquema")
        };
        for par in BACKED_UP_TABLES.windows(2) {
            assert!(
                posicao(par[0]) < posicao(par[1]),
                "'{}' precisa vir antes de '{}' para as FKs fecharem",
                par[0],
                par[1]
            );
        }
    }

    #[test]
    fn historico_e_configuracao_nao_se_sobrepoem() {
        let backup: HashSet<&str> = BACKED_UP_TABLES.iter().copied().collect();
        for table in DEPENDENT_HISTORY {
            assert!(
                !backup.contains(table),
                "'{table}' seria apagada e reinserida ao mesmo tempo"
            );
        }
    }

    #[test]
    fn o_arquivo_sai_com_as_tabelas_na_ordem_de_criacao() {
        let mut tables = serde_json::Map::new();
        // Inseridas fora de ordem; o mapa as guarda em ordem alfabética.
        for table in BACKED_UP_TABLES.iter().rev() {
            tables.insert((*table).to_owned(), serde_json::json!([]));
        }
        tables.insert("tabela_do_futuro".into(), serde_json::json!([]));
        let arquivo = BackupFile {
            format_version: FORMAT_VERSION,
            app_version: "teste".into(),
            generated_at: "2026-08-12T00:00:00Z".into(),
            scope: BackupScope::Config,
            tables,
        };
        let texto = serde_json::to_string(&arquivo).unwrap();
        let posicoes: Vec<usize> = BACKED_UP_TABLES
            .iter()
            .chain(std::iter::once(&"tabela_do_futuro"))
            .map(|table| {
                texto
                    .find(&format!("\"{table}\":"))
                    .expect("tabela no arquivo")
            })
            .collect();
        assert!(
            posicoes.windows(2).all(|par| par[0] < par[1]),
            "fora da ordem de criação: {texto}"
        );
        // E a volta lê o mesmo conteúdo.
        let lido: BackupFile = serde_json::from_str(&texto).unwrap();
        assert_eq!(lido.tables.len(), BACKED_UP_TABLES.len() + 1);
    }

    #[test]
    fn usuarios_ficam_de_fora() {
        assert!(!BACKED_UP_TABLES.contains(&"users"));
        assert!(!DEPENDENT_HISTORY.contains(&"users"));
    }

    #[test]
    fn versao_diferente_e_recusada_antes_de_tocar_no_banco() {
        let arquivo = BackupFile {
            format_version: FORMAT_VERSION + 1,
            app_version: "teste".into(),
            generated_at: "2026-08-12T00:00:00Z".into(),
            scope: BackupScope::Config,
            tables: serde_json::Map::new(),
        };
        let erro = validate_version(&arquivo).unwrap_err();
        assert_eq!(erro.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn inspect_conta_as_linhas_e_tolera_tabela_ausente() {
        let mut tables = serde_json::Map::new();
        tables.insert(
            "sites".into(),
            serde_json::json!([{ "id": 1 }, { "id": 2 }]),
        );
        let arquivo = BackupFile {
            format_version: FORMAT_VERSION,
            app_version: "teste".into(),
            generated_at: "2026-08-12T00:00:00Z".into(),
            scope: BackupScope::Config,
            tables,
        };
        let contagem = inspect(&arquivo).unwrap();
        assert_eq!(contagem, vec![("sites".to_string(), 2)]);
    }

    #[test]
    fn inspect_recusa_tabela_que_nao_e_lista() {
        let mut tables = serde_json::Map::new();
        tables.insert("sites".into(), serde_json::json!({ "id": 1 }));
        let arquivo = BackupFile {
            format_version: FORMAT_VERSION,
            app_version: "teste".into(),
            generated_at: "2026-08-12T00:00:00Z".into(),
            scope: BackupScope::Config,
            tables,
        };
        assert!(inspect(&arquivo).is_err());
    }
}
