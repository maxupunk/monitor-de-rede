//! Garante o dispositivo que representa esta instalação, no boot do servidor.
//!
//! # Por que um `Initializer`, e não `after_context`
//!
//! As migrations do banco principal só convergem **depois** do
//! `create_context`: o `create_app` chama `create_context` e só então
//! `db::converge`. Um serviço que rodasse em `after_context` consultaria
//! `devices.system_key` antes de a coluna existir e falharia em todo boot de
//! banco novo. Os `Initializer` rodam depois da convergência.
//!
//! Um `db migrate` ou uma task não precisam do dispositivo criado — quem
//! precisa do ID resolvido em caminho quente é o processo que atende HTTP e o
//! que roda o agendador, e ambos passam por aqui ou pelo restore.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

use crate::services::{
    alerts::catalog::health_defaults,
    devices::system_device::SystemDeviceService,
    monitoring::{managed::ensure_system_health_monitor, reachability},
};

pub struct SystemDeviceInitializer;

#[async_trait]
impl Initializer for SystemDeviceInitializer {
    fn name(&self) -> String {
        "system_device".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        // Banco indisponível não derruba o boot: sem o dispositivo, os logs
        // internos apenas ficam sem `device_id` até a próxima tentativa.
        let device = match SystemDeviceService::new(&ctx.db).ensure().await {
            Ok(device) => device,
            Err(error) => {
                tracing::warn!(%error, "não foi possível garantir o dispositivo do sistema");
                return Ok(());
            }
        };
        tracing::debug!(device_id = device.id, "dispositivo do sistema garantido");

        // A guarda de criação impede que um monitor de alcance nasça; ela não
        // desfaz o que já está no banco de quem atualizou no meio do caminho.
        // A remoção é idempotente e mora no mesmo boot que garante o
        // dispositivo — se ela ficasse em migration, uma instalação que
        // regredisse de versão recriaria o ping e ele ficaria.
        match reachability::purge_system_device(&ctx.db, &device).await {
            Ok(removidos) if !removidos.is_empty() => {
                tracing::info!(
                    removidos = removidos.len(),
                    "monitores de alcance removidos do dispositivo do sistema"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "não foi possível remover monitores de alcance do dispositivo do sistema");
            }
        }

        // A saúde do servidor entra pelo pipeline normal de monitoramento: um
        // monitor comum, que o agendador executa e cujo resultado o
        // `process_result` grava. Nada aqui é um caminho paralelo.
        match ensure_system_health_monitor(&ctx.db, device.id).await {
            Ok(monitor) => {
                tracing::debug!(monitor_id = monitor.id, "coleta de saúde provisionada");
            }
            Err(error) => {
                tracing::warn!(%error, "não foi possível provisionar a coleta de saúde do sistema");
            }
        }

        // As regras de saúde entram pelo catálogo normal, vinculadas ao
        // dispositivo — os mesmos templates que um roteador SNMP recebe. Uma
        // vez só: regra removida pelo operador não volta no boot seguinte.
        match health_defaults::ensure_for_device(&ctx.db, device.id).await {
            Ok(resultado) if !resultado.created.is_empty() => {
                tracing::info!(
                    criadas = resultado.created.len(),
                    "regras de saúde aplicadas ao dispositivo do sistema"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "não foi possível aplicar as regras de saúde do sistema");
            }
        }
        Ok(())
    }
}
