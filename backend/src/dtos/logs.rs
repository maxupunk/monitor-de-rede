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

/// Uma origem vista pelo servidor desde o último reinício.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LogSourceEntry {
    pub source_ip: String,
    /// A chave a devolver no `bind` — o IP, ou `host:<hostname>` quando a
    /// origem chega mascarada por NAT. A tela não monta esta chave sozinha: só
    /// o servidor sabe se o endereço é gateway.
    pub bind_key: String,
    /// Se o `sourceIp` é o gateway de um NAT em vez do remetente real. Quando
    /// verdadeiro, o endereço é o mesmo para todos os equipamentos e só o
    /// hostname os separa.
    pub masked: bool,
    /// `device` | `network` | `ambiguous` | `unknown`.
    pub kind: String,
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
    pub device_name: Option<String>,
    /// Candidatos quando o mesmo IP existe em mais de um dispositivo. É o que a
    /// tela oferece no vínculo manual.
    #[ts(type = "Array<{ id: number, name: string }>")]
    pub candidates: Vec<LogSourceCandidate>,
    pub hostname: Option<String>,
    #[ts(type = "number")]
    pub message_count: u64,
    /// Quantas foram descartadas por não resolver para nada cadastrado.
    #[ts(type = "number")]
    pub dropped_count: u64,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub last_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LogSourceCandidate {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LogSourcesResponse {
    pub data: Vec<LogSourceEntry>,
    /// Quantas origens estão descartando mensagem agora. É o número do banner
    /// da tela — sem ele, a regra "fonte desconhecida não grava" vira um buraco
    /// negro de suporte.
    #[ts(type = "number")]
    pub unknown_count: usize,
    /// Contadores de ingestão desde o boot.
    pub metrics: LogIngestMetrics,
    /// Diagnóstico do NAT. Vem sempre, para a tela poder explicar um parque
    /// inteiro "desconhecido" sem o operador ter de adivinhar.
    pub nat: LogNatDiagnostics,
}

/// O que o servidor sabe sobre o mascaramento da origem.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LogNatDiagnostics {
    /// Quantas origens chegam com o endereço reescrito.
    #[ts(type = "number")]
    pub masked_count: usize,
    /// Dessas, quantas ainda **não** estão vinculadas a um dispositivo.
    ///
    /// É este número, e não o de cima, que decide se o aviso aparece. O
    /// mascaramento continua existindo depois de tudo vinculado — mas aí ele
    /// deixou de atrapalhar, e um aviso permanente sobre um problema resolvido
    /// treina o operador a ignorar avisos.
    #[ts(type = "number")]
    pub unresolved_masked_count: usize,
    /// Se o processo está dentro de um container — muda o texto da orientação,
    /// não o comportamento.
    pub containerized: bool,
    /// Os endereços reconhecidos como gateway, para o aviso poder nomeá-los.
    pub gateways: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LogIngestMetrics {
    #[ts(type = "number")]
    pub received: u64,
    #[ts(type = "number")]
    pub stored: u64,
    #[ts(type = "number")]
    pub dropped_queue_full: u64,
    #[ts(type = "number")]
    pub dropped_rate_limit: u64,
    #[ts(type = "number")]
    pub dropped_unknown_source: u64,
    #[ts(type = "number")]
    pub dropped_oversized: u64,
}

/// Corpo de `POST /api/logs/sources/{ip}/bind`.
///
/// `deviceId` nulo **desfaz** o vínculo — é o mesmo endpoint, e não um `DELETE`
/// separado, porque a tela oferece um seletor onde "nenhum" é uma opção.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindSourceInput {
    pub device_id: Option<i64>,
}

/// Corpo de `POST /api/logs/devices/{id}/provision`.
///
/// **Estes campos não são persistidos.** `username` e `password` existem
/// enquanto a requisição dura e morrem com ela — não há coluna, cache nem
/// `system_settings` que os receba, nem cifrados. Ver a nota do módulo
/// [`crate::services::syslog::provision`]. O DTO fica sem `Debug` derivado de
/// propósito: um `tracing` distraído despejaria a senha no log.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionLoggingInput {
    /// `ssh`, `telnet` ou `mactelnet`.
    pub protocol: String,
    /// Ausente cai na porta padrão do protocolo (22, 23 ou 20561).
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
    /// Id do catálogo de sistemas. Ausente, o servidor deduz do cadastro.
    pub operating_system: Option<String>,
    /// Endereço deste servidor como o **equipamento** o alcança. Vazio, ou
    /// `localhost`, faz o servidor descobrir sozinho: gravar `localhost` no
    /// roteador o faria mandar o log para si mesmo, sem erro e sem nada
    /// chegando aqui.
    pub server_address: Option<String>,
    /// MAC do equipamento, para o MAC-Telnet. Ausente, o servidor procura nas
    /// interfaces coletadas e no resultado da descoberta.
    pub mac_address: Option<String>,
}

/// O que a tela usa para se preencher antes de perguntar qualquer coisa.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ProvisionHintsResponse {
    /// Endereço deste servidor que o equipamento deve usar. `null` quando não
    /// há resposta confiável — aí a tela exige que o operador digite.
    pub server_address: Option<String>,
    /// De onde veio o palpite, para a tela não apresentar chute como certeza.
    pub server_address_source: String,
    /// Qual entrada da lista de endereços do servidor serve a **este**
    /// equipamento. Nulo quando não há evidência para escolher — a tela então
    /// pergunta ao operador em vez de chutar um endereço.
    pub suggested_address_id: Option<String>,
    /// Por que aquela foi sugerida — "este servidor alcança o equipamento por
    /// 10.8.0.1". O motivo é o que dispensa o operador de entender a lista.
    pub suggested_address_reason: Option<String>,
    /// Forma de acesso em vigor: `local`, `vpn` ou `remote`.
    pub access_mode: String,
    /// Se ela veio do cadastro. A tela diz "definido no cadastro" em vez de
    /// "deduzido", e as duas frases pedem confiança diferente do operador.
    pub access_mode_declared: bool,
    /// Por que esta forma de acesso.
    pub access_mode_reason: String,
    #[ts(type = "number")]
    pub server_port: u16,
    /// Sistema em vigor — id do catálogo de `services::devices::systems`.
    pub operating_system: String,
    /// De onde ele veio: `declarado`, `snmp`, `sonda`, `cadastro` ou `padrão`.
    pub operating_system_source: String,
    /// A frase que explica a conclusão — "o servidor SSH se identifica como
    /// `dropbear`, que é o padrão do OpenWrt". Sem ela a tela só afirma.
    pub operating_system_reason: String,
    /// Porta 22 respondeu à sondagem.
    pub ssh_open: bool,
    /// Porta 23 respondeu à sondagem.
    pub telnet_open: bool,
    pub mac_address: Option<String>,
    /// Se este processo alcança a camada 2 da rede do equipamento. Falso num
    /// container em rede bridge, onde o MAC-Telnet não tem como funcionar.
    pub layer2_reachable: bool,
}

/// O que a tela mostra depois da ativação automática.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ProvisionLoggingResponse {
    pub operating_system: String,
    /// O endereço que foi gravado no equipamento.
    pub server_address: String,
    #[ts(type = "number")]
    pub server_port: u16,
    /// Os comandos enviados. Nunca a credencial.
    pub commands: Vec<String>,
    /// O que o equipamento respondeu, com a senha raspada.
    pub transcript: String,
    /// Se chegou log do dispositivo antes do teto de espera. `null` quando não
    /// havia como confirmar (ingestão desligada).
    pub confirmed: Option<bool>,
}

/// Resposta do vínculo/desvínculo manual de uma origem de syslog.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct BindSourceResponse {
    pub bind_key: String,
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
}

/// Filtros do live tail (`GET /api/logs/stream`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogStreamQuery {
    pub device_id: Option<i64>,
    pub severity: Option<i16>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_source_response_serializa_em_camel_case() {
        let response = BindSourceResponse {
            bind_key: "192.168.1.1".into(),
            device_id: Some(7),
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["bindKey"], "192.168.1.1");
        assert_eq!(json["deviceId"], 7);
    }
}
