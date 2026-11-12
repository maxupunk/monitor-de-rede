//! O pacote `.zip` da exportação: um arquivo por vez, comprimido, em memória.
//!
//! Só sabe escrever arquivos — o que vai em cada um é decidido em `mod.rs`.

use std::io::{Cursor, Write};

use serde::Serialize;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use netmonitor_core::shared::errors::{AppError, AppResult};

fn internal(error: impl std::error::Error + Send + Sync + 'static) -> AppError {
    AppError::Internal(anyhow::Error::new(error))
}

/// Linhas JSON (JSONL): um objeto por linha.
#[derive(Debug, Default)]
pub struct JsonLines {
    buffer: Vec<u8>,
    lines: usize,
}

impl JsonLines {
    /// # Errors
    ///
    /// Valor que não serializa.
    pub fn push(&mut self, value: &impl Serialize) -> AppResult<()> {
        serde_json::to_writer(&mut self.buffer, value).map_err(internal)?;
        self.buffer.push(b'\n');
        self.lines += 1;
        Ok(())
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.lines
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.lines == 0
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }
}

/// O `.zip` em construção.
pub struct Bundle {
    writer: ZipWriter<Cursor<Vec<u8>>>,
}

impl Default for Bundle {
    fn default() -> Self {
        Self::new()
    }
}

impl Bundle {
    #[must_use]
    pub fn new() -> Self {
        Self {
            writer: ZipWriter::new(Cursor::new(Vec::new())),
        }
    }

    /// Abre o próximo arquivo; o que for escrito depois vai para ele.
    ///
    /// # Errors
    ///
    /// Falha do compressor.
    pub fn begin(&mut self, name: &str) -> AppResult<()> {
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        self.writer.start_file(name, options).map_err(internal)
    }

    /// Uma linha JSONL no arquivo aberto — para o que é grande demais para
    /// juntar antes (as conversas).
    ///
    /// # Errors
    ///
    /// Valor que não serializa ou falha do compressor.
    pub fn write_line(&mut self, value: &impl Serialize) -> AppResult<()> {
        serde_json::to_writer(&mut self.writer, value).map_err(internal)?;
        self.writer.write_all(b"\n").map_err(internal)
    }

    /// # Errors
    ///
    /// Falha do compressor.
    pub fn add(&mut self, name: &str, bytes: &[u8]) -> AppResult<()> {
        self.begin(name)?;
        self.writer.write_all(bytes).map_err(internal)
    }

    /// # Errors
    ///
    /// Valor que não serializa ou falha do compressor.
    pub fn add_json(&mut self, name: &str, value: &impl Serialize) -> AppResult<()> {
        let bytes = serde_json::to_vec_pretty(value).map_err(internal)?;
        self.add(name, &bytes)
    }

    /// # Errors
    ///
    /// Falha do compressor.
    pub fn finish(self) -> AppResult<Vec<u8>> {
        Ok(self.writer.finish().map_err(internal)?.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use serde_json::json;

    use super::*;

    #[test]
    fn pacote_guarda_os_arquivos_que_recebeu() {
        let mut lines = JsonLines::default();
        lines.push(&json!({ "a": 1 })).unwrap();
        lines.push(&json!({ "b": 2 })).unwrap();
        assert_eq!(lines.len(), 2);

        let mut bundle = Bundle::new();
        bundle.begin("direto.jsonl").unwrap();
        bundle.write_line(&json!({ "c": 3 })).unwrap();
        bundle.add("linhas.jsonl", &lines.into_bytes()).unwrap();
        bundle
            .add_json("manifesto.json", &json!({ "ok": true }))
            .unwrap();
        let bytes = bundle.finish().unwrap();

        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut content = String::new();
        archive
            .by_name("linhas.jsonl")
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "{\"a\":1}\n{\"b\":2}\n");
        content.clear();
        archive
            .by_name("direto.jsonl")
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "{\"c\":3}\n");
        assert!(archive.by_name("manifesto.json").is_ok());
    }
}
