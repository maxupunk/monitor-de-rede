//! Login de uso único no registry.
//!
//! Em vez de `docker login` — que gravaria a senha em
//! `/root/.docker/config.json` do host para sempre —, o deploy cria um
//! diretório temporário `0700` com um `config.json` mínimo e o passa ao
//! compose por `DOCKER_CONFIG`. O diretório some quando o guarda sai de
//! escopo, com sucesso ou não.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::{json, Value};

use agent_protocol::docker::stacks::RegistryAuth;

use super::super::DockerError;

/// O `config.json` do Docker com a credencial do registry.
#[must_use]
pub fn config_json(auth: &RegistryAuth) -> Value {
    let encoded = base64::engine::general_purpose::STANDARD.encode(format!(
        "{}:{}",
        auth.username,
        auth.secret.expose()
    ));
    json!({ "auths": { auth.registry.clone(): { "auth": encoded } } })
}

/// O diretório temporário com a credencial; apagado no `Drop`.
pub struct EphemeralDockerConfig {
    dir: PathBuf,
}

impl EphemeralDockerConfig {
    /// # Errors
    ///
    /// `Validation` quando o diretório temporário não pode ser criado.
    pub fn create(auth: &RegistryAuth) -> Result<Self, DockerError> {
        let dir =
            std::env::temp_dir().join(format!("netmonitor-registry-{}", uuid::Uuid::new_v4()));
        let failed = |error: std::io::Error| {
            DockerError::Validation(format!(
                "Não foi possível preparar o login no registry: {error}"
            ))
        };
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            builder
        };
        #[cfg(not(unix))]
        let builder = std::fs::DirBuilder::new();
        builder.create(&dir).map_err(failed)?;
        let guard = Self { dir };
        let body = serde_json::to_vec(&config_json(auth)).map_err(|_| DockerError::Engine)?;
        std::fs::write(guard.dir.join("config.json"), body).map_err(failed)?;
        Ok(guard)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for EphemeralDockerConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use agent_protocol::docker::stacks::Secret;

    use super::*;

    fn auth() -> RegistryAuth {
        RegistryAuth {
            registry: "ghcr.io".into(),
            username: "bot".into(),
            secret: Secret::new("ghp_x"),
        }
    }

    #[test]
    fn credencial_no_formato_do_docker() {
        let config = config_json(&auth());
        let encoded = config["auths"]["ghcr.io"]["auth"].as_str().expect("auth");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("base64");
        assert_eq!(decoded, b"bot:ghp_x");
    }

    #[test]
    fn diretorio_some_ao_sair_de_escopo() {
        let path = {
            let guard = EphemeralDockerConfig::create(&auth()).expect("criado");
            assert!(guard.path().join("config.json").exists());
            guard.path().to_path_buf()
        };
        assert!(!path.exists(), "credencial ficou no disco");
    }
}
