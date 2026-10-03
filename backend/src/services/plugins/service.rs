//! O ciclo de vida de um plugin.
//!
//! ```text
//!  importado ──► quarantine ──(revisão aceita)──► draft ──(testes ok)──► tested ──(promover)──► active
//!  criado/IA ────────────────────────────────────► draft
//!  embutido ─────────────────────────────────────────────────────────────────────────────────► active
//!  editado: importado → quarantine;  próprio → draft (testes zerados)
//! ```
//!
//! Só o operador promove a `active`, e só com os testes verdes da versão atual
//! do código (o `checksum` amarra teste e revisão ao código testado). Plugin
//! que não está ativo executa, mas com aprovação a cada acesso — ver `runs`.

use std::collections::{HashMap, HashSet};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::Value;

use super::{
    builtin,
    compat::{self, DeviceFacts},
    manifest,
    package::{self, CompatEntry, PluginPackage, MAX_SCRIPT_BYTES},
    review::{self, ai_review, ReviewReport},
    testing::{self, TestReport},
};
use crate::{
    dtos::plugins::{DevicePluginItem, PluginDetail, PluginPreview, PluginSummary},
    models::{device_plugin_installs, devices, plugins},
    services::shared::errors::{AppError, AppResult},
};

pub mod status {
    pub const QUARANTINE: &str = "quarantine";
    pub const DRAFT: &str = "draft";
    pub const TESTED: &str = "tested";
    pub const ACTIVE: &str = "active";
    pub const DISABLED: &str = "disabled";
}

pub mod source {
    pub const BUILTIN: &str = "builtin";
    pub const USER: &str = "user";
    pub const AI: &str = "ai";
    pub const IMPORTED: &str = "imported";
}

pub mod scope {
    pub const MODEL: &str = "model";
    pub const DEVICE: &str = "device";
}

fn internal(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(anyhow::anyhow!("{error}"))
}

/// Reconstrói o pacote a partir da linha.
///
/// # Errors
///
/// JSON gravado fora do formato (não deveria acontecer).
pub fn package_of(model: &plugins::Model) -> AppResult<PluginPackage> {
    Ok(PluginPackage {
        format: package::FORMAT_VERSION,
        manifest: serde_json::from_value(model.manifest.clone()).map_err(internal)?,
        script: model.script.clone(),
        usage: model.usage.clone(),
        compatibility: serde_json::from_value(model.compatibility.clone()).unwrap_or_default(),
        tests: serde_json::from_value(model.tests.clone()).unwrap_or_default(),
    })
}

fn review_of(model: &plugins::Model) -> Option<ReviewReport> {
    model
        .review
        .as_ref()
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

/// # Errors
///
/// JSON gravado fora do formato.
pub fn summary(model: &plugins::Model) -> AppResult<PluginSummary> {
    let manifest: manifest::PluginManifest =
        serde_json::from_value(model.manifest.clone()).map_err(internal)?;
    let compatibility: Vec<CompatEntry> =
        serde_json::from_value(model.compatibility.clone()).unwrap_or_default();
    let list = manifest.item_list();
    Ok(PluginSummary {
        id: model.id,
        slug: model.slug.clone(),
        name: model.name.clone(),
        version: model.version.clone(),
        description: model.description.clone(),
        scope: model.scope.clone(),
        device_id: model.device_id,
        source: model.source.clone(),
        status: model.status.clone(),
        transports: manifest.transports,
        actions: manifest.actions,
        matcher: manifest.matcher,
        list,
        surfaces: manifest.surfaces,
        settings: manifest.settings,
        fleet: manifest.fleet.map(manifest::FleetSpec::resolved),
        risk: review_of(model).map(|report| report.risk),
        last_test_at: model.last_test_at.map(|at| at.to_rfc3339()),
        last_test_ok: model.last_test_ok,
        validated_count: u32::try_from(
            compatibility
                .iter()
                .filter(|entry| entry.status == "passed")
                .count(),
        )
        .unwrap_or(u32::MAX),
        updated_at: model.updated_at.to_rfc3339(),
    })
}

/// # Errors
///
/// Plugin inexistente ou JSON fora do formato.
pub async fn detail<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<PluginDetail> {
    let model = find(db, id).await?;
    Ok(PluginDetail {
        summary: summary(&model)?,
        package: package_of(&model)?,
        review: review_of(&model),
    })
}

/// # Errors
///
/// Plugin inexistente.
pub async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<plugins::Model> {
    plugins::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Plugin não encontrado."))
}

/// Grava os embutidos que faltam e atualiza os que mudaram de código.
/// Idempotente e barato: uma consulta quando está tudo em dia.
///
/// # Errors
///
/// Erro do banco.
pub async fn ensure_builtins<C: ConnectionTrait>(db: &C) -> AppResult<()> {
    let existing = plugins::Entity::find()
        .filter(plugins::Column::Source.eq(source::BUILTIN))
        .all(db)
        .await?;
    let packages = builtin::packages();
    // Embutido que saiu do binário (renomeado ou aposentado) sai do catálogo.
    // As execuções ficam (FK `SET NULL`); as instalações vão junto.
    let retired: Vec<i64> = existing
        .iter()
        .filter(|model| {
            !packages
                .iter()
                .any(|package| package.manifest.slug == model.slug)
        })
        .map(|model| model.id)
        .collect();
    if !retired.is_empty() {
        plugins::Entity::delete_many()
            .filter(plugins::Column::Id.is_in(retired))
            .exec(db)
            .await?;
    }
    for package in packages {
        let checksum = package.checksum();
        let current = existing
            .iter()
            .find(|model| model.slug == package.manifest.slug);
        match current {
            Some(model) if model.checksum == checksum => {}
            // Nova versão do embutido: o código muda, o estado (ligado ou
            // não) é do operador.
            Some(model) => {
                let mut active = fill(model.clone().into(), &package)?;
                active.review = Set(Some(static_review(&package)?));
                active.update(db).await?;
            }
            // Nasce desligado; liga ao cadastrar um equipamento compatível
            // ([`activate_for_device`]) ou à mão.
            None => {
                let mut active = fill(<plugins::ActiveModel as Default>::default(), &package)?;
                active.scope = Set(scope::MODEL.to_owned());
                active.source = Set(source::BUILTIN.to_owned());
                active.status = Set(status::DISABLED.to_owned());
                active.auto_enable = Set(true);
                active.review = Set(Some(static_review(&package)?));
                active.last_test_ok = Set(Some(true));
                active.last_test_at = Set(Some(chrono::Utc::now().into()));
                active.insert(db).await?;
            }
        }
    }
    Ok(())
}

fn static_review(package: &PluginPackage) -> AppResult<Value> {
    serde_json::to_value(review::build_report(
        package,
        Err("revisão por IA não executada".into()),
    ))
    .map_err(internal)
}

/// Copia os campos do pacote para a linha.
fn fill(
    mut active: plugins::ActiveModel,
    package: &PluginPackage,
) -> AppResult<plugins::ActiveModel> {
    active.slug = Set(package.manifest.slug.clone());
    active.name = Set(package.manifest.name.clone());
    active.version = Set(package.manifest.version.clone());
    active.description = Set(package.manifest.description.clone());
    active.manifest = Set(serde_json::to_value(&package.manifest).map_err(internal)?);
    active.script = Set(package.script.clone());
    active.usage = Set(package.usage.clone());
    active.compatibility = Set(serde_json::to_value(&package.compatibility).map_err(internal)?);
    active.tests = Set(serde_json::to_value(&package.tests).map_err(internal)?);
    active.checksum = Set(package.checksum());
    Ok(active)
}

/// O mínimo para gravar: manifesto válido e script dentro do teto. Uso,
/// testes e o resto aparecem no relatório de teste — um rascunho da IA pode
/// ser salvo incompleto, só não pode ser promovido assim.
fn validate_for_save(package: &PluginPackage) -> AppResult<()> {
    let problems = manifest::validate(&package.manifest);
    if !problems.is_empty() {
        return Err(AppError::validation(format!(
            "Manifesto inválido: {}",
            problems.join("; ")
        )));
    }
    if package.script.len() > MAX_SCRIPT_BYTES {
        return Err(AppError::validation("O script passa do tamanho máximo."));
    }
    if package.format != package::FORMAT_VERSION {
        return Err(AppError::validation(format!(
            "Formato de pacote {} desconhecido.",
            package.format
        )));
    }
    Ok(())
}

async fn ensure_unique<C: ConnectionTrait>(
    db: &C,
    package: &PluginPackage,
    device_id: Option<i64>,
    except: Option<i64>,
) -> AppResult<()> {
    let mut query = plugins::Entity::find()
        .filter(plugins::Column::Slug.eq(package.manifest.slug.clone()))
        .filter(plugins::Column::Version.eq(package.manifest.version.clone()));
    query = match device_id {
        Some(device_id) => query.filter(plugins::Column::DeviceId.eq(device_id)),
        None => query.filter(plugins::Column::DeviceId.is_null()),
    };
    if let Some(id) = except {
        query = query.filter(plugins::Column::Id.ne(id));
    }
    if query.one(db).await?.is_some() {
        return Err(AppError::conflict(format!(
            "Já existe o plugin `{}` na versão {}. Aumente a versão ou edite o existente.",
            package.manifest.slug, package.manifest.version
        )));
    }
    Ok(())
}

async fn ensure_device<C: ConnectionTrait>(db: &C, device_id: Option<i64>) -> AppResult<()> {
    if let Some(device_id) = device_id {
        if devices::Entity::find_by_id(device_id)
            .one(db)
            .await?
            .is_none()
        {
            return Err(AppError::not_found("Dispositivo não encontrado."));
        }
    }
    Ok(())
}

/// Cria um plugin próprio (tela ou IA). Nasce `draft`.
///
/// # Errors
///
/// Manifesto inválido, conflito de versão ou dispositivo inexistente.
pub async fn create<C: ConnectionTrait>(
    db: &C,
    package: &PluginPackage,
    device_id: Option<i64>,
    origin: &str,
) -> AppResult<plugins::Model> {
    validate_for_save(package)?;
    ensure_device(db, device_id).await?;
    ensure_unique(db, package, device_id, None).await?;
    let mut active = fill(<plugins::ActiveModel as Default>::default(), package)?;
    active.scope = Set(scope_of(device_id).to_owned());
    active.device_id = Set(device_id);
    active.source = Set(origin.to_owned());
    active.status = Set(status::DRAFT.to_owned());
    active.review = Set(Some(static_review(package)?));
    let created = active.insert(db).await?;
    // Exclusivo de um equipamento só existe para ele: já nasce instalado.
    if let Some(device_id) = device_id {
        record_install(db, device_id, created.id, None).await?;
    }
    Ok(created)
}

const fn scope_of(device_id: Option<i64>) -> &'static str {
    if device_id.is_some() {
        scope::DEVICE
    } else {
        scope::MODEL
    }
}

/// Importa um pacote de fora: quarentena e revisão completa (estática + IA).
///
/// # Errors
///
/// Manifesto inválido, conflito de versão ou dispositivo inexistente.
pub async fn import<C: ConnectionTrait>(
    db: &C,
    package: &PluginPackage,
    device_id: Option<i64>,
) -> AppResult<plugins::Model> {
    validate_for_save(package)?;
    ensure_device(db, device_id).await?;
    ensure_unique(db, package, device_id, None).await?;
    let report = full_review(db, package).await;
    let mut active = fill(<plugins::ActiveModel as Default>::default(), package)?;
    active.scope = Set(scope_of(device_id).to_owned());
    active.device_id = Set(device_id);
    active.source = Set(source::IMPORTED.to_owned());
    active.status = Set(status::QUARANTINE.to_owned());
    active.review = Set(Some(serde_json::to_value(&report).map_err(internal)?));
    Ok(active.insert(db).await?)
}

async fn full_review<C: ConnectionTrait>(db: &C, package: &PluginPackage) -> ReviewReport {
    let findings = review::static_scan::scan(package);
    let ai = ai_review::review(db, package, &findings).await;
    review::build_report(package, ai)
}

/// Refaz a revisão completa (depois de configurar a IA, por exemplo).
///
/// # Errors
///
/// Plugin inexistente.
pub async fn review_again<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<plugins::Model> {
    let model = find(db, id).await?;
    let report = full_review(db, &package_of(&model)?).await;
    let mut active: plugins::ActiveModel = model.into();
    active.review = Set(Some(serde_json::to_value(&report).map_err(internal)?));
    Ok(active.update(db).await?)
}

/// Tira da quarentena, com a decisão do operador.
///
/// # Errors
///
/// Fora da quarentena, revisão de outro código, risco crítico, ou risco alto
/// (ou sem IA) sem a declaração de revisão.
pub async fn accept_review<C: ConnectionTrait>(
    db: &C,
    id: i64,
    acknowledge_risk: bool,
) -> AppResult<plugins::Model> {
    let model = find(db, id).await?;
    if model.status != status::QUARANTINE {
        return Err(AppError::business_rule("O plugin não está em quarentena."));
    }
    let report = review_of(&model)
        .ok_or_else(|| AppError::business_rule("Rode a revisão de segurança antes."))?;
    if report.checksum != model.checksum {
        return Err(AppError::business_rule(
            "O código mudou depois da revisão. Revise de novo.",
        ));
    }
    if report.blocks() {
        return Err(AppError::business_rule(
            "Risco crítico: a instalação está bloqueada. Edite o plugin para remover o problema e revise de novo.",
        ));
    }
    if report.needs_acknowledgement() && !acknowledge_risk {
        return Err(AppError::business_rule(
            "Marque que você revisou os riscos apontados para instalar este plugin.",
        ));
    }
    let mut active: plugins::ActiveModel = model.into();
    active.status = Set(status::DRAFT.to_owned());
    Ok(active.update(db).await?)
}

/// Substitui o pacote de um plugin.
///
/// # Errors
///
/// Embutido (duplique antes), manifesto inválido ou conflito de versão.
pub async fn update<C: ConnectionTrait>(
    db: &C,
    id: i64,
    package: &PluginPackage,
) -> AppResult<plugins::Model> {
    let model = find(db, id).await?;
    if model.source == source::BUILTIN {
        return Err(AppError::business_rule(
            "Plugins embutidos não são editados. Duplique-o para personalizar.",
        ));
    }
    validate_for_save(package)?;
    ensure_unique(db, package, model.device_id, Some(model.id)).await?;
    let code_changed = model.checksum != package.checksum();
    let imported = model.source == source::IMPORTED;
    let previous_status = model.status.clone();
    let mut active = fill(model.into(), package)?;
    if code_changed {
        active.last_test_at = Set(None);
        active.last_test_ok = Set(None);
        if imported {
            // Código de fora editado volta para a quarentena: a revisão
            // anterior não vale para o código novo.
            let mut report = review::build_report(
                package,
                Err("revisão pendente — rode \"Revisar\" para a análise por IA".into()),
            );
            report.ai = None;
            active.review = Set(Some(serde_json::to_value(&report).map_err(internal)?));
            active.status = Set(status::QUARANTINE.to_owned());
        } else {
            active.review = Set(Some(static_review(package)?));
            if previous_status != status::DISABLED {
                active.status = Set(status::DRAFT.to_owned());
            }
        }
    }
    Ok(active.update(db).await?)
}

/// # Errors
///
/// Embutido ou inexistente.
pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<plugins::Model> {
    let model = find(db, id).await?;
    if model.source == source::BUILTIN {
        return Err(AppError::business_rule(
            "Plugins embutidos não podem ser excluídos; desative-o se não quiser usá-lo.",
        ));
    }
    plugins::Entity::delete_by_id(id).exec(db).await?;
    Ok(model)
}

/// Roda os testes unitários e registra o resultado.
///
/// # Errors
///
/// Plugin inexistente ou erro do banco.
pub async fn run_tests<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> AppResult<(plugins::Model, TestReport)> {
    let model = find(db, id).await?;
    let package = package_of(&model)?;
    let library = super::runs::load_library(db, &package.manifest.uses).await?;
    let report = testing::run_unit_tests_with(&package, &library).await;
    let current = model.status.clone();
    let mut active: plugins::ActiveModel = model.into();
    active.last_test_at = Set(Some(chrono::Utc::now().into()));
    active.last_test_ok = Set(Some(report.passed));
    if current == status::DRAFT && report.passed {
        active.status = Set(status::TESTED.to_owned());
    } else if current == status::TESTED && !report.passed {
        active.status = Set(status::DRAFT.to_owned());
    }
    Ok((active.update(db).await?, report))
}

/// A prévia do editor: roda os testes unitários de um pacote **sem gravar
/// nada** — nem o plugin, nem o resultado, nem o status. As saídas dos testes
/// alimentam a tela simulada; as sugestões de usabilidade vêm junto.
///
/// # Errors
///
/// Plugin citado em `uses` que não está instalado.
pub async fn preview<C: ConnectionTrait>(
    db: &C,
    package: &PluginPackage,
) -> AppResult<PluginPreview> {
    let library = super::runs::load_library(db, &package.manifest.uses).await?;
    Ok(PluginPreview {
        report: testing::run_unit_tests_with(package, &library).await,
        list: package.manifest.item_list(),
        fleet_list: package
            .manifest
            .fleet
            .as_ref()
            .and_then(manifest::FleetSpec::item_list),
    })
}

/// `tested` → `active`.
///
/// # Errors
///
/// Testes não aprovados ou revisão bloqueante.
pub async fn promote<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<plugins::Model> {
    let model = find(db, id).await?;
    if model.status != status::TESTED || model.last_test_ok != Some(true) {
        return Err(AppError::business_rule(
            "Só um plugin com os testes aprovados pode ser ativado. Rode os testes antes.",
        ));
    }
    if review_of(&model).is_some_and(|report| report.blocks()) {
        return Err(AppError::business_rule(
            "A revisão de segurança aponta risco crítico.",
        ));
    }
    let mut active: plugins::ActiveModel = model.into();
    active.status = Set(status::ACTIVE.to_owned());
    Ok(active.update(db).await?)
}

/// Liga ou desliga. Religar volta para `tested`/`draft` conforme os testes.
///
/// # Errors
///
/// Plugin inexistente.
pub async fn set_enabled<C: ConnectionTrait>(
    db: &C,
    id: i64,
    enabled: bool,
) -> AppResult<plugins::Model> {
    let model = find(db, id).await?;
    let next = match (enabled, model.source.as_str(), model.last_test_ok) {
        (false, ..) => status::DISABLED,
        (true, source::BUILTIN, _) => status::ACTIVE,
        (true, _, Some(true)) => status::TESTED,
        (true, _, _) => status::DRAFT,
    };
    if enabled && model.status != status::DISABLED {
        return Ok(model);
    }
    let mut active: plugins::ActiveModel = model.into();
    active.status = Set(next.to_owned());
    if !enabled {
        // Desligado pelo operador: não volta sozinho no próximo cadastro.
        active.auto_enable = Set(false);
    }
    Ok(active.update(db).await?)
}

/// Liga os plugins desligados que ainda podem ligar sozinhos e que dizem,
/// pelo sistema, ser para este equipamento recém-cadastrado (ou alterado).
/// Devolve os nomes ligados.
///
/// # Errors
///
/// Erro do banco.
pub async fn activate_for_device<C: ConnectionTrait>(
    db: &C,
    device: &devices::Model,
) -> AppResult<Vec<String>> {
    ensure_builtins(db).await?;
    let facts = DeviceFacts::from_device(device);
    let dormant = plugins::Entity::find()
        .filter(plugins::Column::Status.eq(status::DISABLED))
        .filter(plugins::Column::AutoEnable.eq(true))
        .filter(plugins::Column::DeviceId.is_null())
        .all(db)
        .await?;
    let mut activated = Vec::new();
    for model in dormant {
        let package = package_of(&model)?;
        let fits = compat::names_platform(&package.manifest, &facts)
            && compat::evaluate(&package.manifest, &package.compatibility, &facts).level
                != compat::Compat::Incompatible;
        if fits {
            let name = model.name.clone();
            set_enabled(db, model.id, true).await?;
            activated.push(name);
        }
    }
    Ok(activated)
}

/// Copia um plugin (embutido, geralmente) como próprio, para editar.
///
/// # Errors
///
/// Plugin inexistente.
pub async fn duplicate<C: ConnectionTrait>(
    db: &C,
    id: i64,
    device_id: Option<i64>,
) -> AppResult<plugins::Model> {
    let model = find(db, id).await?;
    let mut package = package_of(&model)?;
    let base = format!("{}-custom", model.slug.trim_end_matches("-custom"));
    let taken: HashSet<String> = plugins::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|plugin| plugin.slug)
        .collect();
    package.manifest.slug = (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            }
        })
        .find(|slug| slug.len() <= 64 && !taken.contains(slug))
        .unwrap_or(base);
    package.manifest.name = format!("{} (cópia)", package.manifest.name);
    create(db, &package, device_id.or(model.device_id), source::USER).await
}

/// Todos os plugins, ordenados por nome.
///
/// # Errors
///
/// Erro do banco.
pub async fn list<C: ConnectionTrait>(db: &C) -> AppResult<Vec<PluginSummary>> {
    ensure_builtins(db).await?;
    plugins::Entity::find()
        .order_by_asc(plugins::Column::Name)
        .all(db)
        .await?
        .iter()
        .map(summary)
        .collect()
}

/// Os plugins que se aplicam a um equipamento: os de modelo (fora os
/// desativados) e os exclusivos dele, do mais para o menos compatível.
///
/// # Errors
///
/// Erro do banco.
pub async fn for_device<C: ConnectionTrait>(
    db: &C,
    device: &devices::Model,
) -> AppResult<Vec<DevicePluginItem>> {
    ensure_builtins(db).await?;
    let facts = DeviceFacts::from_device(device);
    let rows = plugins::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(plugins::Column::DeviceId.is_null())
                .add(plugins::Column::DeviceId.eq(device.id)),
        )
        .order_by_asc(plugins::Column::Name)
        .all(db)
        .await?;
    let installs: HashMap<i64, String> = device_plugin_installs::Entity::find()
        .filter(device_plugin_installs::Column::DeviceId.eq(device.id))
        .all(db)
        .await?
        .into_iter()
        .map(|install| (install.plugin_id, install.created_at.to_rfc3339()))
        .collect();
    let mut items = Vec::with_capacity(rows.len());
    for model in &rows {
        let package = package_of(model)?;
        let verdict = if model.device_id == Some(device.id) {
            compat::Verdict {
                level: compat::Compat::Validated,
                reasons: vec!["exclusivo deste equipamento".to_owned()],
            }
        } else {
            compat::evaluate(&package.manifest, &package.compatibility, &facts)
        };
        // Desligado só aparece onde serviria ("Ativar e instalar").
        if model.status == status::DISABLED && verdict.level == compat::Compat::Incompatible {
            continue;
        }
        items.push(DevicePluginItem {
            plugin: summary(model)?,
            compat: verdict.level,
            reasons: verdict.reasons,
            installed: installs.contains_key(&model.id),
            installed_at: installs.get(&model.id).cloned(),
            device_settings: if installs.contains_key(&model.id) {
                super::settings::view(db, model, super::settings::Scope::Device(device.id)).await?
            } else {
                None
            },
        });
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.compat));
    Ok(items)
}

async fn record_install<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    plugin_id: i64,
    user_id: Option<i64>,
) -> AppResult<()> {
    let exists = device_plugin_installs::Entity::find()
        .filter(device_plugin_installs::Column::DeviceId.eq(device_id))
        .filter(device_plugin_installs::Column::PluginId.eq(plugin_id))
        .one(db)
        .await?
        .is_some();
    if !exists {
        device_plugin_installs::ActiveModel {
            device_id: Set(device_id),
            plugin_id: Set(plugin_id),
            user_id: Set(user_id),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// Se o plugin está instalado no equipamento.
///
/// # Errors
///
/// Erro do banco.
pub async fn is_installed<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    plugin_id: i64,
) -> AppResult<bool> {
    Ok(device_plugin_installs::Entity::find()
        .filter(device_plugin_installs::Column::DeviceId.eq(device_id))
        .filter(device_plugin_installs::Column::PluginId.eq(plugin_id))
        .one(db)
        .await?
        .is_some())
}

/// Instala o plugin no equipamento — dá a ele uma aba. Não acessa o
/// equipamento; é a escolha do operador. Idempotente.
///
/// # Errors
///
/// Plugin em quarentena ou desativado, exclusivo de outro equipamento ou
/// incompatível (com o porquê).
pub async fn install<C: ConnectionTrait>(
    db: &C,
    device: &devices::Model,
    plugin_id: i64,
    user_id: Option<i64>,
) -> AppResult<plugins::Model> {
    let model = find(db, plugin_id).await?;
    if model.status == status::QUARANTINE {
        return Err(AppError::business_rule(
            "O plugin está em quarentena: aceite a revisão de segurança antes de instalá-lo.",
        ));
    }
    if model.device_id.is_some_and(|owner| owner != device.id) {
        return Err(AppError::business_rule(
            "Este plugin é exclusivo de outro equipamento.",
        ));
    }
    if model.device_id.is_none() {
        let package = package_of(&model)?;
        let verdict = compat::evaluate(
            &package.manifest,
            &package.compatibility,
            &DeviceFacts::from_device(device),
        );
        if verdict.level == compat::Compat::Incompatible {
            return Err(AppError::business_rule(format!(
                "Plugin incompatível com este equipamento: {}.",
                verdict.reasons.join("; ")
            )));
        }
    }
    // "Ativar e instalar": instalar um plugin desligado é a decisão de usá-lo.
    let model = if model.status == status::DISABLED {
        set_enabled(db, model.id, true).await?
    } else {
        model
    };
    record_install(db, device.id, model.id, user_id).await?;
    Ok(model)
}

/// Tira a aba do plugin do equipamento. O histórico de execuções fica.
///
/// # Errors
///
/// Plugin inexistente ou erro do banco.
pub async fn uninstall<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    plugin_id: i64,
) -> AppResult<plugins::Model> {
    let model = find(db, plugin_id).await?;
    device_plugin_installs::Entity::delete_many()
        .filter(device_plugin_installs::Column::DeviceId.eq(device_id))
        .filter(device_plugin_installs::Column::PluginId.eq(plugin_id))
        .exec(db)
        .await?;
    Ok(model)
}

/// Grava o resultado de uma validação funcional na lista de compatibilidade,
/// substituindo a entrada anterior do mesmo sistema/modelo/firmware.
///
/// # Errors
///
/// Plugin inexistente ou erro do banco.
pub async fn record_compatibility<C: ConnectionTrait>(
    db: &C,
    id: i64,
    entry: CompatEntry,
) -> AppResult<plugins::Model> {
    let model = find(db, id).await?;
    let mut list: Vec<CompatEntry> =
        serde_json::from_value(model.compatibility.clone()).unwrap_or_default();
    let same = |a: &Option<String>, b: &Option<String>| {
        a.as_deref().map(str::to_lowercase) == b.as_deref().map(str::to_lowercase)
    };
    list.retain(|existing| {
        !(same(&existing.platform, &entry.platform)
            && same(&existing.model, &entry.model)
            && same(&existing.firmware, &entry.firmware))
    });
    list.push(entry);
    let mut active: plugins::ActiveModel = model.into();
    active.compatibility = Set(serde_json::to_value(&list).map_err(internal)?);
    Ok(active.update(db).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{Database, DatabaseConnection};
    use serde_json::json;

    async fn db() -> DatabaseConnection {
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:".to_owned());
        options.max_connections(1).min_connections(1);
        let db = Database::connect(options).await.expect("banco");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    fn package(version: &str, script: &str) -> PluginPackage {
        serde_json::from_value(json!({
            "manifest": {
                "slug": "meu-plugin", "name": "Meu plugin", "version": version,
                "transports": ["ssh"],
                "actions": [ { "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" } ]
            },
            "script": script,
            "usage": "## Detectar",
            "tests": {
                "unit": [ { "action": "detect", "fixtures": [ { "ssh": "uname -r", "stdout": "6.1\n" } ],
                            "expect": { "firmware": "6.1" } } ],
                "functional": [ { "action": "detect", "expectKeys": ["firmware"] } ]
            }
        }))
        .unwrap()
    }

    const SCRIPT: &str =
        "fn detect(device, params) { #{ firmware: trimmed(device.run(\"uname -r\")) } }";

    #[tokio::test]
    async fn embutidos_sao_semeados_uma_vez() {
        let db = db().await;
        ensure_builtins(&db).await.unwrap();
        ensure_builtins(&db).await.unwrap();
        let all = list(&db).await.unwrap();
        assert_eq!(all.len(), builtin::packages().len());
        assert!(
            all.iter()
                .all(|p| p.status == status::DISABLED && p.source == source::BUILTIN),
            "embutido nasce desligado"
        );
        assert!(
            delete(&db, all[0].id).await.is_err(),
            "embutido não é excluído"
        );
    }

    #[tokio::test]
    async fn ciclo_draft_testado_ativo() {
        let db = db().await;
        let created = create(&db, &package("1.0.0", SCRIPT), None, source::USER)
            .await
            .unwrap();
        assert_eq!(created.status, status::DRAFT);
        assert!(
            promote(&db, created.id).await.is_err(),
            "sem teste não ativa"
        );
        let (tested, report) = run_tests(&db, created.id).await.unwrap();
        assert!(report.passed, "{report:?}");
        assert_eq!(tested.status, status::TESTED);
        assert_eq!(
            promote(&db, created.id).await.unwrap().status,
            status::ACTIVE
        );

        // Editar o código zera os testes e volta para rascunho.
        let edited = update(
            &db,
            created.id,
            &package("1.0.0", &format!("{SCRIPT}\n// v2")),
        )
        .await
        .unwrap();
        assert_eq!(edited.status, status::DRAFT);
        assert_eq!(edited.last_test_ok, None);
    }

    #[tokio::test]
    async fn versao_repetida_e_conflito() {
        let db = db().await;
        create(&db, &package("1.0.0", SCRIPT), None, source::USER)
            .await
            .unwrap();
        let error = create(&db, &package("1.0.0", SCRIPT), None, source::USER)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Conflict(_)));
        assert!(create(&db, &package("1.1.0", SCRIPT), None, source::USER)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn importado_fica_em_quarentena_e_critico_nao_sai() {
        let db = db().await;
        let perigoso = package(
            "1.0.0",
            "fn detect(device, params) { device.run(\"sysupgrade -n /tmp/fw\"); #{} }",
        );
        let imported = import(&db, &perigoso, None).await.unwrap();
        assert_eq!(imported.status, status::QUARANTINE);
        let error = accept_review(&db, imported.id, true).await.unwrap_err();
        assert!(error.to_string().contains("crítico"));
    }

    #[tokio::test]
    async fn importado_limpo_sai_da_quarentena_com_declaracao() {
        let db = db().await;
        let imported = import(&db, &package("1.0.0", SCRIPT), None).await.unwrap();
        // IA desligada no teste: a revisão é só estática e pede declaração.
        assert!(accept_review(&db, imported.id, false).await.is_err());
        let accepted = accept_review(&db, imported.id, true).await.unwrap();
        assert_eq!(accepted.status, status::DRAFT);

        // Editar código importado devolve à quarentena.
        let edited = update(
            &db,
            imported.id,
            &package("1.0.0", &format!("{SCRIPT}\n// mudou")),
        )
        .await
        .unwrap();
        assert_eq!(edited.status, status::QUARANTINE);
    }

    async fn openwrt(db: &DatabaseConnection, firmware: &str) -> devices::Model {
        devices::ActiveModel {
            name: Set("Roteador".into()),
            r#type: Set("router".into()),
            status: Set("online".into()),
            ip_address: Set(Some("192.168.1.1".into())),
            operating_system: Set(Some("openwrt".into())),
            firmware_version: Set(Some(firmware.into())),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn instalar_da_aba_e_desinstalar_tira() {
        let db = db().await;
        ensure_builtins(&db).await.unwrap();
        let device = openwrt(&db, "24.10.0").await;
        let pacotes = list(&db)
            .await
            .unwrap()
            .into_iter()
            .find(|plugin| plugin.slug == "openwrt-packages")
            .expect("gerenciador de pacotes embutido");

        install(&db, &device, pacotes.id, None).await.unwrap();
        install(&db, &device, pacotes.id, None).await.unwrap();
        let item = for_device(&db, &device)
            .await
            .unwrap()
            .into_iter()
            .find(|item| item.plugin.id == pacotes.id)
            .unwrap();
        assert!(item.installed);
        assert!(item.plugin.list.is_some(), "o gerenciador tem tela própria");

        uninstall(&db, device.id, pacotes.id).await.unwrap();
        assert!(!is_installed(&db, device.id, pacotes.id).await.unwrap());
    }

    fn slug_status(all: &[PluginSummary], slug: &str) -> String {
        all.iter()
            .find(|plugin| plugin.slug == slug)
            .map(|plugin| plugin.status.clone())
            .unwrap()
    }

    #[tokio::test]
    async fn cadastro_compativel_liga_e_desligado_a_mao_fica_desligado() {
        let db = db().await;
        ensure_builtins(&db).await.unwrap();
        let device = openwrt(&db, "SNAPSHOT").await;
        let ligados = activate_for_device(&db, &device).await.unwrap();
        assert_eq!(ligados.len(), 3, "{ligados:?}");
        let all = list(&db).await.unwrap();
        assert_eq!(slug_status(&all, "openwrt-packages"), status::ACTIVE);
        assert_eq!(slug_status(&all, "openwrt-wifi"), status::ACTIVE);
        // O Linux genérico cita o OpenWrt entre os sistemas dele.
        assert_eq!(slug_status(&all, "linux-ssh-status"), status::ACTIVE);
        // Sem regra de sistema (serve a qualquer página web): só à mão.
        assert_eq!(slug_status(&all, "http-page-info"), status::DISABLED);

        let wifi = all
            .iter()
            .find(|plugin| plugin.slug == "openwrt-wifi")
            .unwrap();
        set_enabled(&db, wifi.id, false).await.unwrap();
        assert!(activate_for_device(&db, &device).await.unwrap().is_empty());
        assert_eq!(
            slug_status(&list(&db).await.unwrap(), "openwrt-wifi"),
            status::DISABLED
        );
    }

    #[tokio::test]
    async fn instalar_desligado_liga_e_catalogo_esconde_o_incompativel() {
        let db = db().await;
        ensure_builtins(&db).await.unwrap();
        let mut mikrotik = openwrt(&db, "7.15").await;
        mikrotik.operating_system = Some("routeros".into());
        let slugs: Vec<String> = for_device(&db, &mikrotik)
            .await
            .unwrap()
            .into_iter()
            .map(|item| item.plugin.slug)
            .collect();
        assert!(!slugs.contains(&"openwrt-packages".to_owned()), "{slugs:?}");
        assert!(slugs.contains(&"http-page-info".to_owned()), "{slugs:?}");

        let device = openwrt(&db, "24.10.0").await;
        let catalogo = for_device(&db, &device).await.unwrap();
        let pacotes = catalogo
            .iter()
            .find(|item| item.plugin.slug == "openwrt-packages")
            .expect("desligado e compatível aparece para ativar");
        assert_eq!(pacotes.plugin.status, status::DISABLED);
        let instalado = install(&db, &device, pacotes.plugin.id, None)
            .await
            .unwrap();
        assert_eq!(instalado.status, status::ACTIVE);
    }

    #[tokio::test]
    async fn incompativel_nao_instala() {
        let db = db().await;
        ensure_builtins(&db).await.unwrap();
        let mut device = openwrt(&db, "24.10.0").await;
        device.operating_system = Some("routeros".into());
        let pacotes = list(&db)
            .await
            .unwrap()
            .into_iter()
            .find(|plugin| plugin.slug == "openwrt-packages")
            .unwrap();
        let error = install(&db, &device, pacotes.id, None).await.unwrap_err();
        assert!(error.to_string().contains("incompatível"), "{error}");
    }

    #[tokio::test]
    async fn exclusivo_nasce_instalado() {
        let db = db().await;
        let device = openwrt(&db, "24.10.0").await;
        let created = create(
            &db,
            &package("1.0.0", SCRIPT),
            Some(device.id),
            source::USER,
        )
        .await
        .unwrap();
        assert!(is_installed(&db, device.id, created.id).await.unwrap());
    }

    #[tokio::test]
    async fn embutido_aposentado_sai_do_catalogo() {
        let db = db().await;
        let mut antigo = fill(
            <plugins::ActiveModel as Default>::default(),
            &package("1.0.0", SCRIPT),
        )
        .unwrap();
        antigo.slug = Set("openwrt-opkg".into());
        antigo.scope = Set(scope::MODEL.into());
        antigo.source = Set(source::BUILTIN.into());
        antigo.status = Set(status::ACTIVE.into());
        antigo.insert(&db).await.unwrap();
        ensure_builtins(&db).await.unwrap();
        assert!(list(&db)
            .await
            .unwrap()
            .iter()
            .all(|plugin| plugin.slug != "openwrt-opkg"));
    }

    #[tokio::test]
    async fn duplicar_embutido_gera_copia_editavel() {
        let db = db().await;
        ensure_builtins(&db).await.unwrap();
        let original = list(&db).await.unwrap().remove(0);
        let copy = duplicate(&db, original.id, None).await.unwrap();
        assert_eq!(copy.source, source::USER);
        assert!(copy.slug.ends_with("-custom"));
        assert_eq!(copy.status, status::DRAFT);
    }
}
