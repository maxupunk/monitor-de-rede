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

use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait,
    IntoActiveModel, Statement, TransactionTrait,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    models::{
        _entities::{
            alert_rules, device_interfaces, device_links, devices, dns_servers, monitors, networks,
            probes, sites, system_settings, vpn_peers, vpn_servers,
        },
        tables,
    },
    services::shared::errors::{AppError, AppResult},
};

/// Versão do formato do arquivo.
///
/// Só muda quando a estrutura do envelope muda — acrescentar tabela ou coluna
/// não conta, porque a leitura é por nome. Um arquivo de versão desconhecida é
/// recusado na entrada, e não no meio da transação.
pub const FORMAT_VERSION: u32 = 1;

/// As tabelas do backup, na ordem de criação (pai antes de filho).
///
/// A ordem é a mesma de [`tables::CREATION_ORDER`] e é o que permite inserir
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
const DEPENDENT_HISTORY: [&str; 7] = [
    "alert_events",
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
    pub tables: serde_json::Map<String, serde_json::Value>,
}

/// Quantas linhas cada tabela tinha (na exportação) ou recebeu (na restauração).
pub type TableCounts = Vec<(String, usize)>;

/// Monta o arquivo de backup a partir do banco.
///
/// # Errors
///
/// Propaga falha de leitura do banco ou de serialização.
pub async fn export(db: &DatabaseConnection, app_version: String) -> AppResult<BackupFile> {
    let mut tables = serde_json::Map::new();
    dump::<sites::Entity>(db, &mut tables, "sites").await?;
    dump::<probes::Entity>(db, &mut tables, "probes").await?;
    dump::<networks::Entity>(db, &mut tables, "networks").await?;
    dump::<devices::Entity>(db, &mut tables, "devices").await?;
    dump::<device_interfaces::Entity>(db, &mut tables, "device_interfaces").await?;
    dump::<device_links::Entity>(db, &mut tables, "device_links").await?;
    dump::<monitors::Entity>(db, &mut tables, "monitors").await?;
    dump::<alert_rules::Entity>(db, &mut tables, "alert_rules").await?;
    dump::<vpn_servers::Entity>(db, &mut tables, "vpn_servers").await?;
    dump::<vpn_peers::Entity>(db, &mut tables, "vpn_peers").await?;
    dump::<dns_servers::Entity>(db, &mut tables, "dns_servers").await?;
    dump::<system_settings::Entity>(db, &mut tables, "system_settings").await?;

    Ok(BackupFile {
        format_version: FORMAT_VERSION,
        app_version,
        generated_at: chrono::Utc::now().to_rfc3339(),
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
    validate_version(file)?;
    // Falha cedo, fora da transação: um arquivo ilegível não merece o custo de
    // apagar tudo primeiro para só então descobrir que não dava.
    inspect(file)?;

    let txn = db.begin().await?;
    wipe(&txn).await?;

    let mut counts = Vec::new();
    load::<sites::Entity>(&txn, file, "sites", &mut counts).await?;
    load::<probes::Entity>(&txn, file, "probes", &mut counts).await?;
    load::<networks::Entity>(&txn, file, "networks", &mut counts).await?;
    load::<devices::Entity>(&txn, file, "devices", &mut counts).await?;
    load::<device_interfaces::Entity>(&txn, file, "device_interfaces", &mut counts).await?;
    load::<device_links::Entity>(&txn, file, "device_links", &mut counts).await?;
    load::<monitors::Entity>(&txn, file, "monitors", &mut counts).await?;
    load::<alert_rules::Entity>(&txn, file, "alert_rules", &mut counts).await?;
    load::<vpn_servers::Entity>(&txn, file, "vpn_servers", &mut counts).await?;
    load::<vpn_peers::Entity>(&txn, file, "vpn_peers", &mut counts).await?;
    load::<dns_servers::Entity>(&txn, file, "dns_servers", &mut counts).await?;
    load::<system_settings::Entity>(&txn, file, "system_settings", &mut counts).await?;

    realign_sequences(&txn).await?;
    txn.commit().await?;
    Ok(counts)
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

async fn dump<E>(
    db: &DatabaseConnection,
    out: &mut serde_json::Map<String, serde_json::Value>,
    table: &str,
) -> AppResult<()>
where
    E: EntityTrait,
    E::Model: Serialize,
{
    let rows = E::find().all(db).await?;
    let value = serde_json::to_value(rows)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("serializar '{table}': {err}")))?;
    out.insert(table.to_string(), value);
    Ok(())
}

async fn load<E>(
    txn: &sea_orm::DatabaseTransaction,
    file: &BackupFile,
    table: &str,
    counts: &mut TableCounts,
) -> AppResult<()>
where
    E: EntityTrait,
    E::Model: DeserializeOwned + IntoActiveModel<E::ActiveModel>,
    E::ActiveModel: ActiveModelTrait<Entity = E> + Send,
{
    let Some(value) = file.tables.get(table) else {
        return Ok(());
    };
    let rows: Vec<E::Model> = serde_json::from_value(value.clone()).map_err(|err| {
        AppError::validation(format!(
            "A tabela '{table}' do backup não casa com o esquema atual: {err}"
        ))
    })?;

    let total = rows.len();
    for row in rows {
        // `reset_all` troca todo campo de `Unchanged` para `Set`: sem isso o
        // `insert` sairia sem colunas, porque um `ActiveModel` vindo de `Model`
        // nasce inteiro como "não modificado". É também o que preserva o `id`
        // em vez de deixar o banco gerar um novo.
        E::insert(row.into_active_model().reset_all())
            .exec(txn)
            .await?;
    }
    counts.push((table.to_string(), total));
    Ok(())
}

/// Apaga a configuração atual e o histórico que depende dela.
///
/// Percorre filhos antes de pais — a ordem inversa da criação — para não
/// esbarrar em FK nos bancos que as checam.
async fn wipe(txn: &sea_orm::DatabaseTransaction) -> AppResult<()> {
    let existing = tables::existing_tables(txn).await?;
    let backend = txn.get_database_backend();

    let doomed = DEPENDENT_HISTORY
        .iter()
        .copied()
        .chain(BACKED_UP_TABLES.iter().rev().copied());
    // Um mesmo nome não aparece nas duas listas, mas a ordem final ainda tem de
    // respeitar a hierarquia: histórico primeiro, depois configuração de baixo
    // para cima.
    for table in doomed {
        if !existing.contains(table) {
            continue;
        }
        txn.execute_raw(Statement::from_string(
            backend,
            format!("DELETE FROM \"{table}\""),
        ))
        .await?;
    }
    Ok(())
}

/// Realinha as sequências de `id` no PostgreSQL depois de uma carga com ids
/// explícitos.
///
/// O `bigserial` não sabe que alguém inseriu id 42 à mão: a sequência continua
/// no valor de antes e o próximo cadastro feito pela tela estouraria com
/// violação de chave primária. `setval` com o maior id existente conserta isso.
/// No SQLite não há o que fazer — o `rowid` seguinte é derivado do maior valor
/// da tabela.
async fn realign_sequences(txn: &sea_orm::DatabaseTransaction) -> AppResult<()> {
    if txn.get_database_backend() != DatabaseBackend::Postgres {
        return Ok(());
    }
    for table in BACKED_UP_TABLES {
        // `is_called = false` faz o próximo `nextval` devolver o próprio valor
        // passado, o que dá o comportamento certo na tabela vazia (começa em 1).
        let sql = format!(
            "SELECT setval(pg_get_serial_sequence('{table}', 'id'), \
             COALESCE((SELECT MAX(id) FROM \"{table}\"), 0) + 1, false)"
        );
        txn.execute_raw(Statement::from_string(DatabaseBackend::Postgres, sql))
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
            tables,
        };
        assert!(inspect(&arquivo).is_err());
    }
}
