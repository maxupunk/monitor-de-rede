//! `paginate_compat` contra um banco real (§5.4).
//!
//! Os testes unitários em `services/shared/pagination.rs` cobrem a aritmética
//! do `meta`. Aqui o que se prova é o acoplamento com o SeaORM: que a página
//! 1 do contrato HTTP é a página 0 do `Paginator`, e que `total` conta o
//! conjunto inteiro, não a página.

use backend::{
    app::App,
    models::_entities::users,
    services::shared::pagination::{normalize_limit, paginate_compat},
};
use loco_rs::testing::prelude::*;
use sea_orm::EntityTrait;
use serial_test::serial;

async fn semeie_usuarios(db: &sea_orm::DatabaseConnection, quantidade: usize) {
    for i in 0..quantidade {
        backend::models::users::Model::create_with_password(
            db,
            &backend::models::users::RegisterParams {
                email: format!("paginacao{i}@exemplo.com"),
                password: "Senha1234".to_string(),
                name: format!("Usuário {i}"),
            },
        )
        .await
        .expect("criar usuário de teste");
    }
}

#[tokio::test]
#[serial]
async fn pagina_1_do_contrato_e_a_primeira_do_banco() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    semeie_usuarios(&boot.app_context.db, 7).await;

    let primeira = paginate_compat(
        &boot.app_context.db,
        users::Entity::find(),
        1,
        3,
        |u: users::Model| u.email,
    )
    .await
    .expect("paginar");

    assert_eq!(primeira.data.len(), 3);
    assert_eq!(primeira.meta.total, 7);
    assert_eq!(primeira.meta.current_page, 1);
    assert_eq!(primeira.meta.last_page, 3);
    assert_eq!(primeira.meta.previous_page_url, None);

    let segunda = paginate_compat(
        &boot.app_context.db,
        users::Entity::find(),
        2,
        3,
        |u: users::Model| u.email,
    )
    .await
    .expect("paginar");

    assert_eq!(segunda.meta.current_page, 2);
    assert_ne!(
        primeira.data, segunda.data,
        "a página 2 repetiu a 1 — o offset 0-based do SeaORM não foi ajustado"
    );
}

#[tokio::test]
#[serial]
async fn percorrer_ate_o_fim_devolve_todas_as_linhas_uma_vez() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    semeie_usuarios(&boot.app_context.db, 7).await;

    // Réplica do laço do `useInfiniteList`.
    let mut acumulado: Vec<String> = Vec::new();
    let mut pagina = 1;
    loop {
        let atual = paginate_compat(
            &boot.app_context.db,
            users::Entity::find(),
            pagina,
            3,
            |u: users::Model| u.email,
        )
        .await
        .expect("paginar");

        acumulado.extend(atual.data);
        if atual.meta.current_page >= atual.meta.last_page {
            break;
        }
        pagina += 1;
        assert!(pagina < 20, "o laço de paginação não terminou");
    }

    assert_eq!(acumulado.len(), 7);
    let unicos: std::collections::HashSet<_> = acumulado.iter().collect();
    assert_eq!(unicos.len(), 7, "houve linha repetida entre páginas");
}

#[tokio::test]
#[serial]
async fn conjunto_vazio_nao_prende_a_lista_infinita() {
    let boot = boot_test::<App>().await.expect("subir app de teste");

    let pagina = paginate_compat(
        &boot.app_context.db,
        users::Entity::find(),
        1,
        20,
        |u: users::Model| u.email,
    )
    .await
    .expect("paginar");

    assert!(pagina.data.is_empty());
    assert_eq!(pagina.meta.total, 0);
    assert!(pagina.meta.current_page >= pagina.meta.last_page);
}

#[tokio::test]
#[serial]
async fn limite_e_limitado_a_100_antes_de_ir_ao_banco() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    semeie_usuarios(&boot.app_context.db, 3).await;

    assert_eq!(normalize_limit(Some(100_000)), 100);

    let pagina = paginate_compat(
        &boot.app_context.db,
        users::Entity::find(),
        1,
        100_000,
        |u: users::Model| u.email,
    )
    .await
    .expect("paginar");

    assert_eq!(pagina.meta.per_page, 100);
}
