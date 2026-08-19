//! Identidade do dispositivo que representa o **próprio NetMonitor**.
//!
//! O servidor é um dispositivo de primeira classe: aparece na lista, tem
//! monitores, regras, métricas e logs como qualquer outro. Para isso ele
//! precisa ser localizável de forma estável — e nenhum dos campos do cadastro
//! serve. O ID varia por instalação, o nome é editável, IP/site/rede podem ser
//! nulos. A âncora é `devices.system_key`, com o valor [`NETMONITOR_KEY`].
//!
//! # Responsabilidades, separadas
//!
//! - [`SystemDeviceService`] garante que a linha exista — idempotente e
//!   seguro sob boots concorrentes.
//! - [`resolver`] é um cache de processo (`current() -> Option<i64>`) para os
//!   caminhos quentes, alimentado pelo serviço. Ninguém consulta o banco por
//!   linha de log: `device_logs` mora em outro banco e não tem FK para
//!   `devices`.
//! - [`ensure_deletable`] e [`ensure_identity_preserved`] são a regra de
//!   negócio que impede apagar o dispositivo ou mexer no que sustenta sua
//!   identidade.
//!
//! # Por que a proteção não vive em `ActiveModelBehavior`
//!
//! Um gatilho de entidade dispararia também no `wipe()` da restauração de
//! backup e no `truncate` da suíte de testes, quebrando os dois. A proteção é
//! do serviço/controller, onde existe intenção do usuário.
//!
//! # Por que não é um perfil de acesso
//!
//! A política do produto tem duas linhas — `viewer` lê, `operator`/`admin`
//! escrevem, `admin` também administra usuários — e não ganha uma terceira
//! categoria por causa disto. **Ninguém** apaga o dispositivo do sistema, nem
//! `admin`: é regra de negócio, não permissão.

use std::sync::atomic::{AtomicI64, Ordering};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};

use crate::{
    models::devices,
    services::shared::errors::{AppError, AppResult},
};

/// Valor de `devices.system_key` do servidor. Nunca muda: é ele que sobrevive
/// a renomeações, restaurações de backup e troca de IP.
pub const NETMONITOR_KEY: &str = "netmonitor";

/// Nome exibido na criação. Depois disso o usuário pode renomear à vontade —
/// a identidade não depende dele.
pub const NETMONITOR_DEFAULT_NAME: &str = "Servidor NetMonitor";

/// `devices.type` do servidor. Genérico de propósito: é um tipo de
/// equipamento, não uma categoria paralela do produto.
pub const NETMONITOR_TYPE: &str = "server";

/// Cache de processo do ID do dispositivo do sistema.
///
/// `0` significa "ainda não resolvido". Um `AtomicI64` basta: a escrita é rara
/// (boot e pós-restore) e a leitura acontece em caminho quente.
static CURRENT_ID: AtomicI64 = AtomicI64::new(0);

/// Leitura e invalidação do ID cacheado.
pub mod resolver {
    use super::{Ordering, CURRENT_ID};

    /// ID do dispositivo do sistema, se já resolvido neste processo.
    ///
    /// `None` antes de o boot terminar — e é assim que os logs emitidos
    /// durante migrations ficam sem `device_id`, comportamento explícito e
    /// coberto por teste.
    #[must_use]
    pub fn current() -> Option<i64> {
        match CURRENT_ID.load(Ordering::Relaxed) {
            0 => None,
            id => Some(id),
        }
    }

    /// Publica o ID resolvido. Chamado pelo serviço, não pelos consumidores.
    pub fn set(id: i64) {
        CURRENT_ID.store(id, Ordering::Relaxed);
    }

    /// Esquece o ID. Obrigatório ao restaurar um backup: o `wipe` + recarga
    /// devolve as linhas **com os IDs do arquivo**, e um ID cacheado passaria
    /// a apontar para outro equipamento.
    pub fn invalidate() {
        CURRENT_ID.store(0, Ordering::Relaxed);
    }
}

/// Garante exatamente um Servidor NetMonitor.
pub struct SystemDeviceService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> SystemDeviceService<'a> {
    #[must_use]
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// Busca a linha do servidor pela chave — nunca por ID, nome ou IP.
    pub async fn find(&self) -> Result<Option<devices::Model>, sea_orm::DbErr> {
        find_by_key(self.db).await
    }

    /// Garante a existência da linha e publica o ID no [`resolver`].
    ///
    /// Idempotente: rodar no segundo boot devolve a mesma linha. Sob boots
    /// concorrentes, o índice único `devices_system_key_unique` decide o
    /// vencedor e o perdedor relê a linha do outro em vez de falhar.
    ///
    /// Nenhum site, rede ou probe fictício é criado: esses vínculos ficam
    /// nulos porque não representam nada real.
    pub async fn ensure(&self) -> Result<devices::Model, sea_orm::DbErr> {
        if let Some(row) = self.find().await? {
            resolver::set(row.id);
            return Ok(row);
        }

        let txn = self.db.begin().await?;
        let inserted = devices::ActiveModel {
            name: Set(NETMONITOR_DEFAULT_NAME.to_string()),
            r#type: Set(NETMONITOR_TYPE.to_string()),
            system_key: Set(Some(NETMONITOR_KEY.to_string())),
            description: Set(Some(
                "Dispositivo que representa esta instalação do NetMonitor.".to_string(),
            )),
            is_monitored: Set(true),
            snmp_enabled: Set(false),
            status: Set("unknown".to_string()),
            ..Default::default()
        }
        .insert(&txn)
        .await;

        let row = match inserted {
            Ok(row) => {
                txn.commit().await?;
                row
            }
            // Corrida de boot: outro processo inseriu primeiro. A linha dele é
            // tão boa quanto a nossa.
            Err(error) => {
                txn.rollback().await?;
                find_by_key(self.db).await?.ok_or_else(|| {
                    sea_orm::DbErr::Custom(format!(
                        "não foi possível garantir o dispositivo do sistema: {error}"
                    ))
                })?
            }
        };

        resolver::set(row.id);
        Ok(row)
    }
}

async fn find_by_key(db: &DatabaseConnection) -> Result<Option<devices::Model>, sea_orm::DbErr> {
    devices::Entity::find()
        .filter(devices::Column::SystemKey.eq(NETMONITOR_KEY))
        .one(db)
        .await
}

/// Verdadeiro quando a linha é um dispositivo do sistema.
#[must_use]
pub fn is_protected(device: &devices::Model) -> bool {
    device.system_key.is_some()
}

/// Barra exclusão do dispositivo do sistema, para qualquer perfil.
pub fn ensure_deletable(device: &devices::Model) -> AppResult<()> {
    if is_protected(device) {
        return Err(AppError::BusinessRule(
            "O Servidor NetMonitor representa esta instalação e não pode ser excluído".to_string(),
        ));
    }
    Ok(())
}

/// Barra mudança dos campos que quebrariam a identidade técnica.
///
/// O bloqueio cobre só o que sustenta a identidade — tipo, endereçamento e
/// SNMP de um equipamento que não é alcançado pela rede. Nome, descrição e
/// intervalo continuam editáveis: renomear o servidor é legítimo, e a chave
/// não depende do nome.
pub fn ensure_identity_preserved(
    current: &devices::Model,
    proposed: &ProposedIdentity<'_>,
) -> AppResult<()> {
    if !is_protected(current) {
        return Ok(());
    }
    fn recusa(campo: &str) -> AppResult<()> {
        Err(AppError::BusinessRule(format!(
            "{campo} do Servidor NetMonitor não pode ser alterado: ele representa esta instalação"
        )))
    }
    if proposed
        .device_type
        .is_some_and(|kind| kind != current.r#type)
    {
        return recusa("O tipo");
    }
    if proposed.ip_address.is_some() {
        return recusa("O endereço IP");
    }
    if proposed.snmp_enabled.is_some_and(|snmp| snmp) {
        return recusa("O SNMP");
    }
    if proposed.network_id.is_some() {
        return recusa("A rede");
    }
    Ok(())
}

/// Campos propostos por uma edição, na forma em que o controller os recebe.
///
/// `None` significa "não informado", que é como o `PUT` parcial do cadastro já
/// se comporta.
#[derive(Default)]
pub struct ProposedIdentity<'a> {
    pub device_type: Option<&'a str>,
    pub ip_address: Option<&'a str>,
    pub snmp_enabled: Option<bool>,
    pub network_id: Option<i64>,
}
