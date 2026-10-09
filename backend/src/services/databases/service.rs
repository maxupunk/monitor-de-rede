//! Cadastro das conexões de banco.
//!
//! A senha vive cifrada (`password_encrypted`, a mesma `ENCRYPTION_KEY` dos
//! armazenamentos) e nunca volta para a tela: na edição, senha ausente mantém a
//! gravada.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use super::{reach::reach, DatabaseEngine, DatabaseTarget, Probe, SslMode};
use crate::{
    models::{database_connections, probes, storage_destinations},
    services::shared::{
        backup_schedule::validate_policy,
        crypto,
        errors::{AppError, AppResult},
    },
};

/// O formulário de criação e edição.
#[derive(Clone)]
pub struct ConnectionInput {
    pub name: String,
    pub engine: DatabaseEngine,
    pub host: String,
    pub port: i32,
    pub username: String,
    /// `None` na edição mantém a senha gravada.
    pub password: Option<String>,
    pub ssl_mode: SslMode,
    /// Bancos a copiar; vazio é "todos", inclusive os criados depois.
    pub databases: Vec<String>,
    pub storage_destination_id: Option<i64>,
    /// Agente pelo qual a central chega ao banco; `None` = direto (ADR 013).
    pub via_probe_id: Option<i64>,
    pub backup_enabled: bool,
    pub backup_interval_hours: i32,
    pub backup_retention: i32,
}

/// Uma conexão com a senha já em claro.
pub struct Connection {
    pub row: database_connections::Model,
    pub engine: DatabaseEngine,
    pub ssl_mode: SslMode,
    pub databases: Vec<String>,
    password: String,
}

impl Connection {
    /// Para onde conectar.
    #[must_use]
    pub fn target(&self) -> DatabaseTarget {
        DatabaseTarget {
            engine: self.engine,
            host: self.row.host.clone(),
            port: u16::try_from(self.row.port).unwrap_or(self.engine.default_port()),
            username: self.row.username.clone(),
            password: self.password.clone(),
            ssl_mode: self.ssl_mode,
            database: None,
            via: None,
        }
    }
}

/// Lê e decifra uma linha.
///
/// # Errors
///
/// Valor de coluna inválido ou senha que não decifra (a `ENCRYPTION_KEY`
/// mudou).
pub fn open(row: database_connections::Model) -> AppResult<Connection> {
    let password = crypto::decrypt(&row.password_encrypted).map_err(|_| {
        AppError::business_rule(format!(
            "Não foi possível ler a senha de \"{}\" — a ENCRYPTION_KEY mudou desde o cadastro? \
             Edite a conexão e informe a senha de novo.",
            row.name
        ))
    })?;
    Ok(Connection {
        engine: DatabaseEngine::parse(&row.engine)?,
        ssl_mode: SslMode::parse(&row.ssl_mode)?,
        databases: databases_of(&row),
        password,
        row,
    })
}

/// A lista de bancos gravada.
#[must_use]
pub fn databases_of(row: &database_connections::Model) -> Vec<String> {
    serde_json::from_value(row.databases.clone()).unwrap_or_default()
}

/// Todas as conexões, por nome.
///
/// # Errors
///
/// Erro do banco.
pub async fn list<C: ConnectionTrait>(db: &C) -> AppResult<Vec<database_connections::Model>> {
    Ok(database_connections::Entity::find()
        .order_by_asc(database_connections::Column::Name)
        .all(db)
        .await?)
}

/// Uma conexão pelo id.
///
/// # Errors
///
/// `404` quando não existe.
pub async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<database_connections::Model> {
    database_connections::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Conexão de banco não encontrada"))
}

/// Uma conexão pelo id, decifrada.
///
/// # Errors
///
/// `404`; ver [`open`].
pub async fn load<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<Connection> {
    open(find(db, id).await?)
}

/// Cadastra uma conexão.
///
/// # Errors
///
/// Validação, nome repetido ou armazenamento inexistente.
pub async fn create<C: ConnectionTrait>(
    db: &C,
    input: ConnectionInput,
) -> AppResult<database_connections::Model> {
    let input = normalize(db, input, None).await?;
    let password = input.password.clone().unwrap_or_default();
    let row = database_connections::ActiveModel {
        name: Set(input.name),
        engine: Set(input.engine.as_str().to_string()),
        host: Set(input.host),
        port: Set(input.port),
        username: Set(input.username),
        password_encrypted: Set(crypto::encrypt(&password)?),
        ssl_mode: Set(input.ssl_mode.as_str().to_string()),
        databases: Set(serde_json::json!(input.databases)),
        storage_destination_id: Set(input.storage_destination_id),
        via_probe_id: Set(input.via_probe_id),
        backup_enabled: Set(input.backup_enabled),
        backup_interval_hours: Set(input.backup_interval_hours),
        backup_retention: Set(input.backup_retention),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(row)
}

/// Edita uma conexão. Senha ausente mantém a gravada.
///
/// # Errors
///
/// `404`, validação, nome repetido ou armazenamento inexistente.
pub async fn update<C: ConnectionTrait>(
    db: &C,
    id: i64,
    input: ConnectionInput,
) -> AppResult<database_connections::Model> {
    let current = find(db, id).await?;
    let input = normalize(db, input, Some(id)).await?;
    let mut row: database_connections::ActiveModel = current.into();
    row.name = Set(input.name);
    row.engine = Set(input.engine.as_str().to_string());
    row.host = Set(input.host);
    row.port = Set(input.port);
    row.username = Set(input.username);
    if let Some(password) = input.password {
        row.password_encrypted = Set(crypto::encrypt(&password)?);
    }
    row.ssl_mode = Set(input.ssl_mode.as_str().to_string());
    row.databases = Set(serde_json::json!(input.databases));
    row.storage_destination_id = Set(input.storage_destination_id);
    row.via_probe_id = Set(input.via_probe_id);
    row.backup_enabled = Set(input.backup_enabled);
    row.backup_interval_hours = Set(input.backup_interval_hours);
    row.backup_retention = Set(input.backup_retention);
    Ok(row.update(db).await?)
}

/// Remove a conexão e o histórico dela. Os arquivos no armazenamento ficam.
///
/// # Errors
///
/// `404` ou erro do banco.
pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<database_connections::Model> {
    let row = find(db, id).await?;
    database_connections::Entity::delete_by_id(id)
        .exec(db)
        .await?;
    Ok(row)
}

/// O que o "Testar conexão" do formulário manda.
pub struct ProbeInput {
    /// Conexão em edição: senha ausente vem dela.
    pub id: Option<i64>,
    pub engine: DatabaseEngine,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password: Option<String>,
    pub ssl_mode: SslMode,
    /// Testar pela ponte deste agente.
    pub via_probe_id: Option<i64>,
}

/// Testa os dados do formulário. Com `id` e senha ausente, usa a gravada.
///
/// # Errors
///
/// Só falhas daqui (cadastro inexistente, senha ilegível, campo inválido); o
/// servidor que não responde volta como erro de conexão com a mensagem dele.
pub async fn probe(ctx: &loco_rs::app::AppContext, input: ProbeInput) -> AppResult<Probe> {
    let password = match (input.password, input.id) {
        (Some(password), _) => password,
        (None, Some(id)) => load(&ctx.db, id).await?.password,
        (None, None) => String::new(),
    };
    let target = DatabaseTarget {
        engine: input.engine,
        host: validate_host(&input.host)?,
        port: validate_port(input.port)?,
        username: input.username.trim().to_string(),
        password,
        ssl_mode: input.ssl_mode,
        database: None,
        via: None,
    };
    let reached = reach(ctx, target, input.via_probe_id).await?;
    Ok(input.engine.driver().probe(&reached.target).await?)
}

/// Avisa as telas abertas que a lista mudou.
pub async fn publish_updated(ctx: &loco_rs::app::AppContext) {
    if let Ok(bus) = crate::services::events::EventBus::from_context(ctx) {
        if let Err(error) = bus
            .publish(
                &ctx.db,
                "database_connections:updated",
                serde_json::json!({}),
            )
            .await
        {
            tracing::warn!(%error, "falha ao publicar database_connections:updated");
        }
    }
}

async fn normalize<C: ConnectionTrait>(
    db: &C,
    mut input: ConnectionInput,
    except: Option<i64>,
) -> AppResult<ConnectionInput> {
    input.name = input.name.trim().to_string();
    if input.name.is_empty() {
        return Err(AppError::validation("Informe um nome para a conexão"));
    }
    if input.name.chars().count() > 120 {
        return Err(AppError::validation(
            "O nome pode ter no máximo 120 caracteres",
        ));
    }
    input.host = validate_host(&input.host)?;
    validate_port(input.port)?;
    input.username = input.username.trim().to_string();
    if input.username.is_empty() {
        return Err(AppError::validation("Informe o usuário do banco"));
    }
    input.databases = normalize_databases(input.databases)?;
    validate_policy(input.backup_interval_hours, input.backup_retention)?;
    if input.backup_enabled && input.storage_destination_id.is_none() {
        return Err(AppError::validation(
            "Escolha um armazenamento para ligar o backup automático",
        ));
    }
    if let Some(probe_id) = input.via_probe_id {
        probes::Entity::find_by_id(probe_id)
            .one(db)
            .await?
            .ok_or_else(|| AppError::validation("O agente escolhido não existe mais"))?;
    }
    if let Some(destination) = input.storage_destination_id {
        storage_destinations::Entity::find_by_id(destination)
            .one(db)
            .await?
            .ok_or_else(|| AppError::validation("O armazenamento escolhido não existe mais"))?;
    }

    let mut same_name = database_connections::Entity::find()
        .filter(database_connections::Column::Name.eq(input.name.as_str()));
    if let Some(id) = except {
        same_name = same_name.filter(database_connections::Column::Id.ne(id));
    }
    if same_name.count(db).await? > 0 {
        return Err(AppError::conflict(format!(
            "Já existe uma conexão chamada \"{}\"",
            input.name
        )));
    }
    Ok(input)
}

fn validate_host(host: &str) -> AppResult<String> {
    let host = host.trim();
    if host.is_empty() || host.len() > 255 || host.chars().any(char::is_whitespace) {
        return Err(AppError::validation("Informe o endereço do servidor"));
    }
    Ok(host.to_string())
}

fn validate_port(port: i32) -> AppResult<u16> {
    u16::try_from(port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| AppError::validation("A porta deve ficar entre 1 e 65535"))
}

/// Bancos escolhidos, sem repetição nem vazio.
///
/// O nome vem da listagem do próprio servidor — pode ter espaço ou acento no
/// PostgreSQL —, então a regra aqui é só a que protege o caminho do arquivo.
fn normalize_databases(databases: Vec<String>) -> AppResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for name in databases {
        let name = name.trim().to_string();
        if name.is_empty() || out.contains(&name) {
            continue;
        }
        if name.len() > 128
            || name
                .chars()
                .any(|c| c.is_control() || c == '/' || c == '\\')
        {
            return Err(AppError::validation(format!(
                "Nome de banco inválido: \"{name}\""
            )));
        }
        out.push(name);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porta_fora_da_faixa_e_recusada() {
        assert!(validate_port(0).is_err());
        assert!(validate_port(70_000).is_err());
        assert_eq!(validate_port(5432).unwrap(), 5432);
    }

    #[test]
    fn host_com_espaco_e_recusado_e_o_resto_e_aparado() {
        assert!(validate_host("db .local").is_err());
        assert!(validate_host("  ").is_err());
        assert_eq!(validate_host(" 10.0.0.5 ").unwrap(), "10.0.0.5");
    }

    #[test]
    fn bancos_sem_repeticao_e_sem_barra() {
        assert_eq!(
            normalize_databases(vec![
                "vendas".into(),
                " vendas ".into(),
                String::new(),
                "Estoque Central".into()
            ])
            .unwrap(),
            vec!["vendas", "Estoque Central"]
        );
        assert!(normalize_databases(vec!["../etc".into()]).is_err());
    }
}
