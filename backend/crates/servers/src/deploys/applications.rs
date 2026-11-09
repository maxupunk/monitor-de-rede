//! Aplicações e alvos: o cadastro e as regras que impedem um alvo de nascer
//! inválido (e só falhar no primeiro deploy).

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set, TransactionSession, TransactionTrait,
};
use serde_json::json;

use agent_protocol::docker::stacks::{
    validate_project, validate_relative_path, validate_variable, MAX_KEEP_IMAGES,
    MAX_WAIT_TIMEOUT_SECS,
};
use netmonitor_core::{
    agents::identity,
    models::{database_connections, deploy_applications, deploy_targets, probes},
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

use super::{
    connections, credentials, env_versions, oidc, refs,
    window::Window,
    workflow::{self, WorkflowSpec},
    Rollout,
};
use crate::dtos::deploys::{ApplicationInput, BuildImage, SetupInput, TargetInput};

/// O menor `--wait-timeout` que faz sentido: a API precisa subir.
pub const MIN_WAIT_TIMEOUT_SECS: i32 = 10;

/// `https://github.com/dono/repo.git`.
#[must_use]
pub fn repository_url(repository: &str) -> String {
    format!("https://github.com/{repository}.git")
}

/// `dono/repo`, com os caracteres que o GitHub aceita.
///
/// # Errors
///
/// `Validation` fora do formato.
pub fn validate_repository(repository: &str) -> AppResult<()> {
    let part = |value: &str| {
        !value.is_empty()
            && value.len() <= 100
            && value != "."
            && value != ".."
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    match repository.split_once('/') {
        Some((owner, name)) if part(owner) && part(name) => Ok(()),
        _ => Err(AppError::validation(
            "Repositório: informe dono/repo, como aparece no endereço do GitHub",
        )),
    }
}

fn validate_image(image: &BuildImage) -> AppResult<()> {
    let name_ok = !image.name.is_empty()
        && image.name.len() <= 128
        && image.name.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.' | '/')
        })
        && !image.name.starts_with(['-', '.', '/']);
    if !name_ok {
        return Err(AppError::validation(format!(
            "Imagem \"{}\": use letras minúsculas, números, '-', '_' e '.'",
            image.name
        )));
    }
    validate_relative_path(&image.dockerfile).map_err(|_| {
        AppError::validation(format!("Dockerfile inválido: \"{}\"", image.dockerfile))
    })?;
    if image.context != "." {
        validate_relative_path(&image.context).map_err(|_| {
            AppError::validation(format!("Contexto de build inválido: \"{}\"", image.context))
        })?;
    }
    Ok(())
}

fn protocol(error: &agent_protocol::docker::DockerError) -> AppError {
    AppError::validation(error.to_string())
}

/// Confere a aplicação e devolve os campos normalizados.
fn checked(input: &ApplicationInput) -> AppResult<ApplicationInput> {
    let mut input = input.clone();
    input.name = input.name.trim().to_string();
    input.slug = input.slug.trim().to_lowercase();
    input.repository = input.repository.trim().trim_end_matches(".git").to_string();
    input.image_tag_variable = input.image_tag_variable.trim().to_string();
    if input.name.is_empty() || input.name.len() > 120 {
        return Err(AppError::validation(
            "Informe um nome de até 120 caracteres",
        ));
    }
    validate_project(&input.slug).map_err(|error| protocol(&error))?;
    validate_repository(&input.repository)?;
    input.compose_files = input
        .compose_files
        .iter()
        .map(|file| file.trim().to_string())
        .filter(|file| !file.is_empty())
        .collect();
    if input.compose_files.is_empty() {
        return Err(AppError::validation(
            "Informe ao menos um arquivo compose (ex.: docker-compose.yml)",
        ));
    }
    for file in &input.compose_files {
        validate_relative_path(file).map_err(|error| protocol(&error))?;
    }
    validate_variable(&input.image_tag_variable).map_err(|error| protocol(&error))?;
    for image in &input.images {
        validate_image(image)?;
    }
    Ok(input)
}

/// # Errors
///
/// Banco.
pub async fn list<C: ConnectionTrait>(db: &C) -> AppResult<Vec<deploy_applications::Model>> {
    Ok(deploy_applications::Entity::find()
        .order_by_asc(deploy_applications::Column::Name)
        .all(db)
        .await?)
}

/// # Errors
///
/// `404`.
pub async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<deploy_applications::Model> {
    deploy_applications::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Aplicação não encontrada"))
}

/// As aplicações de um repositório do GitHub (o `repository` do OIDC).
///
/// # Errors
///
/// Banco.
pub async fn by_repository<C: ConnectionTrait>(
    db: &C,
    repository: &str,
) -> AppResult<Vec<deploy_applications::Model>> {
    // O GitHub não diferencia caixa no nome; a comparação aqui também não.
    let wanted = repository.to_lowercase();
    Ok(list(db)
        .await?
        .into_iter()
        .filter(|app| app.repository.to_lowercase() == wanted)
        .collect())
}

/// Imagens compiladas pelo CI, como gravadas.
#[must_use]
pub fn images_of(app: &deploy_applications::Model) -> Vec<BuildImage> {
    serde_json::from_value(app.images.clone()).unwrap_or_default()
}

/// Arquivos compose, como gravados.
#[must_use]
pub fn compose_files_of(app: &deploy_applications::Model) -> Vec<String> {
    serde_json::from_value(app.compose_files.clone()).unwrap_or_default()
}

/// # Errors
///
/// `Validation`; `409` com slug repetido; credencial inexistente.
pub async fn create<C: ConnectionTrait>(
    db: &C,
    input: &ApplicationInput,
) -> AppResult<deploy_applications::Model> {
    let input = checked(input)?;
    ensure_unique_slug(db, &input.slug, None).await?;
    ensure_sources(db, &input).await?;
    Ok(deploy_applications::ActiveModel {
        name: Set(input.name),
        slug: Set(input.slug),
        repository: Set(input.repository),
        credential_id: Set(input.credential_id),
        github_connection_id: Set(input.github_connection_id),
        compose_files: Set(json!(input.compose_files)),
        image_tag_variable: Set(input.image_tag_variable),
        images: Set(json!(input.images)),
        rollout: Set(input.rollout.as_str().to_string()),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// # Errors
///
/// `404`, `Validation`, `409`.
pub async fn update<C: ConnectionTrait>(
    db: &C,
    id: i64,
    input: &ApplicationInput,
) -> AppResult<deploy_applications::Model> {
    let row = find(db, id).await?;
    let input = checked(input)?;
    ensure_unique_slug(db, &input.slug, Some(id)).await?;
    ensure_sources(db, &input).await?;
    let mut active: deploy_applications::ActiveModel = row.into();
    active.name = Set(input.name);
    active.slug = Set(input.slug);
    active.repository = Set(input.repository);
    active.credential_id = Set(input.credential_id);
    active.github_connection_id = Set(input.github_connection_id);
    active.compose_files = Set(json!(input.compose_files));
    active.image_tag_variable = Set(input.image_tag_variable);
    active.images = Set(json!(input.images));
    active.rollout = Set(input.rollout.as_str().to_string());
    Ok(active.update(db).await?)
}

/// A conexão e o token escolhidos existem.
async fn ensure_sources<C: ConnectionTrait>(db: &C, input: &ApplicationInput) -> AppResult<()> {
    if let Some(id) = input.credential_id {
        credentials::find(db, id).await?;
    }
    if let Some(id) = input.github_connection_id {
        connections::find(db, id).await?;
    }
    Ok(())
}

/// O workflow sugerido para a aplicação, com os filtros dos alvos dela.
///
/// # Errors
///
/// Banco.
pub async fn workflow_yaml<C: ConnectionTrait>(
    db: &C,
    application: &deploy_applications::Model,
    central_url: Option<&str>,
) -> AppResult<String> {
    let references: Vec<String> = targets_of(db, application.id)
        .await?
        .into_iter()
        .map(|target| target.reference)
        .collect();
    let images = images_of(application);
    Ok(workflow::render(&WorkflowSpec {
        application_name: &application.name,
        images: &images,
        references: &references,
        audience: &oidc::audience(),
        central_url,
    }))
}

/// Apaga a aplicação, os alvos e o histórico. Os containers continuam no
/// ar: só o deploy automático deixa de existir.
///
/// # Errors
///
/// `404`.
pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<deploy_applications::Model> {
    let row = find(db, id).await?;
    deploy_applications::Entity::delete_by_id(id)
        .exec(db)
        .await?;
    Ok(row)
}

async fn ensure_unique_slug<C: ConnectionTrait>(
    db: &C,
    slug: &str,
    except: Option<i64>,
) -> AppResult<()> {
    let mut query =
        deploy_applications::Entity::find().filter(deploy_applications::Column::Slug.eq(slug));
    if let Some(id) = except {
        query = query.filter(deploy_applications::Column::Id.ne(id));
    }
    if query.count(db).await? > 0 {
        return Err(AppError::conflict(format!(
            "Já existe uma aplicação com o identificador \"{slug}\""
        )));
    }
    Ok(())
}

/// # Errors
///
/// Banco.
pub async fn targets_of<C: ConnectionTrait>(
    db: &C,
    application_id: i64,
) -> AppResult<Vec<deploy_targets::Model>> {
    Ok(deploy_targets::Entity::find()
        .filter(deploy_targets::Column::ApplicationId.eq(application_id))
        .order_by_asc(deploy_targets::Column::Position)
        .order_by_asc(deploy_targets::Column::Id)
        .all(db)
        .await?)
}

/// # Errors
///
/// Banco.
pub async fn all_targets<C: ConnectionTrait>(db: &C) -> AppResult<Vec<deploy_targets::Model>> {
    Ok(deploy_targets::Entity::find()
        .order_by_asc(deploy_targets::Column::Position)
        .order_by_asc(deploy_targets::Column::Id)
        .all(db)
        .await?)
}

/// # Errors
///
/// `404`.
pub async fn find_target<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<deploy_targets::Model> {
    deploy_targets::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Alvo de deploy não encontrado"))
}

/// O alvo normalizado e conferido contra o banco.
async fn checked_target<C: ConnectionTrait>(
    db: &C,
    application_id: i64,
    input: &TargetInput,
    except: Option<i64>,
) -> AppResult<TargetInput> {
    let mut input = input.clone();
    input.name = input.name.trim().to_string();
    input.project = input.project.trim().to_lowercase();
    input.reference = input.reference.trim().to_string();
    if input.name.is_empty() || input.name.len() > 120 {
        return Err(AppError::validation(
            "Dê um nome ao alvo (ex.: Produção, Homologação)",
        ));
    }
    validate_project(&input.project).map_err(|error| protocol(&error))?;
    refs::validate_filter(&input.reference)?;
    let window = Window::from_columns(input.window_start.as_deref(), input.window_end.as_deref())?;
    input.window_start = window.map(|window| window.start.format("%H:%M").to_string());
    input.window_end = window.map(|window| window.end.format("%H:%M").to_string());
    let max_wait = i32::try_from(MAX_WAIT_TIMEOUT_SECS).unwrap_or(i32::MAX);
    if !(MIN_WAIT_TIMEOUT_SECS..=max_wait).contains(&input.wait_timeout_secs) {
        return Err(AppError::validation(format!(
            "O tempo de espera dos healthchecks vai de {MIN_WAIT_TIMEOUT_SECS} a {max_wait} \
             segundos"
        )));
    }
    let max_keep = i32::try_from(MAX_KEEP_IMAGES).unwrap_or(i32::MAX);
    if !(0..=max_keep).contains(&input.keep_images) {
        return Err(AppError::validation(format!(
            "Mantenha de 0 a {max_keep} versões antigas das imagens"
        )));
    }

    let agent = probes::Entity::find_by_id(input.probe_id)
        .one(db)
        .await?
        .filter(identity::is_agent)
        .ok_or_else(|| AppError::validation("Escolha um agente cadastrado"))?;
    if agent.status == probes::STATUS_REVOKED {
        return Err(AppError::validation(format!(
            "O agente \"{}\" foi revogado",
            agent.name
        )));
    }
    if let Some(connection) = input.backup_connection_id {
        database_connections::Entity::find_by_id(connection)
            .one(db)
            .await?
            .ok_or_else(|| AppError::validation("A conexão de banco escolhida não existe"))?;
    }

    let mut same_name = deploy_targets::Entity::find()
        .filter(deploy_targets::Column::ApplicationId.eq(application_id))
        .filter(deploy_targets::Column::Name.eq(input.name.clone()));
    let mut same_project = deploy_targets::Entity::find()
        .filter(deploy_targets::Column::ProbeId.eq(input.probe_id))
        .filter(deploy_targets::Column::Project.eq(input.project.clone()));
    if let Some(id) = except {
        same_name = same_name.filter(deploy_targets::Column::Id.ne(id));
        same_project = same_project.filter(deploy_targets::Column::Id.ne(id));
    }
    if same_name.count(db).await? > 0 {
        return Err(AppError::conflict(format!(
            "Esta aplicação já tem um alvo chamado \"{}\"",
            input.name
        )));
    }
    if same_project.count(db).await? > 0 {
        return Err(AppError::conflict(format!(
            "O projeto \"{}\" já é alvo de deploy em \"{}\": dois alvos brigariam pelos mesmos \
             containers",
            input.project, agent.name
        )));
    }
    Ok(input)
}

fn apply(active: &mut deploy_targets::ActiveModel, input: TargetInput) {
    active.name = Set(input.name);
    active.probe_id = Set(Some(input.probe_id));
    active.project = Set(input.project);
    active.reference = Set(input.reference);
    active.requires_approval = Set(input.requires_approval);
    active.window_start = Set(input.window_start);
    active.window_end = Set(input.window_end);
    active.backup_connection_id = Set(input.backup_connection_id);
    active.wait_timeout_secs = Set(input.wait_timeout_secs);
    active.auto_rollback = Set(input.auto_rollback);
    active.keep_images = Set(input.keep_images);
    active.enabled = Set(input.enabled);
    active.position = Set(input.position);
}

/// # Errors
///
/// `404` da aplicação, `Validation`, `409`.
pub async fn create_target<C: ConnectionTrait>(
    db: &C,
    application_id: i64,
    input: &TargetInput,
) -> AppResult<deploy_targets::Model> {
    find(db, application_id).await?;
    let input = checked_target(db, application_id, input, None).await?;
    let mut active = deploy_targets::ActiveModel {
        application_id: Set(application_id),
        ..Default::default()
    };
    apply(&mut active, input);
    Ok(active.insert(db).await?)
}

/// # Errors
///
/// `404`, `Validation`, `409`.
pub async fn update_target<C: ConnectionTrait>(
    db: &C,
    id: i64,
    input: &TargetInput,
) -> AppResult<deploy_targets::Model> {
    let row = find_target(db, id).await?;
    let input = checked_target(db, row.application_id, input, Some(id)).await?;
    let mut active: deploy_targets::ActiveModel = row.into();
    apply(&mut active, input);
    Ok(active.update(db).await?)
}

/// # Errors
///
/// `404`.
pub async fn delete_target<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> AppResult<deploy_targets::Model> {
    let row = find_target(db, id).await?;
    deploy_targets::Entity::delete_by_id(id).exec(db).await?;
    Ok(row)
}

/// O assistente numa transação só: aplicação, alvo e o primeiro `.env`.
///
/// # Errors
///
/// Qualquer recusa de [`create`], [`create_target`] ou do `.env` — e então
/// nada fica gravado.
pub async fn setup<C: TransactionTrait + ConnectionTrait>(
    db: &C,
    input: &SetupInput,
    author: Option<String>,
) -> AppResult<(deploy_applications::Model, deploy_targets::Model)> {
    let txn = begin_write(db).await?;
    let application = match (input.application_id, &input.application) {
        (Some(id), _) => find(&txn, id).await?,
        (None, Some(application)) => create(&txn, application).await?,
        (None, None) => {
            return Err(AppError::validation(
                "Escolha uma aplicação ou descreva uma nova",
            ))
        }
    };
    let target = create_target(&txn, application.id, &input.target).await?;
    if let Some(content) = input
        .env
        .as_deref()
        .filter(|content| !content.trim().is_empty())
    {
        env_versions::save(
            &txn,
            target.id,
            content,
            Some("Versão inicial".to_string()),
            author,
        )
        .await?;
    }
    txn.commit().await?;
    Ok((application, target))
}

/// A estratégia gravada.
#[must_use]
pub fn rollout_of(app: &deploy_applications::Model) -> Rollout {
    Rollout::parse(&app.rollout).unwrap_or(Rollout::Parallel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn application() -> ApplicationInput {
        ApplicationInput {
            name: " SGCv4 ".into(),
            slug: "SGC".into(),
            repository: "master-sistemas/sgcv4.git".into(),
            credential_id: None,
            github_connection_id: None,
            compose_files: vec!["docker-compose.yml".into(), " ".into()],
            image_tag_variable: "IMAGE_TAG".into(),
            images: vec![BuildImage {
                name: "sgc-api".into(),
                dockerfile: "apps/api/Dockerfile".into(),
                context: ".".into(),
            }],
            rollout: Rollout::Sequential,
        }
    }

    #[test]
    fn normaliza_a_aplicacao() {
        let checked = checked(&application()).expect("válida");
        assert_eq!(checked.name, "SGCv4");
        assert_eq!(checked.slug, "sgc");
        assert_eq!(checked.repository, "master-sistemas/sgcv4");
        assert_eq!(checked.compose_files, vec!["docker-compose.yml"]);
        assert_eq!(
            repository_url(&checked.repository),
            "https://github.com/master-sistemas/sgcv4.git"
        );
    }

    #[test]
    fn recusa_o_que_viraria_caminho_ou_url() {
        let mut bad = application();
        bad.repository = "https://github.com/a/b".into();
        assert!(checked(&bad).is_err());
        let mut bad = application();
        bad.compose_files = vec!["../x.yml".into()];
        assert!(checked(&bad).is_err());
        let mut bad = application();
        bad.images[0].name = "Sgc API".into();
        assert!(checked(&bad).is_err());
    }

    #[test]
    fn repositorio_dono_e_nome() {
        assert!(validate_repository("a/b").is_ok());
        assert!(validate_repository("a").is_err());
        assert!(validate_repository("a/b/c").is_err());
        assert!(validate_repository("a/..").is_err());
    }
}
