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
    logger,
    task::Tasks,
    Result,
};
use migration::Migrator;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::path::Path;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[allow(unused_imports)]
use crate::{
    controllers,
    initializers::monitoring::MonitoringInitializer,
    initializers::process_deps,
    initializers::setup::SetupInitializer,
    initializers::syslog::SyslogInitializer,
    initializers::system_device::SystemDeviceInitializer,
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
        require_jwt_secret_in_production();
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

        // O **pipeline** de log (fila, escritor, barramento) monta aqui, e não
        // no `initializers::syslog`, por dois motivos que se somam:
        //
        // 1. `Hooks::init_logger` recebe o `AppContext` **depois** do
        //    `after_context` e **antes** dos initializers. É ali que a camada
        //    de `tracing` é instalada, e ela precisa da fila já existindo.
        // 2. `run_task` não executa initializers. Com a montagem lá, um
        //    processo `task scheduler_loop` ficaria sem log de aplicação
        //    nenhum — justamente o processo que mais tem o que contar.
        //
        // Os *listeners* continuam no initializer: abrir porta é coisa de
        // servidor.
        if let Err(error) = syslog::build(&ctx, &syslog::SyslogConfig::from_env()) {
            tracing::warn!(%error, "não foi possível montar o pipeline de logs");
        }
        Ok(ctx)
    }

    /// Instala a camada que grava o log da aplicação como log do dispositivo.
    ///
    /// Compõe com `logger::init_env_filter` e `logger::init_layer` do próprio
    /// Loco: a política de filtro e o formato continuam sendo os do
    /// `config.logger`, sem whitelist nem formato redeclarados aqui. O stdout
    /// segue intacto — é por ele que se opera o container —, e o evento não é
    /// duplicado dentro da aplicação: são dois destinos do **mesmo** evento.
    ///
    /// **O `file_appender` do Loco não é reproduzido**: nenhum `config/*.yaml`
    /// deste projeto o habilita, e copiar aqui um caminho de código que
    /// ninguém exercita seria dívida antes do primeiro uso. Ligá-lo pede
    /// acrescentar a camada correspondente nesta função — não há como fazê-lo
    /// pela configuração sozinha, porque o `init` do Loco deixou de rodar.
    fn init_logger(ctx: &AppContext) -> Result<bool> {
        let config = &ctx.config.logger;
        if !config.enable {
            // Logger desligado por configuração: nada a instalar, e o Loco
            // também não instalaria nada.
            return Ok(false);
        }

        let filtro =
            logger::init_env_filter::<Self>(config.override_filter.as_ref(), &config.level);
        tracing_subscriber::registry()
            .with(logger::init_layer(std::io::stdout, &config.format, true))
            .with(syslog::app_layer::AppLogLayer)
            .with(filtro)
            .init();
        Ok(true)
    }

    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![
            Box::new(SetupInitializer),
            Box::new(MonitoringInitializer),
            Box::new(SystemDeviceInitializer),
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
            .add_route(controllers::server_addresses::routes().layer(business_auth.clone()))
            .add_route(controllers::settings::routes().layer(business_auth.clone()))
            .add_route(controllers::users::routes().layer(business_auth.clone()))
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

/// Falha o boot em produção se `JWT_SECRET` não estiver definida.
///
/// O Loco assina JWT com HS512 a partir de uma chave base64 vinda do ambiente.
/// Deixar um default no `production.yaml` ou no `docker-compose.yml` é o mesmo
/// que publicar a chave no repositório: qualquer um que leia o código consegue
/// forjar tokens de admin. Por isso a produção exige a variável explicitamente.
fn require_jwt_secret_in_production() {
    let is_production = std::env::var("LOCO_ENV").map(|env| env == "production") == Ok(true);
    if !is_production {
        return;
    }

    let jwt_secret = std::env::var("JWT_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty());
    assert!(
        jwt_secret.is_some(),
        "JWT_SECRET é obrigatória em production: sem ela a assinatura de JWT usa um segredo que \
         está no código-fonte"
    );
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
