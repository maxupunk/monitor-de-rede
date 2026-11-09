//! Execução de um CLI no host do agente (`docker compose`, `git`), com cada
//! linha de saída repassada como progresso.
//!
//! Só o agente chama isto (ADR 010, ADR 014): o processo da API nunca
//! executa `docker` nem `git`. Os argumentos vão um a um — nunca por shell —
//! e segredos vão por variável de ambiente do processo filho, nunca na linha
//! de comando, que qualquer `ps` do host mostraria.

use std::{path::Path, process::Stdio, time::Duration};

use tokio::io::{AsyncBufReadExt, BufReader};

use super::{maintenance::Progress, DockerError};

/// Linhas finais guardadas para a mensagem de erro.
const TAIL_LINES: usize = 20;

/// O que o comando deixou: se terminou bem e as últimas linhas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutcome {
    pub success: bool,
    pub tail: Vec<String>,
}

impl CliOutcome {
    /// As últimas linhas numa só, para a mensagem de erro.
    #[must_use]
    pub fn summary(&self) -> String {
        self.tail.join(" | ")
    }
}

/// O comando a executar.
pub struct Cli<'a> {
    pub program: &'a str,
    pub args: &'a [String],
    pub envs: &'a [(String, String)],
    pub cwd: Option<&'a Path>,
    pub timeout: Duration,
}

impl<'a> Cli<'a> {
    #[must_use]
    pub const fn new(program: &'a str, args: &'a [String], timeout: Duration) -> Self {
        Self {
            program,
            args,
            envs: &[],
            cwd: None,
            timeout,
        }
    }

    #[must_use]
    pub const fn envs(mut self, envs: &'a [(String, String)]) -> Self {
        self.envs = envs;
        self
    }

    #[must_use]
    pub const fn cwd(mut self, cwd: &'a Path) -> Self {
        self.cwd = Some(cwd);
        self
    }

    /// Executa e repassa stdout e stderr, linha a linha, para `progress`.
    ///
    /// # Errors
    ///
    /// `Unavailable` quando o programa não existe ou estoura o tempo;
    /// `Engine` quando a saída não pode ser lida.
    pub async fn run(self, progress: &Progress) -> Result<CliOutcome, DockerError> {
        let mut command = tokio::process::Command::new(self.program);
        command
            .args(self.args)
            .envs(self.envs.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = self.cwd {
            command.current_dir(cwd);
        }
        let mut child = command.spawn().map_err(|_| DockerError::Unavailable)?;
        let stdout = child.stdout.take().ok_or(DockerError::Engine)?;
        let stderr = child.stderr.take().ok_or(DockerError::Engine)?;
        let (lines_tx, mut lines_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        for reader in [
            Box::new(stdout) as Box<dyn tokio::io::AsyncRead + Unpin + Send>,
            Box::new(stderr),
        ] {
            let lines_tx = lines_tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(reader).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if lines_tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        drop(lines_tx);

        let mut tail: Vec<String> = Vec::new();
        let collect = async {
            while let Some(line) = lines_rx.recv().await {
                let _ = progress.send(line.clone());
                tail.push(line);
                if tail.len() > TAIL_LINES {
                    tail.remove(0);
                }
            }
            child.wait().await
        };
        let status = tokio::time::timeout(self.timeout, collect)
            .await
            .map_err(|_| DockerError::Unavailable)?
            .map_err(|_| DockerError::Engine)?;
        Ok(CliOutcome {
            success: status.success(),
            tail,
        })
    }
}

/// A saída inteira de um comando curto (`git rev-parse`, `--version`), sem
/// progresso.
///
/// # Errors
///
/// `Unavailable` quando o programa não existe ou estoura o tempo.
pub async fn capture(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Result<(bool, String), DockerError> {
    let mut command = tokio::process::Command::new(program);
    command.args(args).stdin(Stdio::null()).kill_on_drop(true);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| DockerError::Unavailable)?
        .map_err(|_| DockerError::Unavailable)?;
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}
