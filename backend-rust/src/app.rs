use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Hooks, Initializer},
    bgworker::{BackgroundWorker, Queue},
    boot::{create_app, BootResult, StartMode},
    config::Config,
    controller::AppRoutes,
    db,
    environment::Environment,
    task::Tasks,
    Result,
};
use migration::Migrator;
use std::path::Path;

#[allow(unused_imports)]
use crate::{
    controllers, initializers::monitoring::MonitoringInitializer, models::_entities::users,
    models::tables, tasks, tasks::scheduler_run::SchedulerRun, workers::downloader::DownloadWorker,
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

    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![Box::new(MonitoringInitializer)])
    }

    fn routes(ctx: &AppContext) -> AppRoutes {
        // `prefix` só vale para as rotas adicionadas depois dele (`add_route`
        // funde o prefixo no momento da adição). Por isso `GET /` entra antes:
        // ele tem de continuar na raiz, junto com o `_ping`/`_health` que o
        // `with_default_routes` já registrou. Tudo que é negócio vem depois,
        // sob `/api` (§5.6).
        let business_auth =
            axum::middleware::from_fn_with_state(ctx.clone(), controllers::auth_guard::require_jwt);
        AppRoutes::with_default_routes()
            .add_route(controllers::root::routes())
            .prefix("/api")
            .add_route(controllers::auth::routes())
            .add_route(controllers::dashboard::routes().layer(business_auth.clone()))
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
            .add_route(controllers::zabbix_templates::routes().layer(business_auth))
    }
    async fn connect_workers(ctx: &AppContext, queue: &Queue) -> Result<()> {
        queue.register(DownloadWorker::build(ctx)).await?;
        Ok(())
    }

    #[allow(unused_variables)]
    fn register_tasks(tasks: &mut Tasks) {
        // tasks-inject (do not remove)
        tasks.register(tasks::user_create::UserCreate);
        tasks.register(SchedulerRun);
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
