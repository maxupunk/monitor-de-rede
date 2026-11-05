//! O arquivo do dump: SQL → SHA-256 → gzip → disco.
//!
//! ## Nada é bufferizado
//!
//! Cada pedaço que o driver escreve passa pelo hash e pelo gzip e vai para o
//! arquivo. Um `Vec` com o dump inteiro faria o pico de memória acompanhar o
//! tamanho do banco: um banco de 40 GB derrubaria o processo — e só no cliente
//! maior. Com a escrita em fluxo, o disco lento segura a leitura, que segura o
//! servidor: o backpressure vem de graça.
//!
//! ## O checksum é dos bytes **descomprimidos**
//!
//! O gzip leva timestamp no cabeçalho: o mesmo dump comprimido duas vezes dá
//! arquivos diferentes. A soma do SQL é estável, e é ela que a restauração
//! confere antes de confirmar a transação.

use std::path::{Path, PathBuf};

use async_compression::tokio::write::GzipEncoder;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncWriteExt, BufWriter};

use super::{DatabaseEngine, DatabaseError};

/// O que um dump produziu, além do arquivo.
#[derive(Debug, Clone, Default)]
pub struct DumpStats {
    pub tables: u32,
    pub rows: u64,
    /// Objetos que existem no banco e que o dump não leva — o operador precisa
    /// saber antes de confiar na cópia.
    pub warnings: Vec<String>,
}

/// O arquivo pronto.
#[derive(Debug, Clone)]
pub struct DumpFile {
    pub path: PathBuf,
    /// Tamanho no disco (compactado).
    pub size: u64,
    /// SHA-256 hex do SQL descompactado.
    pub checksum: String,
}

/// Escritor do dump.
pub struct DumpWriter {
    encoder: GzipEncoder<BufWriter<tokio::fs::File>>,
    hasher: Sha256,
    bytes: u64,
    path: PathBuf,
}

impl DumpWriter {
    /// Cria o arquivo em `path`.
    ///
    /// # Errors
    ///
    /// Falha do sistema de arquivos.
    pub async fn create(path: &Path) -> Result<Self, DatabaseError> {
        let file = tokio::fs::File::create(path)
            .await
            .map_err(DatabaseError::io)?;
        Ok(Self {
            encoder: GzipEncoder::new(BufWriter::with_capacity(256 * 1024, file)),
            hasher: Sha256::new(),
            bytes: 0,
            path: path.to_path_buf(),
        })
    }

    /// Escreve bytes crus (o fluxo do `COPY`).
    ///
    /// # Errors
    ///
    /// Falha de escrita.
    pub async fn write(&mut self, data: &[u8]) -> Result<(), DatabaseError> {
        self.hasher.update(data);
        self.bytes += data.len() as u64;
        self.encoder
            .write_all(data)
            .await
            .map_err(DatabaseError::io)
    }

    /// Uma linha de texto.
    ///
    /// # Errors
    ///
    /// Falha de escrita.
    pub async fn line(&mut self, text: &str) -> Result<(), DatabaseError> {
        self.write(text.as_bytes()).await?;
        self.write(b"\n").await
    }

    /// Um comando completo, terminado em `;`.
    ///
    /// # Errors
    ///
    /// Falha de escrita.
    pub async fn statement(&mut self, sql: &str) -> Result<(), DatabaseError> {
        let sql = sql.trim_end();
        let sql = sql.strip_suffix(';').unwrap_or(sql).trim_end();
        self.write(sql.as_bytes()).await?;
        self.write(b";\n").await
    }

    /// Comentário de seção, para quem abrir o arquivo à mão.
    ///
    /// # Errors
    ///
    /// Falha de escrita.
    pub async fn section(&mut self, title: &str) -> Result<(), DatabaseError> {
        self.line("").await?;
        self.line(&format!("-- {title}")).await
    }

    /// Bytes de SQL escritos até agora (descompactados).
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Cabeçalho comum aos dois SGBDs.
    ///
    /// # Errors
    ///
    /// Falha de escrita.
    pub async fn header(
        &mut self,
        engine: DatabaseEngine,
        version: &str,
        database: &str,
    ) -> Result<(), DatabaseError> {
        self.line(&format!(
            "-- NetMonitor — dump nativo de {}",
            engine.label()
        ))
        .await?;
        self.line(&format!("-- Servidor: {version}")).await?;
        self.line(&format!("-- Banco: {database}")).await?;
        self.line(&format!(
            "-- Gerado em: {}",
            chrono::Utc::now().to_rfc3339()
        ))
        .await?;
        self.line("-- Sem dono nem permissões: os objetos ficam com quem restaurar.")
            .await
    }

    /// Fecha o gzip e devolve o arquivo com a soma.
    ///
    /// # Errors
    ///
    /// Falha ao terminar a escrita.
    pub async fn finish(mut self) -> Result<DumpFile, DatabaseError> {
        self.encoder.shutdown().await.map_err(DatabaseError::io)?;
        let size = tokio::fs::metadata(&self.path)
            .await
            .map_err(DatabaseError::io)?
            .len();
        Ok(DumpFile {
            path: self.path,
            size,
            checksum: hex::encode(self.hasher.finalize()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_compression::tokio::bufread::GzipDecoder;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn o_arquivo_descompacta_no_sql_escrito_e_a_soma_e_dele() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dump.sql.gz");
        let mut writer = DumpWriter::create(&path).await.unwrap();
        writer.statement("CREATE TABLE t (a int);").await.unwrap();
        writer.write(b"1\n2\n").await.unwrap();
        let file = writer.finish().await.unwrap();

        let compressed = tokio::fs::read(&path).await.unwrap();
        assert_eq!(file.size, compressed.len() as u64);
        let mut sql = String::new();
        GzipDecoder::new(&compressed[..])
            .read_to_string(&mut sql)
            .await
            .unwrap();
        assert_eq!(sql, "CREATE TABLE t (a int);\n1\n2\n");
        assert_eq!(file.checksum, hex::encode(Sha256::digest(sql.as_bytes())));
    }
}
