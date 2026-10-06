//! Adapter SFTP.
//!
//! Autentica por senha, por chave privada ou por chave com passphrase.
//!
//! ## Por que `russh`, e não o service SFTP do `opendal`
//!
//! O service do `opendal` delega ao binário `ssh` da máquina: o container
//! precisaria de um cliente SSH instalado, e no Windows a coisa depende do
//! OpenSSH opcional do sistema. O `russh` é Rust puro e já está na árvore —
//! é ele que fala com os roteadores na ativação do syslog.
//!
//! ## A conexão é por operação, e não um pool
//!
//! Cada chamada abre e fecha a sessão. É mais lento que manter um pool, e é
//! deliberado: as credenciais vêm de uma linha do banco que pode ser editada a
//! qualquer momento, e um pool serviria conexões autenticadas com a credencial
//! **anterior**.
//!
//! ## A identidade do servidor é conferida (TOFU)
//!
//! Na primeira conexão bem-sucedida a impressão digital da chave do servidor é
//! gravada na config (`hostKeyFingerprint`). Dali em diante uma chave diferente
//! derruba a conexão antes da autenticação — o backup carrega a community SNMP
//! dos equipamentos, e entregá-lo a quem se interpôs no caminho até o NAS seria
//! entregar o acesso à rede.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use russh::client;
use russh::keys::ssh_key::{HashAlg, PublicKey};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::FileType;

use super::config::{join_key, SftpConfig, DEFAULT_SFTP_PORT};
use super::{
    BucketObject, ListOptions, ListPage, ObjectMetadata, ObjectReader, StorageError,
    StorageExplorer,
};

/// Teto para abrir a conexão TCP + handshake SSH. Um NAS desligado não pode
/// segurar o agendador até o timeout do sistema operacional.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Sessão parada por mais que isso é encerrada.
const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(120);

pub struct SftpExplorer {
    config: SftpConfig,
    prefix: String,
    /// Impressão digital apresentada pelo servidor na última conexão.
    observed: Arc<Mutex<Option<String>>>,
}

impl SftpExplorer {
    #[must_use]
    pub fn new(config: &SftpConfig) -> Self {
        Self {
            prefix: super::normalize_path(config.base_path.as_deref().unwrap_or_default()),
            config: config.clone(),
            observed: Arc::new(Mutex::new(None)),
        }
    }

    fn expected_fingerprint(&self) -> Option<String> {
        self.config
            .host_key_fingerprint
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    /// Caminho absoluto no servidor.
    ///
    /// Sempre com `/` inicial: um caminho relativo seria resolvido contra o
    /// diretório de login, que muda conforme a conta e tornaria o mesmo destino
    /// diferente para dois usuários.
    fn absolute(&self, key: &str) -> String {
        format!("/{}", join_key(&self.prefix, key))
    }

    /// Abre uma sessão SFTP autenticada.
    async fn connect(&self) -> Result<SftpSession, StorageError> {
        let config = Arc::new(client::Config {
            inactivity_timeout: Some(INACTIVITY_TIMEOUT),
            ..client::Config::default()
        });
        let address = (
            self.config.host.trim(),
            self.config.port.unwrap_or(DEFAULT_SFTP_PORT),
        );
        let expected = self.expected_fingerprint();
        let handler = ClientHandler {
            expected: expected.clone(),
            observed: Arc::clone(&self.observed),
        };

        let connected =
            tokio::time::timeout(CONNECT_TIMEOUT, client::connect(config, address, handler))
                .await
                .map_err(|_| {
                    StorageError::Backend(format!(
                        "Tempo esgotado ao conectar em {}:{}",
                        address.0, address.1
                    ))
                })?;
        let mut session = match connected {
            Ok(session) => session,
            Err(err) => {
                let observed = self.observed_identity();
                if let (Some(expected), Some(observed)) = (expected, observed) {
                    if expected != observed {
                        return Err(StorageError::Backend(format!(
                            "A identidade do servidor SFTP mudou (esperada {expected}, \
                             recebida {observed}). Se o servidor foi reinstalado, edite o \
                             armazenamento e aceite a nova identidade."
                        )));
                    }
                }
                return Err(StorageError::Backend(format!(
                    "Falha ao conectar por SSH: {err}"
                )));
            }
        };

        self.authenticate(&mut session).await?;

        let channel = session
            .channel_open_session()
            .await
            .map_err(|err| StorageError::Backend(format!("Falha ao abrir o canal SSH: {err}")))?;

        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|err| {
                StorageError::Backend(format!("O servidor não expôs o subsistema SFTP: {err}"))
            })?;

        SftpSession::new(channel.into_stream())
            .await
            .map_err(|err| StorageError::Backend(format!("Falha ao iniciar o SFTP: {err}")))
    }

    /// Autentica com a chave privada primeiro, senha depois.
    ///
    /// A ordem importa quando a config tem as duas: a chave é o método mais
    /// forte, e tentá-la primeiro evita gastar uma tentativa de senha em
    /// servidores que contam falhas de autenticação.
    async fn authenticate(
        &self,
        session: &mut client::Handle<ClientHandler>,
    ) -> Result<(), StorageError> {
        let user = self.config.username.as_str();

        if let Some(key) = self
            .config
            .private_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let passphrase = self
                .config
                .passphrase
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());

            let decoded = russh::keys::decode_secret_key(key, passphrase).map_err(|err| {
                StorageError::Backend(format!(
                    "Chave privada inválida ou passphrase errada: {err}"
                ))
            })?;

            let hash = session
                .best_supported_rsa_hash()
                .await
                .ok()
                .flatten()
                .flatten();

            let authenticated = session
                .authenticate_publickey(
                    user,
                    russh::keys::PrivateKeyWithHashAlg::new(Arc::new(decoded), hash),
                )
                .await
                .map_err(|err| StorageError::Backend(format!("Falha na autenticação: {err}")))?;

            if authenticated.success() {
                return Ok(());
            }

            // Sem cair para senha em silêncio: uma chave recusada é um problema
            // de configuração, e mascará-lo com um login por senha esconderia a
            // causa real na próxima vez que a senha mudasse.
            return Err(StorageError::Backend(
                "O servidor recusou a chave privada informada".to_string(),
            ));
        }

        if let Some(password) = self
            .config
            .password
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            let authenticated = session
                .authenticate_password(user, password)
                .await
                .map_err(|err| StorageError::Backend(format!("Falha na autenticação: {err}")))?;

            if authenticated.success() {
                return Ok(());
            }

            return Err(StorageError::Backend(
                "Usuário ou senha inválidos".to_string(),
            ));
        }

        Err(StorageError::Backend(
            "Informe uma senha ou uma chave privada para autenticar no SFTP".to_string(),
        ))
    }
}

/// Handler do cliente SSH: confere a chave do servidor contra a gravada.
///
/// Sem impressão digital gravada (primeiro uso), aceita e anota a que viu —
/// o serviço a grava junto da config depois que a conexão inteira deu certo.
struct ClientHandler {
    expected: Option<String>,
    observed: Arc<Mutex<Option<String>>>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = fingerprint_of(server_public_key);
        if let Ok(mut observed) = self.observed.lock() {
            *observed = Some(fingerprint.clone());
        }
        Ok(self
            .expected
            .as_deref()
            .is_none_or(|expected| expected == fingerprint))
    }
}

/// `SHA256:<base64>`, o mesmo formato do `ssh-keygen -lf`.
fn fingerprint_of(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

#[async_trait]
impl StorageExplorer for SftpExplorer {
    async fn list_objects(
        &self,
        path: &str,
        options: &ListOptions,
    ) -> Result<ListPage, StorageError> {
        let session = self.connect().await?;
        let target = self.absolute(path);
        let limit = options.effective_limit();

        let Ok(entries) = session.read_dir(&target).await else {
            // Diretório ausente vira página vazia, como no adapter local: a
            // interface trata "pasta vazia" e "pasta inexistente" igual.
            return Ok(ListPage::default());
        };

        let filter = options
            .prefix
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let mut objects = Vec::new();

        for entry in entries {
            let name = entry.file_name();

            // `.` e `..` são entradas reais do protocolo; exibi-las deixaria o
            // usuário navegar para fora do destino pela própria interface.
            if name == "." || name == ".." {
                continue;
            }

            if filter.is_some_and(|value| !name.starts_with(value)) {
                continue;
            }

            let key = join_key(&super::normalize_path(path), &name);
            let metadata = entry.metadata();
            let modified = metadata
                .modified()
                .ok()
                .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());

            objects.push(if entry.file_type() == FileType::Dir {
                BucketObject::directory(key)
            } else {
                let mut file = BucketObject::file(
                    key,
                    i64::try_from(metadata.size.unwrap_or(0)).unwrap_or(i64::MAX),
                    modified.clone(),
                );
                file.last_modified = modified;
                file
            });
        }

        // Mesma ordem estável do adapter local: o servidor devolve na ordem do
        // sistema de arquivos dele, e sem ordenar a paginação não é reprodutível.
        objects.sort_by(|a, b| a.key.cmp(&b.key));

        if let Some(cursor) = options.cursor.as_deref() {
            objects.retain(|object| object.key.as_str() > cursor);
        }

        let is_truncated = objects.len() > limit;
        objects.truncate(limit);

        let next_cursor = is_truncated
            .then(|| objects.last().map(|object| object.key.clone()))
            .flatten();

        Ok(ListPage {
            objects,
            next_cursor,
            is_truncated,
        })
    }

    async fn object_metadata(&self, key: &str) -> Result<ObjectMetadata, StorageError> {
        let session = self.connect().await?;

        let metadata = session
            .metadata(self.absolute(key))
            .await
            .map_err(|_| StorageError::NotFound(format!("Arquivo \"{key}\"")))?;

        Ok(ObjectMetadata {
            key: key.to_string(),
            size: i64::try_from(metadata.size.unwrap_or(0)).unwrap_or(i64::MAX),
            last_modified: metadata
                .modified()
                .ok()
                .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339()),
            content_type: None,
            etag: None,
        })
    }

    async fn delete_object(&self, key: &str, is_directory: bool) -> Result<(), StorageError> {
        let session = self.connect().await?;
        let target = self.absolute(key);

        if is_directory {
            remove_tree(&session, &target).await
        } else {
            session
                .remove_file(&target)
                .await
                .map_err(|err| StorageError::Backend(format!("Erro ao excluir \"{key}\": {err}")))
        }
    }

    async fn test_connection(&self) -> Result<(), StorageError> {
        let session = self.connect().await?;
        let target = self.absolute("");

        session.read_dir(&target).await.map_err(|err| {
            StorageError::Backend(format!("Diretório \"{target}\" não acessível: {err}"))
        })?;

        Ok(())
    }

    async fn put_file(&self, key: &str, source: &Path) -> Result<(), StorageError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let session = self.connect().await?;
        let target = self.absolute(key);

        // O SFTP não cria diretório intermediário sozinho, e o destino de um
        // backup é sempre `<id da conexão>/arquivo`.
        if let Some(parent) = target.rsplit_once('/').map(|(head, _)| head) {
            create_dir_all(&session, parent).await?;
        }

        let mut file = tokio::fs::File::open(source)
            .await
            .map_err(StorageError::backend)?;

        let mut remote = session
            .create(&target)
            .await
            .map_err(|err| StorageError::Backend(format!("Erro ao criar \"{key}\": {err}")))?;

        let mut chunk = vec![0_u8; 1024 * 1024];
        loop {
            let read = file.read(&mut chunk).await.map_err(StorageError::backend)?;
            if read == 0 {
                break;
            }

            remote
                .write_all(&chunk[..read])
                .await
                .map_err(StorageError::backend)?;
        }

        remote.flush().await.map_err(StorageError::backend)?;
        remote.shutdown().await.map_err(StorageError::backend)?;

        Ok(())
    }

    async fn read_object(&self, key: &str) -> Result<ObjectReader, StorageError> {
        let session = self.connect().await?;

        let file = session
            .open(self.absolute(key))
            .await
            .map_err(|_| StorageError::NotFound(format!("Arquivo \"{key}\"")))?;

        Ok(Box::pin(file))
    }

    fn observed_identity(&self) -> Option<String> {
        self.observed
            .lock()
            .ok()
            .and_then(|observed| observed.clone())
    }
}

/// Cria a árvore de diretórios, ignorando os que já existem.
async fn create_dir_all(session: &SftpSession, path: &str) -> Result<(), StorageError> {
    let mut current = String::new();

    for segment in path.split('/').filter(|value| !value.is_empty()) {
        current.push('/');
        current.push_str(segment);

        // Um diretório que já existe faz o `create_dir` falhar; a alternativa
        // seria consultar antes, o que custa uma ida ao servidor por segmento.
        let _ = session.create_dir(&current).await;
    }

    Ok(())
}

/// Remove um diretório e tudo dentro dele.
///
/// O protocolo SFTP não tem remoção recursiva: `rmdir` falha em diretório não
/// vazio. A varredura é iterativa em vez de recursiva porque uma função async
/// recursiva exigiria `Box::pin` a cada nível — e a profundidade aqui é a de
/// uma árvore de backups, que pode ser grande.
async fn remove_tree(session: &SftpSession, root: &str) -> Result<(), StorageError> {
    let mut to_visit = vec![root.to_string()];
    let mut directories = Vec::new();

    while let Some(current) = to_visit.pop() {
        let entries = session
            .read_dir(&current)
            .await
            .map_err(|err| StorageError::Backend(format!("Erro ao ler \"{current}\": {err}")))?;

        directories.push(current.clone());

        for entry in entries {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }

            let child = format!("{}/{name}", current.trim_end_matches('/'));

            if entry.file_type() == FileType::Dir {
                to_visit.push(child);
            } else {
                session.remove_file(&child).await.map_err(|err| {
                    StorageError::Backend(format!("Erro ao excluir \"{child}\": {err}"))
                })?;
            }
        }
    }

    // Do mais fundo para o mais raso: `rmdir` falha em diretório não vazio.
    for directory in directories.into_iter().rev() {
        session.remove_dir(&directory).await.map_err(|err| {
            StorageError::Backend(format!("Erro ao excluir \"{directory}\": {err}"))
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> SftpConfig {
        SftpConfig {
            host: "127.0.0.1".to_string(),
            port: Some(12222),
            username: "tester".to_string(),
            password: Some("tester".to_string()),
            base_path: Some("/home/tester/backups".to_string()),
            ..SftpConfig::default()
        }
    }

    #[test]
    fn builds_an_absolute_path_under_the_base() {
        // Caminho relativo seria resolvido contra o diretorio de login, que
        // muda conforme a conta.
        let explorer = SftpExplorer::new(&config());

        assert_eq!(
            explorer.absolute("12/vendas.sql.gz"),
            "/home/tester/backups/12/vendas.sql.gz"
        );
        assert_eq!(explorer.absolute(""), "/home/tester/backups");
    }

    #[test]
    fn a_destination_without_a_base_path_starts_at_the_root() {
        let explorer = SftpExplorer::new(&SftpConfig {
            host: "h".to_string(),
            username: "u".to_string(),
            ..SftpConfig::default()
        });

        assert_eq!(explorer.absolute("12/a.gz"), "/12/a.gz");
        assert_eq!(explorer.absolute(""), "/");
    }

    #[test]
    fn a_blank_fingerprint_counts_as_first_use() {
        let explorer = SftpExplorer::new(&SftpConfig {
            host: "h".to_string(),
            username: "u".to_string(),
            host_key_fingerprint: Some("  ".to_string()),
            ..SftpConfig::default()
        });

        assert_eq!(explorer.expected_fingerprint(), None);
        assert_eq!(explorer.observed_identity(), None);
    }

    #[tokio::test]
    async fn refuses_to_connect_without_any_credential() {
        // Sem senha nem chave, a mensagem precisa dizer o que falta — o
        // servidor recusaria com um erro de protocolo que nao ajuda ninguem.
        let explorer = SftpExplorer::new(&SftpConfig {
            host: "127.0.0.1".to_string(),
            port: Some(1),
            username: "tester".to_string(),
            ..SftpConfig::default()
        });

        let error = explorer.test_connection().await.expect_err("devia falhar");
        assert!(!error.message().is_empty());
    }

    #[test]
    fn the_default_port_is_the_ssh_one() {
        let explorer = SftpExplorer::new(&SftpConfig {
            host: "h".to_string(),
            username: "u".to_string(),
            ..SftpConfig::default()
        });

        assert_eq!(
            explorer.config.port.unwrap_or(DEFAULT_SFTP_PORT),
            DEFAULT_SFTP_PORT
        );
    }
}
