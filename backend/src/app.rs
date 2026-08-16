use async_trait::async_trait;
use axum::Router as AxumRouter;
use loco_rs::{
    app::{AppContext, Hooks, Initializer},
    bgworker::Queue,
    boot::{create_app, BootResult, StartMode},
    config::Config,
    controller::AppRoutes,
    db,
    environment::Environment,
    task::Tasks,
    Result,
};
use migration::Migrator;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::path::Path;

#[allow(unused_imports)]
use crate::{
    controllers,
    initializers::monitoring::MonitoringInitializer,
    initializers::process_deps,
    initializers::setup::SetupInitializer,
    initializers::syslog::SyslogInitializer,
    models::_entities::users,
    models::tables,
    services::syslog,
    spa, tasks,
    tasks::scheduler_run::{SchedulerLoop, SchedulerRun},
};

pub struct App;
#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> Result<BootResult> {
        create_app::<Self, Migrator>(mode, environment, config).await
    }

    /// Dependências de processo, disponíveis em **todos** os modos.
    ///
    /// Este é o único gancho do Loco que roda tanto no `start` quanto no
    /// `task` e no `db migrate`: o `create_context` o chama antes de qualquer
    /// coisa (`loco_rs::boot::create_context`). Os `Initializer` **não**
    /// servem para isto — o `run_task` não os executa, e foi exatamente essa
    /// suposição que deixou o `scheduler` e o `probe` sem cliente ICMP e sem
    /// barramento de eventos, com todo monitor de ping caindo em `unknown`.
    ///
    /// É também o único ponto que roda **antes** do migrator: o `create_app`
    /// chama `create_context` e só depois `db::converge`. É daí que
    /// [`migration::purge_removed_migrations`] tira a chance de apagar o
    /// registro das migrations que saíram do repositório — sem isso, o
    /// `sea-orm-migration` aborta o boot de qualquer banco que já as tenha
    /// aplicado.
    async fn after_context(ctx: AppContext) -> Result<AppContext> {
        process_deps::install(&ctx);
        enable_sqlite_wal(&ctx).await;
        migration::purge_removed_migrations(&ctx.db)
            .await
            .map_err(loco_rs::Error::DB)?;
        // O banco de logs é separado e tem migrator próprio. Fica aqui, e não
        // num `Initializer`, porque a purga de retenção roda no ciclo do
        // `scheduler` — que pode ser invocado como `task`, e o `run_task` não
        // executa initializers (ADR 007). Os *listeners*, esses sim, são
        // exclusivos do servidor e vivem em `initializers::syslog`.
        let database_url = ctx.config.database.uri.clone();
        syslog::db::install(&ctx, &database_url).await;
        Ok(ctx)
    }

    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![
            Box::new(SetupInitializer),
            Box::new(MonitoringInitializer),
            Box::new(SyslogInitializer),
        ])
    }

    fn routes(ctx: &AppContext) -> AppRoutes {
        // `prefix` só vale para as rotas adicionadas depois dele (`add_route`
        // funde o prefixo no momento da adição). Tudo que é negócio vem depois,
        // sob `/api` (§5.6) — inclusive a identificação do serviço, que morava
        // em `GET /`. A raiz agora é da SPA (ver `Hooks::after_routes`), e uma
        // rota registrada vence o `fallback_service`: mantê-la lá devolveria
        // JSON a quem abrisse o endereço no navegador.
        let business_auth =
            axum::middleware::from_fn_with_state(ctx.clone(), controllers::auth_guard::require_jwt);
        AppRoutes::with_default_routes()
            .prefix("/api")
            .add_route(controllers::root::routes())
            .add_route(controllers::auth::routes())
            // O agente do probe não tem sessão de usuário: autentica-se pelo
            // `X-Probe-Token` dentro do handler (§7.10). Fora do guarda JWT.
            .add_route(controllers::probes::agent_routes())
            .add_route(controllers::dashboard::routes().layer(business_auth.clone()))
            .add_route(controllers::backup::routes().layer(business_auth.clone()))
            .add_route(controllers::sites::routes().layer(business_auth.clone()))
            .add_route(controllers::networks::routes().layer(business_auth.clone()))
            .add_route(controllers::devices::routes().layer(business_auth.clone()))
            .add_route(controllers::monitors::routes().layer(business_auth.clone()))
            .add_route(controllers::discovery::routes().layer(business_auth.clone()))
            .add_route(controllers::topology::routes().layer(business_auth.clone()))
            .add_route(controllers::snmp::routes().layer(business_auth.clone()))
            .add_route(controllers::probes::routes().layer(business_auth.clone()))
            .add_route(controllers::port_scan::routes().layer(business_auth.clone()))
            .add_route(controllers::dns::routes().layer(business_auth.clone()))
            .add_route(controllers::dns_servers::routes().layer(business_auth.clone()))
            .add_route(controllers::events::routes().layer(business_auth.clone()))
            .add_route(controllers::logs::routes().layer(business_auth.clone()))
            .add_route(controllers::alerts::rules_routes().layer(business_auth.clone()))
            .add_route(controllers::alerts::routes().layer(business_auth.clone()))
            .add_route(controllers::vpn_servers::routes().layer(business_auth.clone()))
            .add_route(controllers::vpn_peers::routes().layer(business_auth))
    }

    /// Monta a SPA depois das rotas — e só depois delas.
    ///
    /// O `fallback_service` do [`spa::mount`] atende o que não casou com rota
    /// nenhuma; qualquer caminho sob `/api` continua sendo da API. É esta
    /// inversão que aposenta o nginx: um processo, uma porta, uma origem.
    async fn after_routes(router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        Ok(spa::mount(router, &spa::web_root()))
    }
    /// Nenhum worker de fila registrado — o trabalho de background é o ciclo do
    /// `scheduler` e o agente do `probe`, ambos processos próprios.
    async fn connect_workers(_ctx: &AppContext, _queue: &Queue) -> Result<()> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn register_tasks(tasks: &mut Tasks) {
        // tasks-inject (do not remove)
        tasks.register(tasks::user_create::UserCreate);
        tasks.register(tasks::auth_setup_token::AuthSetupToken);
        tasks.register(SchedulerRun);
        tasks.register(SchedulerLoop);
        tasks.register(tasks::probe_run::ProbeRun);
        tasks.register(tasks::probe_register::ProbeRegister);
        tasks.register(tasks::vpn_probe_register::VpnProbeRegister);
        tasks.register(tasks::vpn_secrets_import::VpnSecretsImport);
    }
    /// Limpa o esquema inteiro entre testes.
    ///
    /// A lista vive em [`tables::CREATION_ORDER`] e é percorrida ao contrário
    /// (filhos antes de pais). Tabelas ainda não migradas são puladas, então a
    /// lista já está completa desde a Fase 0 — nenhuma tabela nova precisa ser
    /// lembrada aqui depois.
    async fn truncate(ctx: &AppContext) -> Result<()> {
        tables::truncate_all(&ctx.db).await
    }
    async fn seed(ctx: &AppContext, base: &Path) -> Result<()> {
        db::seed::<users::ActiveModel>(&ctx.db, &base.join("users.yaml").display().to_string())
            .await?;
        Ok(())
    }
}

/// Liga o WAL quando o banco é SQLite.
///
/// No modo padrão (`delete`) um `INSERT` bloqueia toda leitura, e o servidor
/// virou um processo só: o ciclo do scheduler grava resultados enquanto a tela
/// consulta o histórico. Com WAL, leitor e escritor convivem — é a diferença
/// entre o SQLite servir de banco de produção e não servir.
///
/// `journal_mode` é propriedade **do arquivo**, não da conexão: gravado uma
/// vez, vale para sempre e para todo processo que abrir o banco. Por isso um
/// `PRAGMA` no boot basta, e por isso ele não precisa entrar no pool.
///
/// O `busy_timeout` de 5 s que serializa dois escritores já vem do `sqlx`.
///
/// Falha aqui não derruba o boot: um banco em modo `delete` é mais lento, não
/// inutilizável — e no Postgres a função nem chega a executar.
async fn enable_sqlite_wal(ctx: &AppContext) {
    if ctx.db.get_database_backend() != DatabaseBackend::Sqlite {
        return;
    }
    let pragma = Statement::from_string(DatabaseBackend::Sqlite, "PRAGMA journal_mode = WAL;");
    match ctx.db.query_one_raw(pragma).await {
        Ok(_) => tracing::debug!("SQLite em WAL"),
        Err(error) => tracing::warn!(%error, "não foi possível ligar o WAL do SQLite"),
    }
}
