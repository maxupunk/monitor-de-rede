//! A configuração guardada de um plugin — o estado desejado.
//!
//! Dois escopos, um esquema cada (`manifest.settings`):
//!
//! * **frota** (`device_id` nulo) — vale para todos os equipamentos onde o
//!   plugin está instalado (os SSIDs da rede);
//! * **dispositivo** — o ajuste de um equipamento (o canal daquele rádio).
//!
//! # Segredos
//!
//! Campo `secret: true` (senha do Wi-Fi) é gravado cifrado
//! (`crypto::encrypt`) dentro do JSON, como `{"$enc": "…"}`, e volta para a
//! tela como [`SECRET_MASK`] — nunca em claro. Ao salvar, a máscara significa
//! "manter a guardada": na raiz pelo nome, numa lista de objetos pelo `id` do
//! item (gerado aqui quando falta). O valor em claro só existe no
//! [`effective`], entregue ao script na execução — e a lista de segredos vai
//! junto, para o runtime mascarar o transcript e a saída.

use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Map, Value};

use super::{
    manifest::PluginManifest,
    params::{self, for_each_secret, SecretField, ITEM_ID},
};

pub use super::params::SECRET_MASK;
use crate::{
    models::{plugin_settings, plugins},
    services::shared::{
        crypto,
        errors::{AppError, AppResult},
    },
};

const ENCRYPTED_KEY: &str = "$enc";

/// De qual configuração se trata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Fleet,
    Device(i64),
}

impl Scope {
    const fn device_id(self) -> Option<i64> {
        match self {
            Self::Fleet => None,
            Self::Device(id) => Some(id),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Fleet => "da frota",
            Self::Device(_) => "do dispositivo",
        }
    }
}

fn internal(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(anyhow::anyhow!("{error}"))
}

fn manifest_of(plugin: &plugins::Model) -> AppResult<PluginManifest> {
    serde_json::from_value(plugin.manifest.clone()).map_err(internal)
}

fn schema_of(manifest: &PluginManifest, scope: Scope) -> Option<Value> {
    let settings = manifest.settings.as_ref()?;
    match scope {
        Scope::Fleet => settings.fleet.clone(),
        Scope::Device(_) => settings.device.clone(),
    }
}

async fn row<C: ConnectionTrait>(
    db: &C,
    plugin_id: i64,
    scope: Scope,
) -> AppResult<Option<plugin_settings::Model>> {
    let mut query =
        plugin_settings::Entity::find().filter(plugin_settings::Column::PluginId.eq(plugin_id));
    query = match scope.device_id() {
        Some(device_id) => query.filter(plugin_settings::Column::DeviceId.eq(device_id)),
        None => query.filter(plugin_settings::Column::DeviceId.is_null()),
    };
    Ok(query.one(db).await?)
}

fn decrypt_in_place(value: &mut Value, fields: &[SecretField]) -> AppResult<()> {
    let mut failure = None;
    for_each_secret(value, fields, &mut |slot| {
        if let Some(cipher) = slot.get(ENCRYPTED_KEY).and_then(Value::as_str) {
            match crypto::decrypt(cipher) {
                Ok(plain) => *slot = Value::String(plain),
                Err(error) => failure = Some(error),
            }
        }
    });
    failure.map_or(Ok(()), Err)
}

fn encrypt_in_place(value: &mut Value, fields: &[SecretField]) -> AppResult<()> {
    let mut failure = None;
    for_each_secret(value, fields, &mut |slot| {
        if let Some(plain) = slot.as_str().filter(|plain| !plain.is_empty()) {
            match crypto::encrypt(plain) {
                Ok(cipher) => *slot = json!({ ENCRYPTED_KEY: cipher }),
                Err(error) => failure = Some(error),
            }
        }
    });
    failure.map_or(Ok(()), Err)
}

/// Segredo guardado vira [`SECRET_MASK`]; vazio continua vazio.
fn mask_in_place(value: &mut Value, fields: &[SecretField]) {
    for_each_secret(value, fields, &mut |slot| {
        let set = slot.get(ENCRYPTED_KEY).is_some() || slot.as_str().is_some_and(|s| !s.is_empty());
        *slot = Value::String(if set {
            SECRET_MASK.into()
        } else {
            String::new()
        });
    });
}

/// A máscara volta a ser o segredo guardado; itens novos ganham `id`.
fn restore_secrets(incoming: &mut Value, previous: &Value, schema: &Value) {
    for list in params::object_lists(Some(schema)) {
        if let Some(items) = incoming.get_mut(&list).and_then(Value::as_array_mut) {
            for item in items {
                if let Some(map) = item.as_object_mut() {
                    let has_id = map
                        .get(ITEM_ID)
                        .and_then(Value::as_str)
                        .is_some_and(|id| !id.is_empty());
                    if !has_id {
                        map.insert(
                            ITEM_ID.to_owned(),
                            Value::String(crypto::random_token()[..12].to_owned()),
                        );
                    }
                }
            }
        }
    }
    for field in params::secret_fields(Some(schema)) {
        match field {
            SecretField::Root(name) => {
                if incoming.get(&name).and_then(Value::as_str) == Some(SECRET_MASK) {
                    incoming[&name] = previous.get(&name).cloned().unwrap_or(Value::Null);
                }
            }
            SecretField::Item { list, field } => {
                let old_items = previous
                    .get(&list)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if let Some(items) = incoming.get_mut(&list).and_then(Value::as_array_mut) {
                    for item in items {
                        if item.get(&field).and_then(Value::as_str) != Some(SECRET_MASK) {
                            continue;
                        }
                        let id = item.get(ITEM_ID).cloned();
                        let kept = old_items
                            .iter()
                            .find(|old| old.get(ITEM_ID) == id.as_ref())
                            .and_then(|old| old.get(&field))
                            .cloned()
                            .unwrap_or_else(|| Value::String(String::new()));
                        item[&field] = kept;
                    }
                }
            }
        }
    }
}

/// O valor guardado, em claro, com os padrões do esquema aplicados.
async fn plain<C: ConnectionTrait>(
    db: &C,
    plugin: &plugins::Model,
    manifest: &PluginManifest,
    scope: Scope,
) -> AppResult<Value> {
    let Some(schema) = schema_of(manifest, scope) else {
        return Ok(json!({}));
    };
    let fields = params::secret_fields(Some(&schema));
    let mut value = match row(db, plugin.id, scope).await? {
        Some(model) => model.value,
        None => json!({}),
    };
    decrypt_in_place(&mut value, &fields)?;
    // Um campo novo no esquema (plugin atualizado) ganha o padrão sem que a
    // configuração antiga fique inválida.
    Ok(params::validate(Some(&schema), &value).unwrap_or(value))
}

/// A configuração para a tela: segredos mascarados.
///
/// # Errors
///
/// Erro do banco ou de decifra.
pub async fn view<C: ConnectionTrait>(
    db: &C,
    plugin: &plugins::Model,
    scope: Scope,
) -> AppResult<Option<Value>> {
    let manifest = manifest_of(plugin)?;
    let Some(schema) = schema_of(&manifest, scope) else {
        return Ok(None);
    };
    let mut value = plain(db, plugin, &manifest, scope).await?;
    mask_in_place(&mut value, &params::secret_fields(Some(&schema)));
    Ok(Some(value))
}

/// Salva a configuração vinda da tela (com máscaras) e devolve a vista.
///
/// # Errors
///
/// Plugin sem configuração nesse escopo, ou valor fora do esquema (lista de
/// problemas por campo).
pub async fn save<C: ConnectionTrait>(
    db: &C,
    plugin: &plugins::Model,
    scope: Scope,
    mut incoming: Value,
    user_id: Option<i64>,
) -> AppResult<Value> {
    let manifest = manifest_of(plugin)?;
    let schema = schema_of(&manifest, scope).ok_or_else(|| {
        AppError::business_rule(format!("O plugin não tem configuração {}.", scope.label()))
    })?;
    if !incoming.is_object() {
        return Err(AppError::validation(
            "A configuração precisa ser um objeto.",
        ));
    }
    let previous = plain(db, plugin, &manifest, scope).await?;
    restore_secrets(&mut incoming, &previous, &schema);
    let validated = params::validate(Some(&schema), &incoming)
        .map_err(|errors| AppError::validation(errors.join("; ")))?;
    let fields = params::secret_fields(Some(&schema));
    let mut stored = validated.clone();
    encrypt_in_place(&mut stored, &fields)?;

    match row(db, plugin.id, scope).await? {
        Some(model) => {
            let version = model.version + 1;
            let mut active: plugin_settings::ActiveModel = model.into();
            active.value = Set(stored);
            active.version = Set(version);
            active.updated_by = Set(user_id);
            active.update(db).await?;
        }
        None => {
            plugin_settings::ActiveModel {
                plugin_id: Set(plugin.id),
                device_id: Set(scope.device_id()),
                value: Set(stored),
                version: Set(1),
                updated_by: Set(user_id),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }
    let mut view = validated;
    mask_in_place(&mut view, &fields);
    Ok(view)
}

/// Mescla campos no ajuste de um dispositivo (ex.: o canal sugerido pelo
/// plano). Os valores chegam em claro, sem máscara.
///
/// # Errors
///
/// Os de [`save`].
pub async fn patch_device<C: ConnectionTrait>(
    db: &C,
    plugin: &plugins::Model,
    device_id: i64,
    patch: &Map<String, Value>,
    user_id: Option<i64>,
) -> AppResult<Value> {
    let manifest = manifest_of(plugin)?;
    let scope = Scope::Device(device_id);
    let mut current = plain(db, plugin, &manifest, scope).await?;
    if let Some(map) = current.as_object_mut() {
        for (key, value) in patch {
            map.insert(key.clone(), value.clone());
        }
    }
    save(db, plugin, scope, current, user_id).await
}

/// A configuração que o script recebe.
#[derive(Debug, Clone, Default)]
pub struct Effective {
    /// `{ "fleet": {…}, "device": {…} }`, em claro.
    pub value: Value,
    /// Os segredos, para o runtime mascarar o que sair do equipamento.
    pub secrets: Vec<String>,
}

/// # Errors
///
/// Erro do banco ou de decifra.
pub async fn effective<C: ConnectionTrait>(
    db: &C,
    plugin: &plugins::Model,
    device_id: Option<i64>,
) -> AppResult<Effective> {
    let manifest = manifest_of(plugin)?;
    let fleet = plain(db, plugin, &manifest, Scope::Fleet).await?;
    let device = match device_id {
        Some(id) => plain(db, plugin, &manifest, Scope::Device(id)).await?,
        None => json!({}),
    };
    let mut secrets = Vec::new();
    for (scope, value) in [(Scope::Fleet, &fleet), (Scope::Device(0), &device)] {
        if let Some(schema) = schema_of(&manifest, scope) {
            secrets.extend(params::secret_values(Some(&schema), value));
        }
    }
    Ok(Effective {
        value: json!({ "fleet": fleet, "device": device }),
        secrets,
    })
}

/// A configuração de um teste unitário (`UnitTest.settings`): padrões do
/// esquema aplicados e segredos listados, como em [`effective`].
#[must_use]
pub fn for_test(manifest: &PluginManifest, given: Option<&Value>) -> Effective {
    let mut value = json!({ "fleet": {}, "device": {} });
    let mut secrets = Vec::new();
    for (key, scope) in [("fleet", Scope::Fleet), ("device", Scope::Device(0))] {
        let raw = given
            .and_then(|given| given.get(key))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let Some(schema) = schema_of(manifest, scope) else {
            value[key] = raw;
            continue;
        };
        let filled = params::validate(Some(&schema), &raw).unwrap_or(raw);
        secrets.extend(params::secret_values(Some(&schema), &filled));
        value[key] = filled;
    }
    Effective { value, secrets }
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{Database, DatabaseConnection};

    async fn setup() -> (DatabaseConnection, plugins::Model, i64) {
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:".to_owned());
        options.max_connections(1).min_connections(1);
        let db = Database::connect(options).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let manifest = json!({
            "slug": "wifi", "name": "Wi-Fi", "version": "1.0.0", "transports": ["ssh"],
            "surfaces": ["device", "fleet"],
            "actions": [ { "id": "detect", "title": "Detectar", "effect": "read" } ],
            "fleet": { "title": "Rede" },
            "settings": {
                "fleet": { "type": "object", "properties": {
                    "mesh_key": { "type": "string", "secret": true },
                    "networks": { "type": "array", "items": { "type": "object",
                        "properties": {
                            "ssid": { "type": "string", "minLength": 1 },
                            "key": { "type": "string", "secret": true } },
                        "required": ["ssid"] } } } },
                "device": { "type": "object", "properties": {
                    "channel": { "type": "string", "default": "" } } }
            }
        });
        let plugin = plugins::ActiveModel {
            slug: Set("wifi".into()),
            name: Set("Wi-Fi".into()),
            version: Set("1.0.0".into()),
            scope: Set("model".into()),
            source: Set("user".into()),
            status: Set("active".into()),
            manifest: Set(manifest),
            script: Set(String::new()),
            usage: Set(String::new()),
            compatibility: Set(json!([])),
            tests: Set(json!({})),
            checksum: Set(String::new()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let device = crate::models::devices::ActiveModel {
            name: Set("AP".into()),
            r#type: Set("ap".into()),
            status: Set("online".into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        (db, plugin, device.id)
    }

    #[tokio::test]
    async fn segredo_e_cifrado_mascarado_e_mantido_pela_mascara() {
        let (db, plugin, _) = setup().await;
        let view = save(
            &db,
            &plugin,
            Scope::Fleet,
            json!({ "mesh_key": "meshsecret", "networks": [ { "ssid": "Loja", "key": "senha-loja-1" } ] }),
            None,
        )
        .await
        .unwrap();
        assert_eq!(view["mesh_key"], SECRET_MASK);
        assert_eq!(view["networks"][0]["key"], SECRET_MASK);
        let id = view["networks"][0]["id"].as_str().unwrap().to_owned();

        let stored = row(&db, plugin.id, Scope::Fleet).await.unwrap().unwrap();
        assert!(
            !stored.value.to_string().contains("senha-loja-1"),
            "cifrado no banco"
        );

        // Renomear a rede mantendo a senha (máscara) e acrescentar outra.
        save(
            &db,
            &plugin,
            Scope::Fleet,
            json!({ "mesh_key": SECRET_MASK, "networks": [
                { "id": id, "ssid": "Loja Centro", "key": SECRET_MASK },
                { "ssid": "Visitantes", "key": "visitante1" } ] }),
            None,
        )
        .await
        .unwrap();
        let effective = effective(&db, &plugin, None).await.unwrap();
        assert_eq!(effective.value["fleet"]["mesh_key"], "meshsecret");
        assert_eq!(
            effective.value["fleet"]["networks"][0]["key"],
            "senha-loja-1"
        );
        assert_eq!(effective.value["fleet"]["networks"][1]["key"], "visitante1");
        assert!(effective.secrets.contains(&"senha-loja-1".to_string()));
    }

    #[tokio::test]
    async fn ajuste_do_dispositivo_e_independente_e_aceita_patch() {
        let (db, plugin, device_id) = setup().await;
        let patch = json!({ "channel": "6" });
        patch_device(&db, &plugin, device_id, patch.as_object().unwrap(), None)
            .await
            .unwrap();
        let effective = effective(&db, &plugin, Some(device_id)).await.unwrap();
        assert_eq!(effective.value["device"]["channel"], "6");
        assert_eq!(
            effective.value["fleet"],
            json!({}),
            "frota sem configuração ainda"
        );
        let invalid = save(
            &db,
            &plugin,
            Scope::Fleet,
            json!({ "networks": [ { "ssid": "" } ] }),
            None,
        )
        .await
        .unwrap_err();
        assert!(
            invalid.to_string().contains("networks[1].ssid"),
            "{invalid}"
        );
    }
}
