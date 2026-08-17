//! Preferências globais do sistema.
//!
//! # A regra que este módulo existe para cumprir
//!
//! **Uma preferência que ninguém lê continua sendo fachada.** A tela de
//! Configurações tinha três campos e um botão "Salvar" que chamava um `alert()`
//! — nada era gravado, e nada seria lido se fosse. Gravar sem consumir teria
//! trocado um engano visível por um invisível, que é pior: o operador ajusta o
//! valor, acredita ter mudado o comportamento e leva meses para descobrir que
//! não mudou.
//!
//! Por isso cada campo aqui tem um ponto de consumo declarado, e a tela diz
//! qual é:
//!
//! | preferência | onde ela muda o comportamento |
//! |---|---|
//! | `default_ping_interval_seconds` | intervalo de um monitor novo que não declara o seu |
//! | `default_snmp_community` | comunidade de um dispositivo com SNMP ligado e sem comunidade própria |
//! | `auto_discovery_enabled` | trava global da varredura periódica de redes |
//!
//! # Onde mora
//!
//! Em `system_settings`, chave [`STORAGE_KEY`], em JSON — mesma escolha do
//! `server.addresses` e do `syslog.source_bindings`: um punhado de valores,
//! escritos por ação manual, sem FK, já cobertos pelo backup e pelo `truncate`
//! de teste.
//!
//! # Leitura no caminho quente
//!
//! O agendador consulta [`load`] a cada ciclo. É uma linha de `system_settings`
//! por ciclo de cinco segundos — barato o bastante para não justificar cache, e
//! cache aqui teria o defeito de sempre: o operador desliga a descoberta na tela
//! e ela continua rodando até o cache vencer.

use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    models::system_settings,
    services::shared::errors::{AppError, AppResult},
};

/// Chave em `system_settings`.
pub const STORAGE_KEY: &str = "preferences";

/// Padrões. São os mesmos valores que estavam fixos no código antes de a tela
/// existir — trocar um deles aqui muda o comportamento de quem nunca abriu a
/// tela de Configurações.
pub const DEFAULT_PING_INTERVAL_SECONDS: i32 = 60;
pub const DEFAULT_SNMP_COMMUNITY: &str = "public";
pub const DEFAULT_AUTO_DISCOVERY: bool = true;

/// Piso e teto do intervalo de coleta.
///
/// O piso não é enfeite: um monitor de ping a cada segundo multiplica por
/// sessenta a carga do agendador e a taxa de escrita de métricas. O teto evita
/// o engano de digitar milissegundos achando que são segundos e acabar com um
/// monitor que roda uma vez por semana.
pub const MIN_PING_INTERVAL_SECONDS: i32 = 10;
pub const MAX_PING_INTERVAL_SECONDS: i32 = 86_400;

/// O contrato exportado para a tela vive aqui, e não em `dtos/`, pelo mesmo
/// motivo do `SetupGuide` do syslog: são os mesmos três campos, e um DTO
/// espelho divergiria deste struct na primeira mudança.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct Preferences {
    /// Intervalo aplicado a um monitor novo que não declara o seu.
    #[serde(default = "ping_padrao")]
    #[ts(type = "number")]
    pub default_ping_interval_seconds: i32,
    /// Comunidade aplicada a um dispositivo com SNMP ligado e sem comunidade.
    #[serde(default = "comunidade_padrao")]
    pub default_snmp_community: String,
    /// Trava global da varredura periódica. Desligada, as redes mantêm o
    /// `scan_enabled` individual — o que este campo faz é impedir o agendador
    /// de disparar qualquer uma delas, sem apagar a configuração de nenhuma.
    #[serde(default = "descoberta_padrao")]
    pub auto_discovery_enabled: bool,
}

fn ping_padrao() -> i32 {
    DEFAULT_PING_INTERVAL_SECONDS
}
fn comunidade_padrao() -> String {
    DEFAULT_SNMP_COMMUNITY.to_owned()
}
const fn descoberta_padrao() -> bool {
    DEFAULT_AUTO_DISCOVERY
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            default_ping_interval_seconds: DEFAULT_PING_INTERVAL_SECONDS,
            default_snmp_community: DEFAULT_SNMP_COMMUNITY.to_owned(),
            auto_discovery_enabled: DEFAULT_AUTO_DISCOVERY,
        }
    }
}

/// Lê as preferências. Chave ausente ou JSON corrompido devolve os padrões.
///
/// Degradar para o padrão, e não para erro, é deliberado: estas preferências
/// são consultadas no meio do ciclo do agendador e na criação de recursos. Um
/// valor ilegível não pode parar a coleta do parque inteiro — e o padrão é
/// exatamente o comportamento que existia antes de a tela existir.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn load<C: ConnectionTrait>(db: &C) -> AppResult<Preferences> {
    Ok(system_settings::Model::get(db, STORAGE_KEY)
        .await?
        .and_then(|linha| linha.value)
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default())
}

/// Grava as preferências, validando antes.
///
/// # Errors
///
/// Valor fora da faixa, comunidade vazia, ou erro do banco.
pub async fn save(
    db: &DatabaseConnection,
    mut preferencias: Preferences,
) -> AppResult<Preferences> {
    let intervalo = preferencias.default_ping_interval_seconds;
    if !(MIN_PING_INTERVAL_SECONDS..=MAX_PING_INTERVAL_SECONDS).contains(&intervalo) {
        return Err(AppError::validation(format!(
            "O intervalo padrão de ping precisa ficar entre {MIN_PING_INTERVAL_SECONDS} e \
             {MAX_PING_INTERVAL_SECONDS} segundos."
        )));
    }

    preferencias.default_snmp_community = preferencias.default_snmp_community.trim().to_owned();
    if preferencias.default_snmp_community.is_empty() {
        return Err(AppError::validation(
            "A comunidade SNMP padrão não pode ficar em branco — um dispositivo sem comunidade \
             própria não teria como responder.",
        ));
    }

    let texto = serde_json::to_string(&preferencias)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    system_settings::Model::set(db, STORAGE_KEY, Some(texto)).await?;
    Ok(preferencias)
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};

    async fn banco() -> DatabaseConnection {
        let db = sea_orm::Database::connect(
            sea_orm::ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    #[tokio::test]
    async fn sem_nada_gravado_valem_os_padroes_antigos() {
        // Os padrões precisam ser exatamente o comportamento que existia antes
        // da tela: quem nunca abriu Configurações não pode perceber a mudança.
        let db = banco().await;
        let preferencias = load(&db).await.expect("carregar");
        assert_eq!(preferencias, Preferences::default());
        assert_eq!(preferencias.default_ping_interval_seconds, 60);
        assert_eq!(preferencias.default_snmp_community, "public");
        assert!(preferencias.auto_discovery_enabled);
    }

    #[tokio::test]
    async fn o_que_e_gravado_e_lido_de_volta() {
        let db = banco().await;
        save(
            &db,
            Preferences {
                default_ping_interval_seconds: 120,
                default_snmp_community: "  privada  ".to_owned(),
                auto_discovery_enabled: false,
            },
        )
        .await
        .expect("gravar");

        let preferencias = load(&db).await.expect("carregar");
        assert_eq!(preferencias.default_ping_interval_seconds, 120);
        assert_eq!(preferencias.default_snmp_community, "privada", "aparado");
        assert!(!preferencias.auto_discovery_enabled);
    }

    #[tokio::test]
    async fn o_intervalo_fora_da_faixa_e_recusado_nas_duas_pontas() {
        let db = banco().await;
        for invalido in [0, 1, 9, 86_401, -30] {
            let erro = save(
                &db,
                Preferences {
                    default_ping_interval_seconds: invalido,
                    ..Preferences::default()
                },
            )
            .await
            .expect_err("devia recusar");
            assert!(
                format!("{erro:?}").contains("intervalo padrão"),
                "{invalido}"
            );
        }
        // E os extremos válidos passam.
        for valido in [MIN_PING_INTERVAL_SECONDS, MAX_PING_INTERVAL_SECONDS] {
            save(
                &db,
                Preferences {
                    default_ping_interval_seconds: valido,
                    ..Preferences::default()
                },
            )
            .await
            .expect("devia aceitar");
        }
    }

    #[tokio::test]
    async fn comunidade_em_branco_e_recusada() {
        let db = banco().await;
        for vazia in ["", "   "] {
            assert!(save(
                &db,
                Preferences {
                    default_snmp_community: vazia.to_owned(),
                    ..Preferences::default()
                },
            )
            .await
            .is_err());
        }
    }

    #[tokio::test]
    async fn json_corrompido_cai_no_padrao_em_vez_de_parar_o_agendador() {
        // `load` roda no ciclo do agendador: um valor ilegível não pode derrubar
        // a coleta do parque inteiro.
        let db = banco().await;
        system_settings::Model::set(&db, STORAGE_KEY, Some("{não é json".to_owned()))
            .await
            .expect("gravar lixo");
        assert_eq!(load(&db).await.expect("carregar"), Preferences::default());
    }

    #[tokio::test]
    async fn documento_parcial_completa_com_os_padroes() {
        // Uma versão futura acrescenta campo; a gravada pela versão antiga não
        // pode virar documento inválido.
        let db = banco().await;
        system_settings::Model::set(
            &db,
            STORAGE_KEY,
            Some(r#"{"defaultPingIntervalSeconds":300}"#.to_owned()),
        )
        .await
        .expect("gravar parcial");

        let preferencias = load(&db).await.expect("carregar");
        assert_eq!(preferencias.default_ping_interval_seconds, 300);
        assert_eq!(preferencias.default_snmp_community, "public");
        assert!(preferencias.auto_discovery_enabled);
    }
}
