//! Persistência do `wg0.conf` no volume compartilhado (§8.10.2).
//!
//! O servidor **nunca** executa `docker exec`: escreve o arquivo e o watcher
//! dentro do container aplica com `wg syncconf`, sem derrubar túneis ativos.
//! É essa fronteira que permite o processo da API rodar sem `NET_ADMIN` e sem
//! acesso ao socket do Docker.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use tokio::io::AsyncWriteExt;

use netmonitor_core::shared::errors::{AppError, AppResult};

use crate::config_builder::{self, PeerEntryInput, ServerInterfaceInput};

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

/// Idade a partir da qual um temporário de escrita é tido como órfão. Uma
/// escrita leva milissegundos; o que passa disso sobrou de um processo que
/// morreu entre o `create_new` e o `rename`.
const STALE_TEMPORARY_AGE: Duration = Duration::from_secs(60);

/// O nome é um temporário de escrita de `file_name`
/// (`.{file_name}.{pid}.{nonce}.tmp`)?
fn is_temporary_of(name: &str, file_name: &str) -> bool {
    name.strip_prefix('.')
        .and_then(|rest| rest.strip_prefix(file_name))
        .and_then(|rest| rest.strip_prefix('.'))
        .and_then(|rest| rest.strip_suffix(".tmp"))
        .is_some_and(|middle| {
            let mut parts = middle.split('.');
            matches!(
                (parts.next(), parts.next(), parts.next()),
                (Some(pid), Some(nonce), None)
                    if !pid.is_empty()
                        && pid.bytes().all(|byte| byte.is_ascii_digit())
                        && !nonce.is_empty()
                        && nonce.bytes().all(|byte| byte.is_ascii_hexdigit())
            )
        })
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

    /// Apaga os temporários órfãos de `file_name`: eles carregam a chave
    /// privada do servidor e não podem ficar esquecidos no volume. Só os que
    /// casam com o padrão e já passaram de [`STALE_TEMPORARY_AGE`] — o de uma
    /// escrita concorrente em curso fica. Falha aqui não impede a escrita.
    async fn remove_stale_temporaries(&self, file_name: &str) {
        let Ok(mut entries) = tokio::fs::read_dir(&self.base_dir).await else {
            return;
        };
        let now = SystemTime::now();
        while let Ok(Some(entry)) = entries.next_entry().await {
            if !is_temporary_of(&entry.file_name().to_string_lossy(), file_name) {
                continue;
            }
            let stale = entry
                .metadata()
                .await
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age >= STALE_TEMPORARY_AGE);
            if stale {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
}

#[async_trait::async_trait]
impl VpnConfigSink for FileConfigSink {
    async fn write(&self, file_name: &str, contents: &str) -> AppResult<()> {
        tokio::fs::create_dir_all(&self.base_dir)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
        self.remove_stale_temporaries(file_name).await;

        // Escrita atômica (matriz de paridade #35): o watcher do container
        // acorda com o `rename`, e nunca com um arquivo pela metade — carregar
        // meio `wg0.conf` derrubaria os peers que ainda não foram escritos.
        // O nome temporário único impede que escritas concorrentes corrompam o
        // mesmo `.tmp`, e no Unix o arquivo nasce já em `0600` antes de
        // receber a chave privada do servidor.
        let target = self.resolve(file_name);
        let nonce: u64 = rand::random();
        let temporary = self.base_dir.join(format!(
            ".{file_name}.{}.{nonce:016x}.tmp",
            std::process::id()
        ));
        if let Err(error) = write_temp_file(&temporary, contents).await {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(error);
        }
        if let Err(error) = tokio::fs::rename(&temporary, &target).await {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(AppError::Internal(error.into()));
        }
        Ok(())
    }

    async fn read(&self, file_name: &str) -> Option<String> {
        tokio::fs::read_to_string(self.resolve(file_name))
            .await
            .ok()
    }
}

/// Grava o conteúdo em um arquivo temporário recém-criado (`0600` no Unix) e
/// força `sync_all` antes do `rename` atômico.
async fn write_temp_file(path: &Path, contents: &str) -> AppResult<()> {
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    file.write_all(contents.as_bytes())
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    file.sync_all()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    Ok(())
}

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

        // Escritas concorrentes usam arquivos temporários distintos e nunca
        // corrompem nem deixam `.tmp` órfãos.
        let cfg = servidor();
        let (a, b, c) = tokio::join!(
            write_server_config(&sink, &cfg, &[]),
            write_server_config(&sink, &cfg, &[]),
            write_server_config(&sink, &cfg, &[]),
        );
        a.unwrap();
        b.unwrap();
        c.unwrap();
        let mut entries = tokio::fs::read_dir(&dir).await.unwrap();
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        assert_eq!(names, vec!["wg0.conf".to_string()]);
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[test]
    fn reconhece_so_os_temporarios_do_proprio_arquivo() {
        assert!(is_temporary_of(
            ".wg0.conf.42.00ff00ff00ff00ff.tmp",
            "wg0.conf"
        ));
        assert!(!is_temporary_of("wg0.conf", "wg0.conf"));
        assert!(!is_temporary_of(".wg0.conf.tmp", "wg0.conf"));
        assert!(!is_temporary_of(".wg1.conf.42.00ff.tmp", "wg0.conf"));
        assert!(!is_temporary_of(".wg0.conf.x.00ff.tmp", "wg0.conf"));
        assert!(!is_temporary_of(".wg0.conf.42.00ff.extra.tmp", "wg0.conf"));
        assert!(!is_temporary_of(".wg0.conf.42.zz.tmp", "wg0.conf"));
    }

    #[tokio::test]
    async fn apaga_o_temporario_orfao_e_poupa_o_recente() {
        let dir = std::env::temp_dir().join(format!("wg-conf-orfao-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&dir).await;
        std::fs::create_dir_all(&dir).unwrap();
        let orphan = dir.join(".wg0.conf.7.0000000000000001.tmp");
        let recent = dir.join(".wg0.conf.8.0000000000000002.tmp");
        let other = dir.join(".wg1.conf.7.0000000000000003.tmp");
        for path in [&orphan, &recent, &other] {
            std::fs::write(path, "PrivateKey = PRIV").unwrap();
        }
        let old = SystemTime::now() - Duration::from_secs(600);
        for path in [&orphan, &other] {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(old)
                .unwrap();
        }

        let sink = FileConfigSink::new(&dir);
        write_server_config(&sink, &servidor(), &[]).await.unwrap();

        assert!(!orphan.exists(), "o órfão com a chave privada sai");
        assert!(recent.exists(), "uma escrita concorrente em curso fica");
        assert!(
            other.exists(),
            "temporário de outra interface não é deste write"
        );
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn arquivo_ausente_le_como_none_e_nao_como_erro() {
        let sink = FileConfigSink::new(std::env::temp_dir().join("wg-conf-inexistente"));
        assert!(sink.read("wg0.status").await.is_none());
    }
}
