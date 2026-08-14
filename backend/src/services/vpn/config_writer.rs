//! Persistência do `wg0.conf` no volume compartilhado (§8.10.2).
//!
//! O servidor **nunca** executa `docker exec`: escreve o arquivo e o watcher
//! dentro do container aplica com `wg syncconf`, sem derrubar túneis ativos.
//! É essa fronteira que permite o processo da API rodar sem `NET_ADMIN` e sem
//! acesso ao socket do Docker.

use std::path::{Path, PathBuf};

use crate::services::{
    shared::errors::{AppError, AppResult},
    vpn::config_builder::{self, PeerEntryInput, ServerInterfaceInput},
};

/// Diretório do volume `wg-config`; em Windows cai para uma pasta local.
#[must_use]
pub fn resolve_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("WG_CONFIG_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    if cfg!(windows) {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("tmp")
            .join("wireguard")
    } else {
        PathBuf::from("/config")
    }
}

/// Porta de saída — permite trocar disco por outro destino em testes.
#[async_trait::async_trait]
pub trait VpnConfigSink: Send + Sync {
    async fn write(&self, file_name: &str, contents: &str) -> AppResult<()>;
    async fn read(&self, file_name: &str) -> Option<String>;
}

pub struct FileConfigSink {
    base_dir: PathBuf,
}

impl Default for FileConfigSink {
    fn default() -> Self {
        Self::new(resolve_config_dir())
    }
}

impl FileConfigSink {
    #[must_use]
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    fn resolve(&self, file_name: &str) -> PathBuf {
        self.base_dir.join(file_name)
    }
}

#[async_trait::async_trait]
impl VpnConfigSink for FileConfigSink {
    async fn write(&self, file_name: &str, contents: &str) -> AppResult<()> {
        tokio::fs::create_dir_all(&self.base_dir)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;

        // Escrita atômica (matriz de paridade #35): o watcher do container
        // acorda com o `rename`, e nunca com um arquivo pela metade — carregar
        // meio `wg0.conf` derrubaria os peers que ainda não foram escritos.
        let target = self.resolve(file_name);
        let temporary = target.with_extension("conf.tmp");
        tokio::fs::write(&temporary, contents)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
        set_owner_only_permissions(&temporary).await;
        tokio::fs::rename(&temporary, &target)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
        Ok(())
    }

    async fn read(&self, file_name: &str) -> Option<String> {
        tokio::fs::read_to_string(self.resolve(file_name))
            .await
            .ok()
    }
}

/// `0600` no arquivo temporário: ele carrega a chave privada do servidor, e o
/// volume é compartilhado com outro container.
#[cfg(unix)]
async fn set_owner_only_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) =
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await
    {
        tracing::warn!(%error, "não foi possível restringir a permissão do wg0.conf");
    }
}

/// No Windows não há bit de permissão equivalente; o diretório local de
/// desenvolvimento já é do usuário.
#[cfg(not(unix))]
async fn set_owner_only_permissions(_path: &Path) {}

/// Escreve `<interface>.conf` e devolve o conteúdo aplicado.
///
/// # Errors
///
/// Falha quando o CIDR é inválido ou o volume não é gravável.
pub async fn write_server_config(
    sink: &dyn VpnConfigSink,
    server: &ServerInterfaceInput,
    peers: &[PeerEntryInput],
) -> AppResult<String> {
    let contents = config_builder::build(server, peers)?;
    sink.write(&format!("{}.conf", server.interface_name), &contents)
        .await?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn servidor() -> ServerInterfaceInput {
        ServerInterfaceInput {
            interface_name: "wg0".into(),
            address: "10.8.0.1".into(),
            cidr: "10.8.0.0/24".into(),
            listen_port: 51_820,
            private_key: "PRIV".into(),
            mtu: 1_420,
            allow_peer_to_peer: false,
        }
    }

    #[test]
    #[serial]
    fn a_variavel_de_ambiente_tem_precedencia_sobre_o_padrao() {
        std::env::set_var("WG_CONFIG_DIR", "/volume/wg");
        assert_eq!(resolve_config_dir(), PathBuf::from("/volume/wg"));
        // Vazia é como ausente: um compose que declara `WG_CONFIG_DIR=` sem
        // valor não pode fazer o servidor gravar na raiz.
        std::env::set_var("WG_CONFIG_DIR", "");
        assert_ne!(resolve_config_dir(), PathBuf::from(""));
        std::env::remove_var("WG_CONFIG_DIR");
    }

    #[tokio::test]
    async fn escreve_o_arquivo_e_nao_deixa_temporario_para_tras() {
        let dir = std::env::temp_dir().join(format!("wg-conf-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&dir).await;
        let sink = FileConfigSink::new(&dir);

        let contents = write_server_config(&sink, &servidor(), &[]).await.unwrap();
        assert!(contents.contains("[Interface]"));
        assert_eq!(
            sink.read("wg0.conf").await.as_deref(),
            Some(contents.as_str())
        );
        assert!(sink.read("wg0.conf.tmp").await.is_none());

        // Regravar por cima funciona (o `rename` substitui).
        write_server_config(&sink, &servidor(), &[]).await.unwrap();
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn arquivo_ausente_le_como_none_e_nao_como_erro() {
        let sink = FileConfigSink::new(std::env::temp_dir().join("wg-conf-inexistente"));
        assert!(sink.read("wg0.status").await.is_none());
    }
}
