//! Catálogo das tabelas do esquema e limpeza para os testes.
//!
//! [`CREATION_ORDER`] é a ordem de criação da §6 do roadmap — a mesma ordem em
//! que as migrations rodam, porque cada tabela só pode nascer depois dos alvos
//! das suas FKs. Manter a lista num lugar só evita o problema clássico de o
//! `truncate` dos testes esquecer uma tabela nova e um teste passar a depender
//! de sujeira deixada pelo anterior.
//!
//! O `Hooks::truncate` do Loco recebe entidades SeaORM, que só existem depois
//! de `cargo loco db entities` (Fase 1). Enquanto o esquema é construído, a
//! limpeza é feita por nome, pulando tabelas ainda inexistentes — assim a lista
//! já pode estar completa desde a Fase 0.

use std::collections::HashSet;

use loco_rs::Result;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

/// As 68 tabelas do esquema, na ordem de criação da §6 (pais antes de filhos).
pub const CREATION_ORDER: [&str; 68] = [
    "users",
    "sites",
    "probes",
    "networks",
    "devices",
    "device_interfaces",
    "device_links",
    "monitors",
    "monitor_results",
    "monitor_results_hourly",
    "metrics",
    "discovery_runs",
    "discovery_results",
    "alert_rules",
    "alert_events",
    "vpn_servers",
    "vpn_peers",
    "dns_servers",
    "event_outbox",
    // Fase 4 do roadmap de alertas inteligentes: o diário de notificações.
    // Depois de `alert_rules` e `devices`, para quem ela referencia já existir.
    "notification_outbox",
    "probe_tasks",
    "system_settings",
    // Fase 3 do roadmap: janelas de manutenção. Depois de `sites`,
    // `devices` e `users`, para as FKs já existirem.
    "maintenance_windows",
    // Fase 3 do roadmap: trilha de auditoria. Depois de `users` pela FK.
    "audit_logs",
    // Notificações Web Push (PWA). Depois de `users` pela FK.
    "push_subscriptions",
    // Conversas do Assistente IA. Depois de `users` pela FK.
    "ai_conversations",
    // Tokens pessoais do servidor MCP (ADR 024). Depois de `users` pela FK.
    "mcp_tokens",
    // Histórico de métricas de host e containers (ADR 011). Sem FK.
    "host_metrics_1m",
    "container_metrics_1m",
    // Plugins de dispositivo. Depois de `devices`, `probes` e `users` pelas FKs;
    // `plugin_runs` depois de `plugins`.
    "plugins",
    "device_credentials",
    "plugin_runs",
    "plugin_auto_accept",
    // Plugins instalados por equipamento. Depois de `plugins`.
    "device_plugin_installs",
    // Plugins de frota: configuração guardada e execução em lote.
    "plugin_settings",
    "plugin_batches",
    // Registro de fabricantes do IEEE (dado público, fora do backup). Sem FK.
    "oui_vendors",
    // Armazenamentos, destinos do backup das configurações. Sem FK.
    "storage_destinations",
    // Bancos de terceiros e o histórico dos backups deles. Depois de
    // `storage_destinations` pela FK do destino.
    "database_connections",
    "database_backups",
    // Deploy contínuo (ADR 021). Depois de `probes` e `database_connections`
    // pelas FKs dos alvos; cada filha depois do pai.
    "deploy_credentials",
    "deploy_github_connections",
    "deploy_applications",
    "deploy_targets",
    "deploy_env_versions",
    "deploy_build_secrets",
    "deploy_builds",
    "deploy_runs",
    // Registry próprio (ADR 022). Repositórios antes do resto; tokens depois
    // de `users`; achados depois dos manifests.
    "registry_repositories",
    "registry_blobs",
    "registry_repository_blobs",
    "registry_manifests",
    "registry_tags",
    "registry_uploads",
    "registry_tokens",
    "registry_findings",
    "registry_advisories",
    "registry_feeds",
    "registry_exceptions",
    // Plano de backup do próprio NetMonitor. Depois de `storage_destinations`.
    "system_backup_plan",
    // Teste de enlace (ADR 015). Agendamentos antes dos testes (FK
    // `schedule_id`) e testes antes das pernas.
    "link_test_schedules",
    "link_tests",
    "link_test_legs",
    // Tarefas agendadas. Depois de `users` pela FK; execuções depois das
    // tarefas.
    "scheduled_tasks",
    "scheduled_task_runs",
    // Clientes Wi-Fi vigiados. Depois de `devices` e `users` pelas FKs.
    "wifi_watched_clients",
    // Histórico de roaming Wi-Fi. Depois de `devices` pelas FKs.
    "wifi_roaming_events",
    // Opcional (§6 #23 / §10). **Não migrada:** a §10.2 optou por
    // `loco_rs::auth::JWT`, que não guarda token no banco. Fica listada porque
    // a limpeza pula tabelas inexistentes e a Fase 6 ainda pode voltar atrás.
    "auth_tokens",
];

/// As tabelas do banco de logs (`SYSLOG_DB_URL`, ADR 020), fora do
/// [`CREATION_ORDER`] porque não nascem no migrator principal.
pub const LOG_TABLES: [&str; 2] = ["device_logs", "log_templates"];

/// Nome legível de cada tabela, para as telas que falam de banco (o espaço
/// ocupado, o conteúdo de um backup). Um teste garante que nenhuma tabela do
/// esquema fique sem nome.
const LABELS: [(&str, &str); 71] = [
    ("users", "Usuários"),
    ("sites", "Sites"),
    ("probes", "Agentes e sondas"),
    ("networks", "Redes IP"),
    ("devices", "Dispositivos"),
    ("device_interfaces", "Interfaces dos dispositivos"),
    ("device_links", "Enlaces de topologia"),
    ("monitors", "Monitores"),
    ("monitor_results", "Resultados dos monitores"),
    (
        "monitor_results_hourly",
        "Resultados por hora (consolidado)",
    ),
    ("metrics", "Métricas dos dispositivos"),
    ("discovery_runs", "Execuções de descoberta"),
    ("discovery_results", "Resultados de descoberta"),
    ("alert_rules", "Regras de alerta"),
    ("alert_events", "Histórico de alertas"),
    ("vpn_servers", "Servidor VPN"),
    ("vpn_peers", "Dispositivos da VPN"),
    ("dns_servers", "Servidores DNS"),
    ("event_outbox", "Fila de eventos em tempo real"),
    ("notification_outbox", "Diário de notificações"),
    ("probe_tasks", "Tarefas das sondas"),
    ("system_settings", "Preferências do sistema"),
    ("maintenance_windows", "Janelas de manutenção"),
    ("audit_logs", "Auditoria"),
    ("push_subscriptions", "Inscrições de notificação push"),
    ("ai_conversations", "Conversas do Assistente IA"),
    ("mcp_tokens", "MCP: tokens pessoais"),
    ("host_metrics_1m", "Métricas dos servidores (1 min)"),
    ("container_metrics_1m", "Métricas dos containers (1 min)"),
    ("plugins", "Plugins de dispositivo"),
    (
        "device_credentials",
        "Credenciais de acesso aos equipamentos",
    ),
    ("plugin_runs", "Execuções de plugins"),
    ("plugin_auto_accept", "Aceites automáticos de plugins"),
    (
        "device_plugin_installs",
        "Plugins instalados por equipamento",
    ),
    ("plugin_settings", "Configurações de plugins"),
    ("plugin_batches", "Execuções de plugins em lote"),
    ("oui_vendors", "Fabricantes (registro IEEE)"),
    ("storage_destinations", "Destinos de backup"),
    ("database_connections", "Bancos de dados protegidos"),
    ("database_backups", "Histórico de backups de bancos"),
    ("deploy_credentials", "Credenciais de deploy"),
    ("deploy_github_connections", "Conexões com o GitHub"),
    ("deploy_applications", "Aplicações de deploy"),
    ("deploy_targets", "Destinos de deploy"),
    ("deploy_env_versions", "Versões do .env das aplicações"),
    ("deploy_build_secrets", "Segredos de compilação"),
    ("deploy_builds", "Compilações"),
    ("deploy_runs", "Execuções de deploy"),
    ("registry_repositories", "Registry: repositórios"),
    ("registry_blobs", "Registry: camadas"),
    (
        "registry_repository_blobs",
        "Registry: camadas por repositório",
    ),
    ("registry_manifests", "Registry: manifests"),
    ("registry_tags", "Registry: tags"),
    ("registry_uploads", "Registry: envios em andamento"),
    ("registry_tokens", "Registry: tokens pessoais"),
    (
        "registry_findings",
        "Registry: vulnerabilidades encontradas",
    ),
    ("registry_advisories", "Registry: base de vulnerabilidades"),
    ("registry_feeds", "Registry: fontes de vulnerabilidades"),
    (
        "registry_exceptions",
        "Registry: exceções de vulnerabilidade",
    ),
    ("system_backup_plan", "Plano de backup do NetMonitor"),
    ("link_test_schedules", "Agendamentos de teste de enlace"),
    ("link_tests", "Testes de enlace"),
    ("link_test_legs", "Medições dos testes de enlace"),
    ("scheduled_tasks", "Tarefas agendadas"),
    ("scheduled_task_runs", "Execuções de tarefas agendadas"),
    ("wifi_watched_clients", "Clientes Wi-Fi vigiados"),
    ("wifi_roaming_events", "Histórico de roaming Wi-Fi"),
    ("auth_tokens", "Tokens de sessão"),
    ("device_logs", "Logs dos dispositivos (Syslog)"),
    ("log_templates", "Modelos de log aprendidos"),
    ("seaql_migrations", "Controle de versões do esquema"),
];

/// O nome legível da tabela; o próprio nome técnico quando ela não é do
/// esquema (uma tabela criada à mão no banco, por exemplo).
#[must_use]
pub fn label(table: &str) -> &str {
    LABELS
        .iter()
        .find(|(name, _)| *name == table)
        .map_or(table, |(_, label)| label)
}

/// Tabela de controle do migrator. Precisa ser esvaziada entre testes para que
/// o `auto_migrate` do Loco reaplique as migrations sem erro de UNIQUE.
pub const MIGRATIONS_TABLE: &str = "seaql_migrations";

/// Ordem segura de limpeza: filhos antes dos pais, para não violar FK em bancos
/// que as checam (Postgres sempre; SQLite quando `foreign_keys=ON`).
#[must_use]
pub fn truncate_order() -> Vec<&'static str> {
    CREATION_ORDER.iter().rev().copied().collect()
}

/// Tabelas que existem de fato no banco conectado.
///
/// # Errors
///
/// Propaga falha de consulta ao catálogo do banco.
pub async fn existing_tables<C: ConnectionTrait>(db: &C) -> Result<HashSet<String>> {
    let backend = db.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Sqlite => {
            "SELECT name AS table_name FROM sqlite_master WHERE type = 'table'"
        }
        DatabaseBackend::Postgres => {
            "SELECT table_name::text AS table_name FROM information_schema.tables \
             WHERE table_schema = current_schema()"
        }
        // `DatabaseBackend` é `non_exhaustive`; MySQL e qualquer backend futuro
        // caem no catálogo padrão do SQL. Só SQLite e Postgres são suportados
        // pelo projeto (§13), então isto é rede de segurança, não promessa.
        _ => {
            "SELECT table_name AS table_name FROM information_schema.tables \
             WHERE table_schema = database()"
        }
    };

    // `*_raw` recebe um `Statement` pronto; `query_all` espera um builder do
    // sea-query, que não serve para SQL escrito à mão.
    let rows = db
        .query_all_raw(Statement::from_string(backend, sql))
        .await?;
    let mut names = HashSet::new();
    for row in rows {
        names.insert(row.try_get::<String>("", "table_name")?);
    }
    Ok(names)
}

/// Apaga todas as linhas das tabelas do esquema, ignorando as que ainda não
/// foram criadas.
///
/// Usa `DELETE FROM` e não `TRUNCATE`: o SQLite não tem `TRUNCATE`, e no
/// Postgres o `DELETE` dispensa o `CASCADE` quando a ordem já é a correta.
///
/// # Errors
///
/// Propaga falha do banco em qualquer uma das tabelas.
pub async fn truncate_all<C: ConnectionTrait>(db: &C) -> Result<()> {
    let existing = existing_tables(db).await?;
    let backend = db.get_database_backend();

    for table in truncate_order() {
        if !existing.contains(table) {
            continue;
        }
        db.execute_raw(Statement::from_string(
            backend,
            format!("DELETE FROM \"{table}\""),
        ))
        .await?;
    }
    // Limpa o controle do migrator por último, depois que todo o schema está
    // vazio — assim o `auto_migrate` dos testes reaplica as migrations sem
    // conflito de versão.
    if existing.contains(MIGRATIONS_TABLE) {
        db.execute_raw(Statement::from_string(
            backend,
            format!("DELETE FROM \"{MIGRATIONS_TABLE}\""),
        ))
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cobre_todas_as_tabelas_sem_repetir() {
        assert_eq!(CREATION_ORDER.len(), 68);
        let unicas: HashSet<&&str> = CREATION_ORDER.iter().collect();
        assert_eq!(unicas.len(), 68, "há nome de tabela repetido");
    }

    #[test]
    fn toda_tabela_tem_nome_legivel() {
        for table in CREATION_ORDER.iter().chain(LOG_TABLES.iter()) {
            assert_ne!(label(table), *table, "tabela sem nome legível: {table}");
        }
        assert_eq!(label("tabela_qualquer"), "tabela_qualquer");
    }

    #[test]
    fn limpeza_comeca_pelos_filhos() {
        let ordem = truncate_order();
        let posicao = |nome: &str| ordem.iter().position(|t| *t == nome).unwrap();
        // `devices` referencia `sites` e `networks`; tem de sumir antes deles.
        assert!(posicao("devices") < posicao("sites"));
        assert!(posicao("devices") < posicao("networks"));
        // `monitor_results` e seu rollup referenciam `monitors`.
        assert!(posicao("monitor_results") < posicao("monitors"));
        assert!(posicao("monitor_results_hourly") < posicao("monitors"));
        assert!(posicao("monitor_results_hourly") < posicao("monitor_results"));
        // `notification_outbox` referencia `alert_rules` e `devices`.
        assert!(posicao("notification_outbox") < posicao("alert_rules"));
        assert!(posicao("notification_outbox") < posicao("devices"));
        // `users` é a raiz: sempre por último.
        assert_eq!(*ordem.last().unwrap(), "users");
    }
}
