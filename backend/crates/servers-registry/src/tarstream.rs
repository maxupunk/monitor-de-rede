//! `tar` em fluxo, assíncrono: o arquivo OCI que o agente manda e o que o
//! `docker load` do host recebe nunca ficam inteiros na memória (RNF-01).
//!
//! O formato do cabeçalho é do crate `tar` (`tar::Header`); aqui só se lê e se
//! escreve em blocos de 512 bytes sobre `AsyncRead`/`AsyncWrite`. Nomes longos
//! chegam por extensão PAX (`path=`) ou GNU (`././@LongLink`).

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const BLOCK: usize = 512;
/// Teto de um cabeçalho estendido (PAX/GNU): é só um nome.
const MAX_EXTENDED: u64 = 64 * 1024;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

fn padding(size: u64) -> usize {
    let rest = (size % BLOCK as u64) as usize;
    if rest == 0 {
        0
    } else {
        BLOCK - rest
    }
}

/// Um arquivo dentro do tar: o nome e quanto ler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub size: u64,
    /// Arquivo comum (o resto — pastas, links — é pulado por quem lê).
    pub regular: bool,
}

/// Leitor de tar sobre um fluxo.
pub struct Reader<R> {
    inner: R,
    /// Bytes do conteúdo da entrada atual que ainda não foram lidos.
    remaining: u64,
    pad: usize,
    finished: bool,
}

impl<R: AsyncRead + Unpin> Reader<R> {
    pub const fn new(inner: R) -> Self {
        Self {
            inner,
            remaining: 0,
            pad: 0,
            finished: false,
        }
    }

    async fn skip(&mut self, mut count: u64) -> io::Result<()> {
        let mut buffer = [0_u8; 8192];
        while count > 0 {
            let take = usize::try_from(count.min(buffer.len() as u64)).unwrap_or(buffer.len());
            self.inner.read_exact(&mut buffer[..take]).await?;
            count -= take as u64;
        }
        Ok(())
    }

    /// Termina a entrada atual (o que sobrou e o enchimento).
    async fn close_entry(&mut self) -> io::Result<()> {
        let rest = self.remaining + self.pad as u64;
        self.remaining = 0;
        self.pad = 0;
        self.skip(rest).await
    }

    async fn read_extended(&mut self, size: u64) -> io::Result<Vec<u8>> {
        if size > MAX_EXTENDED {
            return Err(invalid("cabeçalho estendido grande demais"));
        }
        let mut data = vec![0_u8; usize::try_from(size).unwrap_or(0)];
        self.inner.read_exact(&mut data).await?;
        self.skip(padding(size) as u64).await?;
        Ok(data)
    }

    /// A próxima entrada, ou `None` no fim do arquivo.
    ///
    /// # Errors
    ///
    /// Fluxo quebrado ou cabeçalho inválido.
    pub async fn next(&mut self) -> io::Result<Option<Entry>> {
        if self.finished {
            return Ok(None);
        }
        self.close_entry().await?;
        let mut long_name: Option<String> = None;
        loop {
            let mut block = [0_u8; BLOCK];
            match self.inner.read_exact(&mut block).await {
                Ok(_) => {}
                // Arquivo sem os blocos finais: termina aqui mesmo.
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                    self.finished = true;
                    return Ok(None);
                }
                Err(error) => return Err(error),
            }
            if block.iter().all(|byte| *byte == 0) {
                self.finished = true;
                return Ok(None);
            }
            let header = tar::Header::from_byte_slice(&block);
            let computed: u32 = block
                .iter()
                .enumerate()
                .map(|(index, byte)| {
                    if (148..156).contains(&index) {
                        32
                    } else {
                        u32::from(*byte)
                    }
                })
                .sum();
            if header.cksum().ok() != Some(computed) {
                return Err(invalid("cabeçalho de tar com checksum errado"));
            }
            let size = header
                .entry_size()
                .map_err(|_| invalid("tamanho inválido"))?;
            let kind = header.entry_type();
            if kind.is_pax_local_extensions() {
                let data = self.read_extended(size).await?;
                long_name = pax_path(&data).or(long_name);
                continue;
            }
            if kind.is_gnu_longname() {
                let data = self.read_extended(size).await?;
                let name = String::from_utf8_lossy(&data);
                long_name = Some(name.trim_end_matches('\0').to_string());
                continue;
            }
            if kind.is_pax_global_extensions() {
                self.read_extended(size).await?;
                continue;
            }
            let path = long_name
                .take()
                .unwrap_or_else(|| String::from_utf8_lossy(&header.path_bytes()).into_owned());
            self.remaining = size;
            self.pad = padding(size);
            return Ok(Some(Entry {
                path: path.trim_start_matches("./").to_string(),
                size,
                regular: kind.is_file(),
            }));
        }
    }

    /// Lê até `buffer.len()` bytes do conteúdo da entrada atual; 0 no fim
    /// dela.
    ///
    /// # Errors
    ///
    /// Fluxo quebrado.
    pub async fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Ok(0);
        }
        let take = usize::try_from(self.remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let count = self.inner.read(&mut buffer[..take]).await?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "o tar terminou no meio de um arquivo",
            ));
        }
        self.remaining -= count as u64;
        Ok(count)
    }

    /// O conteúdo inteiro da entrada atual, até `limit` bytes.
    ///
    /// # Errors
    ///
    /// Entrada maior que `limit` ou fluxo quebrado.
    pub async fn read_all(&mut self, limit: u64) -> io::Result<Vec<u8>> {
        if self.remaining > limit {
            return Err(invalid("arquivo grande demais"));
        }
        let mut data = vec![0_u8; usize::try_from(self.remaining).unwrap_or(0)];
        let mut filled = 0;
        while filled < data.len() {
            let count = self.read(&mut data[filled..]).await?;
            filled += count;
        }
        Ok(data)
    }
}

fn pax_path(data: &[u8]) -> Option<String> {
    // Registros `<tamanho> chave=valor\n`.
    let text = String::from_utf8_lossy(data);
    text.lines().find_map(|record| {
        let (_, pair) = record.split_once(' ')?;
        pair.strip_prefix("path=").map(ToString::to_string)
    })
}

/// Escritor de tar sobre um fluxo.
pub struct Writer<W> {
    inner: W,
}

impl<W: AsyncWrite + Unpin> Writer<W> {
    pub const fn new(inner: W) -> Self {
        Self { inner }
    }

    /// Abre um arquivo comum de `size` bytes; o conteúdo vem em seguida por
    /// [`write_data`](Self::write_data) e fecha com [`end_file`](Self::end_file).
    ///
    /// # Errors
    ///
    /// Nome inválido para o formato ou fluxo quebrado.
    pub async fn begin_file(&mut self, path: &str, size: u64) -> io::Result<()> {
        let mut header = tar::Header::new_ustar();
        header.set_path(path)?;
        header.set_size(size);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        self.inner.write_all(header.as_bytes()).await
    }

    /// # Errors
    ///
    /// Fluxo quebrado.
    pub async fn write_data(&mut self, data: &[u8]) -> io::Result<()> {
        self.inner.write_all(data).await
    }

    /// Fecha o arquivo de `size` bytes com o enchimento até o bloco.
    ///
    /// # Errors
    ///
    /// Fluxo quebrado.
    pub async fn end_file(&mut self, size: u64) -> io::Result<()> {
        let pad = padding(size);
        if pad > 0 {
            self.inner.write_all(&[0_u8; BLOCK][..pad]).await?;
        }
        Ok(())
    }

    /// Um arquivo pequeno de uma vez.
    ///
    /// # Errors
    ///
    /// Fluxo quebrado.
    pub async fn file(&mut self, path: &str, data: &[u8]) -> io::Result<()> {
        let size = data.len() as u64;
        self.begin_file(path, size).await?;
        self.write_data(data).await?;
        self.end_file(size).await
    }

    /// Os dois blocos zerados do fim.
    ///
    /// # Errors
    ///
    /// Fluxo quebrado.
    pub async fn finish(mut self) -> io::Result<W> {
        self.inner.write_all(&[0_u8; BLOCK * 2]).await?;
        self.inner.flush().await?;
        Ok(self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ida_e_volta_com_nome_longo() {
        let long = format!("blobs/sha256/{}/{}", "a".repeat(64), "b".repeat(60));
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(3);
        header.set_mode(0o644);
        builder
            .append_data(&mut header, &long, &b"abc"[..])
            .unwrap();
        let produced = builder.into_inner().unwrap();

        let mut reader = Reader::new(&produced[..]);
        let entry = reader.next().await.unwrap().unwrap();
        assert_eq!(entry.path, long);
        assert_eq!(reader.read_all(10).await.unwrap(), b"abc");
        assert!(reader.next().await.unwrap().is_none());

        let mut writer = Writer::new(Vec::new());
        writer.file("index.json", b"{}").await.unwrap();
        writer.begin_file("blobs/x", 5).await.unwrap();
        writer.write_data(b"12").await.unwrap();
        writer.write_data(b"345").await.unwrap();
        writer.end_file(5).await.unwrap();
        let bytes = writer.finish().await.unwrap();
        let mut archive = tar::Archive::new(&bytes[..]);
        let names: Vec<String> = archive
            .entries()
            .unwrap()
            .map(|entry| entry.unwrap().path().unwrap().display().to_string())
            .collect();
        assert_eq!(names, vec!["index.json", "blobs/x"]);
    }

    #[tokio::test]
    async fn conteudo_nao_lido_e_pulado() {
        let mut writer = Writer::new(Vec::new());
        writer.file("a", &[1_u8; 700]).await.unwrap();
        writer.file("b", b"ok").await.unwrap();
        let bytes = writer.finish().await.unwrap();
        let mut reader = Reader::new(&bytes[..]);
        reader.next().await.unwrap().unwrap();
        let second = reader.next().await.unwrap().unwrap();
        assert_eq!(second.path, "b");
        assert_eq!(reader.read_all(10).await.unwrap(), b"ok");
    }
}
