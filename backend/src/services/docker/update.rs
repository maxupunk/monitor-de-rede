//! Atualização de imagem e recriação de container com rollback.
//!
//! O fluxo é o mesmo de ferramentas como o Watchtower, só que com um único
//! alvo e sem deixar nada pela metade:
//!
//! 1. baixa a tag atual da imagem;
//! 2. se a imagem não mudou, para por aí;
//! 3. renomeia o container antigo para `<nome>-old-<ts>` e o para;
//! 4. cria o novo com a mesma configuração (volumes anônimos incluídos) e o
//!    mesmo nome, reconecta as redes e o inicia;
//! 5. espera o healthcheck, quando existe;
//! 6. só então remove o antigo — sem apagar volumes.
//!
//! Qualquer falha a partir do passo 3 desfaz o que foi feito: o novo sai, o
//! antigo volta ao nome e ao estado originais.

use std::{collections::HashMap, time::Duration};

use bollard::{
    container::{
        Config, CreateContainerOptions, RemoveContainerOptions, RenameContainerOptions,
        StopContainerOptions,
    },
    image::CreateImageOptions,
    models::EndpointSettings,
    network::ConnectNetworkOptions,
};
use futures::StreamExt;
use serde_json::{Map, Value};

use crate::views::docker::DockerActionResponse;

use super::{call, client, log_clear::is_current_container, maintenance::Progress, DockerError};

const PULL_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const PULL_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const HEALTH_TIMEOUT: Duration = Duration::from_secs(90);
const HEALTH_STEP: Duration = Duration::from_secs(2);

/// O que o recreate precisa saber do container atual, já decidido.
#[derive(Debug, Clone, PartialEq)]
pub struct RecreatePlan {
    pub name: String,
    pub image: String,
    pub image_id: String,
    pub running: bool,
    /// Corpo de criação no formato da Engine (`Config` + `HostConfig` +
    /// `NetworkingConfig` com a primeira rede).
    pub create_body: Value,
    /// Redes além da primeira: a Engine só aceita uma no create em APIs antigas.
    pub extra_networks: Vec<(String, Value)>,
}

/// Monta o plano a partir do `inspect` cru. Função pura: é aqui que mora a
/// regra de "o que é preservado", e é isso que os testes fixam.
///
/// # Errors
///
/// `Validation` quando o container não referencia uma tag que possa ser
/// baixada (imagem por id) ou quando o inspect vem incompleto.
pub fn recreate_plan(inspect: &Value) -> Result<RecreatePlan, DockerError> {
    let id = inspect
        .get("Id")
        .and_then(Value::as_str)
        .ok_or(DockerError::Engine)?;
    let name = inspect
        .get("Name")
        .and_then(Value::as_str)
        .map(|name| name.trim_start_matches('/').to_string())
        .filter(|name| !name.is_empty())
        .ok_or(DockerError::Engine)?;
    let config = inspect
        .get("Config")
        .and_then(Value::as_object)
        .ok_or(DockerError::Engine)?;
    let image = config
        .get("Image")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if image.is_empty() || image.starts_with("sha256:") {
        return Err(DockerError::Validation(
            "O container foi criado a partir de um id de imagem, sem tag para atualizar".into(),
        ));
    }
    let image_id = inspect
        .get("Image")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let running = inspect
        .pointer("/State/Running")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let short_id: String = id.chars().take(12).collect();
    let mut body = config.clone();
    // O Docker usa o id curto como hostname padrão: copiá-lo faria o novo
    // container se apresentar com o nome do antigo.
    if body.get("Hostname").and_then(Value::as_str) == Some(short_id.as_str()) {
        body.remove("Hostname");
    }
    body.insert("Image".into(), Value::String(image.clone()));

    let mut host_config = inspect
        .get("HostConfig")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    preserve_anonymous_volumes(inspect, &mut host_config);
    let network_mode = host_config
        .get("NetworkMode")
        .and_then(Value::as_str)
        .unwrap_or("default")
        .to_string();
    body.insert("HostConfig".into(), Value::Object(host_config));

    let (first, extra_networks) = endpoints(inspect, &network_mode, &short_id);
    if let Some((network, endpoint)) = first {
        let mut endpoints = Map::new();
        endpoints.insert(network, endpoint);
        body.insert(
            "NetworkingConfig".into(),
            serde_json::json!({ "EndpointsConfig": endpoints }),
        );
    }

    Ok(RecreatePlan {
        name,
        image,
        image_id,
        running,
        create_body: Value::Object(body),
        extra_networks,
    })
}

/// Volume anônimo declarado pela imagem ganharia um volume **novo e vazio**
/// no recreate. Montá-lo explicitamente pelo nome preserva os dados.
fn preserve_anonymous_volumes(inspect: &Value, host_config: &mut Map<String, Value>) {
    let declared: Vec<String> = host_config
        .get("Binds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|bind| bind.split(':').nth(1).map(ToString::to_string))
        .chain(
            host_config
                .get("Mounts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|mount| mount.get("Target").and_then(Value::as_str))
                .map(ToString::to_string),
        )
        .collect();
    let anonymous: Vec<Value> = inspect
        .get("Mounts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|mount| mount.get("Type").and_then(Value::as_str) == Some("volume"))
        .filter_map(|mount| {
            let name = mount.get("Name").and_then(Value::as_str)?;
            let target = mount.get("Destination").and_then(Value::as_str)?;
            let is_anonymous = name.len() == 64 && name.chars().all(|c| c.is_ascii_hexdigit());
            (is_anonymous && !declared.iter().any(|path| path == target)).then(|| {
                serde_json::json!({
                    "Type": "volume",
                    "Source": name,
                    "Target": target,
                    "ReadOnly": !mount.get("RW").and_then(Value::as_bool).unwrap_or(true),
                })
            })
        })
        .collect();
    if anonymous.is_empty() {
        return;
    }
    let mounts = host_config
        .entry("Mounts")
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Value::Array(items) = mounts {
        items.extend(anonymous);
    } else {
        *mounts = Value::Array(anonymous);
    }
}

type Endpoint = (String, Value);

/// Endpoints a recriar. Só a configuração declarada viaja; IP, MAC e ids são
/// atribuídos de novo pela Engine.
fn endpoints(
    inspect: &Value,
    network_mode: &str,
    short_id: &str,
) -> (Option<Endpoint>, Vec<Endpoint>) {
    if matches!(network_mode, "host" | "none") || network_mode.starts_with("container:") {
        return (None, Vec::new());
    }
    let mut networks: Vec<Endpoint> = inspect
        .pointer("/NetworkSettings/Networks")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, endpoint)| {
            let aliases: Vec<Value> = endpoint
                .get("Aliases")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|alias| alias.as_str() != Some(short_id))
                .cloned()
                .collect();
            let mut settings = Map::new();
            if !aliases.is_empty() {
                settings.insert("Aliases".into(), Value::Array(aliases));
            }
            for key in ["IPAMConfig", "Links", "DriverOpts"] {
                if let Some(value) = endpoint.get(key).filter(|value| !value.is_null()) {
                    settings.insert(key.into(), value.clone());
                }
            }
            (name.clone(), Value::Object(settings))
        })
        .collect();
    // A rede do `NetworkMode` vai no create; as demais, depois.
    networks.sort_by_key(|(name, _)| (name != network_mode, name.clone()));
    let mut iter = networks.into_iter();
    let first = iter.next();
    (first, iter.collect())
}

/// Baixa a imagem relatando o progresso da Engine.
///
/// # Errors
///
/// `Unavailable` por timeout; `Validation` quando a Engine recusa a tag.
pub async fn pull_image(
    image: &str,
    progress: &Progress,
) -> Result<DockerActionResponse, DockerError> {
    let image = image.trim();
    if image.is_empty() || image.chars().any(char::is_control) || image.len() > 255 {
        return Err(DockerError::Validation("Imagem inválida".into()));
    }
    let client = client()?;
    let _ = progress.send(format!("Baixando {image}…"));
    let pull = async {
        let mut stream = client.create_image(
            Some(CreateImageOptions {
                from_image: image,
                ..Default::default()
            }),
            None,
            None,
        );
        loop {
            let next = tokio::time::timeout(PULL_IDLE_TIMEOUT, stream.next())
                .await
                .map_err(|_| DockerError::Unavailable)?;
            let Some(item) = next else { break };
            let info = item.map_err(|error| match error {
                bollard::errors::Error::DockerResponseServerError { message, .. } => {
                    DockerError::Validation(format!("A Engine recusou o pull: {message}"))
                }
                bollard::errors::Error::DockerStreamError { error } => {
                    DockerError::Validation(format!("A Engine recusou o pull: {error}"))
                }
                _ => DockerError::Engine,
            })?;
            if let Some(error) = info.error {
                return Err(DockerError::Validation(format!("Falha no pull: {error}")));
            }
            if let Some(line) = progress_line(
                info.id.as_deref(),
                info.status.as_deref(),
                info.progress.as_deref(),
            ) {
                let _ = progress.send(line);
            }
        }
        Ok::<(), DockerError>(())
    };
    tokio::time::timeout(PULL_TIMEOUT, pull)
        .await
        .map_err(|_| DockerError::Unavailable)??;
    Ok(DockerActionResponse {
        success: true,
        message: format!("Imagem {image} atualizada."),
    })
}

fn progress_line(id: Option<&str>, status: Option<&str>, progress: Option<&str>) -> Option<String> {
    let status = status?.trim();
    if status.is_empty() {
        return None;
    }
    let mut line = String::new();
    if let Some(id) = id.filter(|id| !id.is_empty()) {
        line.push_str(id);
        line.push_str(": ");
    }
    line.push_str(status);
    if let Some(progress) = progress.filter(|progress| !progress.is_empty()) {
        line.push(' ');
        line.push_str(progress);
    }
    Some(line)
}

/// Atualiza o container para a versão atual da sua tag.
///
/// # Errors
///
/// Propaga falhas da Engine depois de restaurar o container original.
pub async fn update_container(
    id: &str,
    progress: &Progress,
) -> Result<DockerActionResponse, DockerError> {
    let client = client()?;
    let inspect = serde_json::to_value(call(client.inspect_container(id, None)).await?)
        .map_err(|_| DockerError::Engine)?;
    let full_id = inspect
        .get("Id")
        .and_then(Value::as_str)
        .unwrap_or(id)
        .to_string();
    if is_current_container(&full_id) {
        return Err(DockerError::Validation(
            "O container que executa o NetMonitor não pode se recriar".into(),
        ));
    }
    let plan = recreate_plan(&inspect)?;

    pull_image(&plan.image, progress).await?;
    let pulled = call(client.inspect_image(&plan.image)).await?;
    if pulled.id.as_deref() == Some(plan.image_id.as_str()) {
        let _ = progress.send("A imagem já está na versão mais recente.".into());
        return Ok(DockerActionResponse {
            success: true,
            message: format!(
                "{} já usa a versão mais recente de {}.",
                plan.name, plan.image
            ),
        });
    }

    let backup = format!("{}-old-{}", plan.name, chrono::Utc::now().timestamp());
    let _ = progress.send(format!("Renomeando {} para {backup}…", plan.name));
    call(client.rename_container(
        &full_id,
        RenameContainerOptions {
            name: backup.clone(),
        },
    ))
    .await?;
    if plan.running {
        let _ = progress.send("Parando o container atual…".into());
        if let Err(error) =
            call(client.stop_container(&full_id, None::<StopContainerOptions>)).await
        {
            rollback(&client, &full_id, &plan, None, progress).await;
            return Err(error);
        }
    }

    match create_and_start(&client, &plan, progress).await {
        Ok(new_id) => {
            let _ = progress.send("Removendo o container anterior…".into());
            // Sem `v: true`: volumes nomeados e anônimos agora pertencem ao novo.
            let _ = call(client.remove_container(
                &full_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            ))
            .await;
            Ok(DockerActionResponse {
                success: true,
                message: format!(
                    "{} recriado com a imagem atual de {} ({}).",
                    plan.name,
                    plan.image,
                    new_id.chars().take(12).collect::<String>()
                ),
            })
        }
        Err((error, created)) => {
            rollback(&client, &full_id, &plan, created.as_deref(), progress).await;
            Err(error)
        }
    }
}

async fn create_and_start(
    client: &bollard::Docker,
    plan: &RecreatePlan,
    progress: &Progress,
) -> Result<String, (DockerError, Option<String>)> {
    let config: Config<String> = serde_json::from_value(plan.create_body.clone())
        .map_err(|_| (DockerError::Engine, None))?;
    let _ = progress.send(format!("Criando {} com a nova imagem…", plan.name));
    let created = call(client.create_container(
        Some(CreateContainerOptions {
            name: plan.name.clone(),
            platform: None,
        }),
        config,
    ))
    .await
    .map_err(|error| (error, None))?;
    let new_id = created.id;
    for (network, endpoint) in &plan.extra_networks {
        let endpoint: EndpointSettings =
            serde_json::from_value(endpoint.clone()).unwrap_or_default();
        call(client.connect_network(
            network,
            ConnectNetworkOptions {
                container: new_id.clone(),
                endpoint_config: endpoint,
            },
        ))
        .await
        .map_err(|error| (error, Some(new_id.clone())))?;
    }
    if plan.running {
        let _ = progress.send("Iniciando o novo container…".into());
        call(client.start_container::<String>(&new_id, None))
            .await
            .map_err(|error| (error, Some(new_id.clone())))?;
        wait_healthy(client, &new_id, progress)
            .await
            .map_err(|error| (error, Some(new_id.clone())))?;
    }
    Ok(new_id)
}

/// Espera o healthcheck declarado. Sem healthcheck, basta estar rodando.
async fn wait_healthy(
    client: &bollard::Docker,
    id: &str,
    progress: &Progress,
) -> Result<(), DockerError> {
    let deadline = tokio::time::Instant::now() + HEALTH_TIMEOUT;
    loop {
        let inspect = call(client.inspect_container(id, None)).await?;
        let state = inspect.state.unwrap_or_default();
        let health = state
            .health
            .and_then(|health| health.status)
            .map(|status| status.to_string());
        match (state.running.unwrap_or(false), health.as_deref()) {
            (false, _) => {
                return Err(DockerError::Validation(
                    "O novo container parou logo após iniciar".into(),
                ))
            }
            (true, None | Some("none" | "healthy" | "")) => return Ok(()),
            (true, Some("unhealthy")) => {
                return Err(DockerError::Validation(
                    "O novo container ficou unhealthy".into(),
                ))
            }
            (true, Some(_)) => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(DockerError::Validation(
                        "O healthcheck do novo container não ficou healthy a tempo".into(),
                    ));
                }
                let _ = progress.send("Aguardando o healthcheck…".into());
                tokio::time::sleep(HEALTH_STEP).await;
            }
        }
    }
}

async fn rollback(
    client: &bollard::Docker,
    original_id: &str,
    plan: &RecreatePlan,
    created: Option<&str>,
    progress: &Progress,
) {
    let _ = progress.send("Falha na atualização; restaurando o container original…".into());
    if let Some(created) = created {
        let _ = call(client.remove_container(
            created,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        ))
        .await;
    }
    let _ = call(client.rename_container(
        original_id,
        RenameContainerOptions {
            name: plan.name.clone(),
        },
    ))
    .await;
    if plan.running {
        let _ = call(client.start_container::<String>(original_id, None)).await;
    }
}

/// Rótulos que o novo container herda do antigo; exposto para os testes.
#[must_use]
pub fn labels(plan: &RecreatePlan) -> HashMap<String, String> {
    plan.create_body
        .get("Labels")
        .and_then(Value::as_object)
        .map(|labels| {
            labels
                .iter()
                .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.into())))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn inspect() -> Value {
        json!({
            "Id": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "Name": "/web",
            "Image": "sha256:antiga",
            "State": { "Running": true },
            "Config": {
                "Hostname": "0123456789ab",
                "Image": "nginx:alpine",
                "Env": ["TZ=America/Sao_Paulo"],
                "Labels": { "com.docker.compose.project": "portal" }
            },
            "HostConfig": {
                "NetworkMode": "portal_default",
                "Binds": ["/srv/html:/usr/share/nginx/html:ro"],
                "RestartPolicy": { "Name": "unless-stopped" }
            },
            "Mounts": [
                { "Type": "bind", "Source": "/srv/html", "Destination": "/usr/share/nginx/html", "RW": false },
                { "Type": "volume", "Name": "a".repeat(64), "Destination": "/var/cache/nginx", "RW": true },
                { "Type": "volume", "Name": "dados", "Destination": "/dados", "RW": true }
            ],
            "NetworkSettings": {
                "Networks": {
                    "portal_default": { "Aliases": ["web", "0123456789ab"], "IPAddress": "172.18.0.2" },
                    "monitoramento": { "Aliases": null, "IPAMConfig": { "IPv4Address": "10.9.0.5" } }
                }
            }
        })
    }

    #[test]
    fn plano_preserva_configuracao_e_descarta_o_que_e_dinamico() {
        let plan = recreate_plan(&inspect()).expect("plano");
        assert_eq!(plan.name, "web");
        assert_eq!(plan.image, "nginx:alpine");
        assert!(plan.running);
        let body = &plan.create_body;
        assert!(
            body.get("Hostname").is_none(),
            "hostname padrão não é copiado"
        );
        assert_eq!(body["Env"][0], "TZ=America/Sao_Paulo");
        assert_eq!(
            body["HostConfig"]["RestartPolicy"]["Name"],
            "unless-stopped"
        );
        assert_eq!(labels(&plan)["com.docker.compose.project"], "portal");

        let endpoint = &body["NetworkingConfig"]["EndpointsConfig"]["portal_default"];
        assert_eq!(
            endpoint["Aliases"],
            json!(["web"]),
            "alias do id antigo sai"
        );
        assert!(endpoint.get("IPAddress").is_none());
        assert_eq!(plan.extra_networks.len(), 1);
        assert_eq!(plan.extra_networks[0].0, "monitoramento");
        assert_eq!(
            plan.extra_networks[0].1["IPAMConfig"]["IPv4Address"],
            "10.9.0.5"
        );
    }

    #[test]
    fn volume_anonimo_e_montado_pelo_nome_para_nao_perder_dados() {
        let plan = recreate_plan(&inspect()).expect("plano");
        let mounts = plan.create_body["HostConfig"]["Mounts"]
            .as_array()
            .expect("mounts");
        assert_eq!(
            mounts.len(),
            1,
            "só o anônimo; bind e nomeado já estão declarados"
        );
        assert_eq!(mounts[0]["Source"], "a".repeat(64));
        assert_eq!(mounts[0]["Target"], "/var/cache/nginx");
        assert_eq!(mounts[0]["ReadOnly"], false);
    }

    #[test]
    fn plano_vira_config_da_engine() {
        let plan = recreate_plan(&inspect()).expect("plano");
        let config: Config<String> =
            serde_json::from_value(plan.create_body).expect("config de criação");
        assert_eq!(config.image.as_deref(), Some("nginx:alpine"));
        assert!(config.host_config.is_some());
        assert!(config.networking_config.is_some());
    }

    #[test]
    fn rede_do_host_nao_leva_endpoints() {
        let mut value = inspect();
        value["HostConfig"]["NetworkMode"] = json!("host");
        let plan = recreate_plan(&value).expect("plano");
        assert!(plan.create_body.get("NetworkingConfig").is_none());
        assert!(plan.extra_networks.is_empty());
    }

    #[test]
    fn imagem_por_id_nao_tem_o_que_atualizar() {
        let mut value = inspect();
        value["Config"]["Image"] = json!("sha256:abc");
        assert!(matches!(
            recreate_plan(&value),
            Err(DockerError::Validation(_))
        ));
    }

    #[test]
    fn linha_de_progresso_junta_camada_estado_e_barra() {
        assert_eq!(
            progress_line(Some("abc"), Some("Downloading"), Some("[==>  ] 1MB/2MB")).as_deref(),
            Some("abc: Downloading [==>  ] 1MB/2MB")
        );
        assert_eq!(progress_line(None, Some(" "), None), None);
    }
}
