//! Quais imagens os alvos usam (ADR 022): a versão no ar, as que o rollback
//! pode precisar e as que estão a caminho (na fila, aguardando aprovação).
//!
//! Uma regra só para a retenção do registry (o que não pode ser coletado),
//! para o apagar da tela e da API `/v2`, e para o "No ar em…" da tela.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use netmonitor_core::{
    models::{deploy_applications, deploy_runs, deploy_targets},
    shared::errors::AppResult,
};

use super::{applications::images_of, images::repository_for, RunStatus};

/// `(repositório no registry, tag)`.
pub type ImageKey = (String, String);

/// O uso das imagens pelos alvos.
#[derive(Debug, Default, Clone)]
pub struct ImagesInUse {
    /// Não pode sair: no ar, guardada para o rollback ou a caminho.
    pub protected: BTreeSet<ImageKey>,
    /// No ar agora, e em quais alvos (`Aplicação / Alvo`).
    pub live: BTreeMap<ImageKey, Vec<String>>,
}

impl ImagesInUse {
    #[must_use]
    pub fn is_protected(&self, repository: &str, tag: &str) -> bool {
        self.protected
            .contains(&(repository.to_string(), tag.to_string()))
    }

    #[must_use]
    pub fn live_in(&self, repository: &str, tag: &str) -> &[String] {
        self.live
            .get(&(repository.to_string(), tag.to_string()))
            .map_or(&[], Vec::as_slice)
    }
}

/// Monta o uso a partir dos deploys de cada alvo (lê apenas as 3 colunas
/// necessárias, sem carregar os logs de `deploy_runs`).
///
/// # Errors
///
/// Banco.
pub async fn images_in_use<C: ConnectionTrait>(db: &C) -> AppResult<ImagesInUse> {
    let mut usage = ImagesInUse::default();
    let targets = deploy_targets::Entity::find().all(db).await?;
    let applications: BTreeMap<i64, deploy_applications::Model> =
        deploy_applications::Entity::find()
            .all(db)
            .await?
            .into_iter()
            .map(|application| (application.id, application))
            .collect();
    let rows: Vec<(i64, String, String)> = deploy_runs::Entity::find()
        .select_only()
        .column(deploy_runs::Column::TargetId)
        .column(deploy_runs::Column::Status)
        .column(deploy_runs::Column::ImageTag)
        .order_by_desc(deploy_runs::Column::Id)
        .filter(deploy_runs::Column::Status.is_in([
            RunStatus::Succeeded.as_str(),
            RunStatus::Running.as_str(),
            RunStatus::Queued.as_str(),
            RunStatus::AwaitingApproval.as_str(),
            RunStatus::Scheduled.as_str(),
        ]))
        .into_tuple()
        .all(db)
        .await?;
    let mut by_target: HashMap<i64, Vec<(String, String)>> = HashMap::new();
    for (target_id, status, image_tag) in rows {
        by_target
            .entry(target_id)
            .or_default()
            .push((status, image_tag));
    }
    for target in targets {
        let Some(application) = applications.get(&target.application_id) else {
            continue;
        };
        let Some(target_runs) = by_target.get(&target.id) else {
            continue;
        };
        let images = images_of(application);
        // A versão no ar e as anteriores que o alvo guarda para o rollback.
        let window = usize::try_from(target.keep_images.max(0)).unwrap_or(3) + 1;
        let mut succeeded = 0;
        for (status_raw, image_tag) in target_runs {
            let status = RunStatus::parse(status_raw).unwrap_or(RunStatus::Failed);
            let live = status == RunStatus::Succeeded && succeeded == 0;
            if status == RunStatus::Succeeded {
                succeeded += 1;
                if succeeded > window {
                    continue;
                }
            }
            for image in &images {
                let key = (
                    repository_for(&application.slug, &image.name),
                    image_tag.clone(),
                );
                if live {
                    usage
                        .live
                        .entry(key.clone())
                        .or_default()
                        .push(format!("{} / {}", application.name, target.name));
                }
                usage.protected.insert(key);
            }
        }
    }
    Ok(usage)
}
