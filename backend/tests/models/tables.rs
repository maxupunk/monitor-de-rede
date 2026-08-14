//! A lista de tabelas (`CREATION_ORDER`) contra o esquema realmente migrado.
//!
//! Sem isto, uma migration nova só apareceria no `Hooks::truncate` quando
//! alguém lembrasse — e o sintoma seria um teste passando por causa de sujeira
//! deixada pelo anterior, que é o tipo de falha que custa uma tarde.

use backend::{
    app::App,
    models::tables::{existing_tables, truncate_all, CREATION_ORDER},
};
use loco_rs::testing::prelude::*;
use serial_test::serial;

/// Tabelas de infraestrutura do próprio Loco/SeaORM — não são do domínio.
const INFRAESTRUTURA: &[&str] = &[
    "seaql_migrations",
    "pg_loco_queue",
    "sqlt_loco_queue",
    "sqlt_loco_queue_lock",
];

/// §10.2 optou por JWT, que não guarda token no banco. A entrada continua em
/// `CREATION_ORDER` para o dia em que a Fase 6 quiser tokens opacos.
const NAO_MIGRADAS: &[&str] = &["auth_tokens"];

#[tokio::test]
#[serial]
async fn toda_tabela_migrada_esta_no_creation_order() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let existentes = existing_tables(&boot.app_context.db)
        .await
        .expect("ler o catálogo");

    let faltando: Vec<_> = existentes
        .iter()
        .filter(|t| !t.starts_with("sqlite_"))
        .filter(|t| !INFRAESTRUTURA.contains(&t.as_str()))
        .filter(|t| !CREATION_ORDER.contains(&t.as_str()))
        .collect();

    assert!(
        faltando.is_empty(),
        "tabelas migradas fora de `CREATION_ORDER` — o truncate dos testes não \
         as limpa: {faltando:?}"
    );
}

#[tokio::test]
#[serial]
async fn toda_entrada_do_creation_order_existe_no_banco() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let existentes = existing_tables(&boot.app_context.db)
        .await
        .expect("ler o catálogo");

    let ausentes: Vec<_> = CREATION_ORDER
        .iter()
        .filter(|t| !NAO_MIGRADAS.contains(*t))
        .filter(|t| !existentes.contains(**t))
        .collect();

    assert!(
        ausentes.is_empty(),
        "entradas de `CREATION_ORDER` sem migration correspondente: {ausentes:?}"
    );
}

#[tokio::test]
#[serial]
async fn truncate_respeita_as_chaves_estrangeiras() {
    // Se a ordem estivesse errada, apagar `sites` antes de `devices` violaria a
    // FK e o `truncate_all` estouraria — que é o ponto do teste.
    let boot = boot_test::<App>().await.expect("subir app de teste");
    truncate_all(&boot.app_context.db)
        .await
        .expect("limpar o esquema inteiro");
    truncate_all(&boot.app_context.db)
        .await
        .expect("limpar de novo deve ser idempotente");
}
