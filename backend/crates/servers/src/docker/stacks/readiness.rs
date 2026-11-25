//! O que falta no host para deployar uma stack — a checagem de prontidão
//! que o assistente da tela mostra antes do primeiro deploy. Só lê.

use std::{collections::BTreeSet, time::Duration};

use agent_protocol::docker::stacks::validate_project;
use futures::{stream, StreamExt};
use serde_json::Value;

use super::{
    super::{
        cli, compose, compose::PROJECT_LABEL, compose::SERVICE_LABEL, engine, source::DockerEngine,
        DockerError,
    },
    StackReadiness, StacksRoot,
};

const CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// Bytes livres da saída de `df -Pk <dir>` (coluna "Available", em KiB).
#[must_use]
pub fn parse_df(output: &str) -> Option<u64> {
    let line = output.lines().nth(1)?;
    let available: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    available.checked_mul(1024)
}

/// O container tem healthcheck declarado (e não desligado com `NONE`)?
#[must_use]
pub fn has_healthcheck(inspected: &Value) -> bool {
    inspected
        .pointer("/Config/Healthcheck/Test")
        .and_then(Value::as_array)
        .and_then(|test| test.first())
        .and_then(Value::as_str)
        .is_some_and(|first| first != "NONE")
}

/// # Errors
///
/// `Validation` para nome de projeto inválido. O resto vira campo da
/// resposta: a tela precisa ver tudo o que falta de uma vez.
pub async fn inspect(
    engine: &dyn DockerEngine,
    root: Result<StacksRoot, String>,
    project: &str,
) -> Result<StackReadiness, DockerError> {
    validate_project(project)?;
    let checked = match root {
        Ok(root) => root.check().await.map(|()| root),
        Err(reason) => Err(reason),
    };
    let (stacks_dir, stacks_dir_issue) = match &checked {
        Ok(root) => (Some(root.path().display().to_string()), None),
        Err(reason) => (None, Some(reason.clone())),
    };
    let (git_res, compose_res, df_res, containers_res) = tokio::join!(
        cli::capture("git", &["--version"], None, CHECK_TIMEOUT),
        compose::availability(),
        async {
            if let Some(path) = &stacks_dir {
                cli::capture("df", &["-Pk", path], None, CHECK_TIMEOUT)
                    .await
                    .ok()
                    .and_then(|(_, output)| parse_df(&output))
            } else {
                None
            }
        },
        engine::list_containers(engine),
    );
    let git_version = git_res
        .ok()
        .filter(|(ok, _)| *ok)
        .map(|(_, version)| version);
    let compose_version = compose_res.ok();
    let free_bytes = df_res;
    let project_dir_exists = checked
        .as_ref()
        .is_ok_and(|root| root.project_dir(project).join(".git").exists());

    let containers = containers_res.unwrap_or_default();
    let known = compose::projects(&containers);
    let found = known.iter().find(|known| known.name == project);
    let mut seen_services = BTreeSet::new();
    let to_inspect: Vec<(String, String)> = containers
        .iter()
        .filter(|container| {
            container.labels.get(PROJECT_LABEL).map(String::as_str) == Some(project)
        })
        .filter_map(|container| {
            let service = container.labels.get(SERVICE_LABEL)?;
            if !seen_services.insert(service.clone()) {
                return None;
            }
            Some((container.id.clone(), service.clone()))
        })
        .collect();
    let mut services_without_healthcheck: Vec<String> = stream::iter(to_inspect)
        .map(|(id, service)| async move {
            match engine.inspect_container_raw(&id).await {
                Ok(inspected) if !has_healthcheck(&inspected) => Some(service),
                _ => None,
            }
        })
        .buffer_unordered(8)
        .filter_map(|item| async move { item })
        .collect()
        .await;
    services_without_healthcheck.sort();

    Ok(StackReadiness {
        stacks_dir,
        stacks_dir_issue,
        git_version,
        compose_version,
        free_bytes,
        project_dir_exists,
        running_working_dir: found.map(|project| project.working_dir.clone()),
        services_without_healthcheck,
        protected: found.is_some_and(|project| project.protected),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn espaco_livre_do_df() {
        let output = "Filesystem     1024-blocks      Used Available Capacity Mounted on\n\
                      /dev/sda1        100000000  40000000  60000000      40% /srv\n";
        assert_eq!(parse_df(output), Some(60_000_000 * 1024));
        assert_eq!(parse_df("lixo"), None);
    }

    #[test]
    fn healthcheck_desligado_nao_conta() {
        assert!(has_healthcheck(
            &json!({"Config": {"Healthcheck": {"Test": ["CMD", "curl", "-f", "x"]}}})
        ));
        assert!(!has_healthcheck(
            &json!({"Config": {"Healthcheck": {"Test": ["NONE"]}}})
        ));
        assert!(!has_healthcheck(&json!({"Config": {}})));
    }
}
