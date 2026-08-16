//! Contrato HTTP da tela de logs.
//!
//! **O envelope não é o `{data, meta}` paginado por número de página.** O
//! `useInfiniteList` decide o fim da lista por `meta.currentPage >=
//! meta.lastPage`, e um cursor não tem `lastPage` — nem poderia ter, porque
//! contar as linhas da janela custaria um `COUNT(*)` a cada rolagem sobre
//! milhões de registros. Fabricar número de página falso para caber no
//! composable existente quebraria na primeira linha inserida durante a
//! rolagem. Daí o envelope próprio e o `useInfiniteCursor.ts` no frontend.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Filtros de `GET /api/logs`, como chegam na query string.
///
/// Tudo opcional: a tela abre sem filtro nenhum e recebe as últimas 24 h.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsQuery {
    pub device_id: Option<i64>,
    /// Severidade numérica **máxima**. No syslog o número menor é o mais
    /// grave, então `3` significa "erro e acima" — erro, crítico, alerta e
    /// emergência.
    pub severity: Option<i16>,
    pub facility: Option<i16>,
    /// Início da janela, em RFC 3339. Ausente, valem as últimas 24 h.
    pub from: Option<String>,
    pub to: Option<String>,
    /// Busca literal na mensagem. `%` e `_` do usuário são escapados.
    pub q: Option<String>,
    /// Cursor opaco devolvido em `meta.nextCursor`.
    pub cursor: Option<String>,
    pub limit: Option<u64>,
}

/// Uma linha de log como a tela a consome.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LogEntry {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
    /// Nome do dispositivo, hidratado do banco principal. `null` quando a
    /// origem caiu numa rede cadastrada sem dispositivo, quando o vínculo é
    /// ambíguo, ou quando o dispositivo foi apagado depois — não há FK entre
    /// os dois bancos, então a linha sobrevive ao aparelho.
    pub device_name: Option<String>,
    pub source_ip: String,
    /// A verdade: quando **este** servidor recebeu. RFC 3339.
    pub received_at: String,
    /// O que o dispositivo alegou. Pode faltar, e pode estar errado — relógio
    /// de roteador sem NTP manda 1970.
    pub device_time: Option<String>,
    #[ts(type = "number | null")]
    pub facility: Option<i16>,
    #[ts(type = "number | null")]
    pub severity: Option<i16>,
    /// Rótulo pronto da severidade (`erro`, `aviso`, …), para a tela não
    /// reimplementar a tabela do RFC 5424.
    pub severity_label: Option<String>,
    pub hostname: Option<String>,
    pub app_name: Option<String>,
    #[ts(type = "number | null")]
    pub pid: Option<i32>,
    /// Tópicos do RouterOS, já quebrados em lista (`system`, `info`, …).
    pub topics: Vec<String>,
    pub message: String,
}

/// O `meta` do envelope por cursor.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LogPageMeta {
    /// Cursor da próxima página. `null` quando acabou — é este campo, e não uma
    /// contagem, que encerra a rolagem infinita.
    pub next_cursor: Option<String>,
    pub has_more: bool,
    #[ts(type = "number")]
    pub limit: u64,
    /// A janela efetivamente consultada, depois de aplicados o padrão de 24 h e
    /// o teto de 7 dias. A tela mostra isto para o usuário não achar que está
    /// vendo o período inteiro que pediu.
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LogPageResponse {
    pub data: Vec<LogEntry>,
    pub meta: LogPageMeta,
}
