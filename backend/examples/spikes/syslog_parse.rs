//! **SPIKE-06 — syslog** (Fase 1 do `docs/roadmap_syslog_nativo.md`).
//!
//! Resultados registrados em [`docs/adr/008-syslog-parser.md`].
//!
//! Quatro perguntas, uma por modo de execução:
//!
//! 1. `syslog_loose` parseia o que os roteadores realmente mandam — inclusive o
//!    RouterOS **sem** `bsd-syslog=yes`, que não é RFC 3164 nem 5424?
//! 2. Quanto custa gravar 500 linhas em lote no SQLite em WAL?
//! 3. Qual é o IP de origem **observado dentro do container**? Toda a regra de
//!    "fonte desconhecida não grava" depende dele ser o IP do roteador; se a
//!    publicação de porta do Docker mascarar a origem, o sistema não gravaria
//!    nada e não haveria erro visível em lugar nenhum.
//! 4. O filtro composto responde em tempo de tela com 1 M de linhas? — o
//!    critério de aceite da Fase 3.
//!
//! ```sh
//! cargo run --example spike_syslog_parse            # amostras
//! cargo run --example spike_syslog_parse -- bench   # inserção em lote
//! cargo run --example spike_syslog_parse -- listen [porta]
//! cargo run --release --example spike_syslog_parse -- query
//! ```

use std::time::Instant;

use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Utc};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use syslog_loose::{
    decompose_pri, IncompleteDate, Message, ProcId, SyslogFacility, SyslogSeverity, Variant,
};

/// Amostras reais, uma por linha, com o rótulo do que se espera provar.
const AMOSTRAS: &[(&str, &str)] = &[
    // --- RouterOS com `bsd-syslog=yes` -------------------------------------
    // O caminho feliz: RFC 3164 legítimo. Repare que os tópicos do RouterOS
    // caem no lugar do `tag` do BSD (`system,info,account`), porque o parser
    // pega o primeiro token depois do hostname.
    (
        "RouterOS bsd-syslog · login",
        "<134>Aug 15 10:23:45 MikroTik-CCR system,info,account user admin logged in from 192.168.88.50 via winbox",
    ),
    (
        "RouterOS bsd-syslog · falha de login",
        "<131>Aug 15 10:24:01 MikroTik-CCR system,error,critical login failure for user admin from 203.0.113.7 via ssh",
    ),
    (
        "RouterOS bsd-syslog · firewall",
        "<134>Aug 15 10:25:13 MikroTik-CCR firewall,info input: in:ether1 out:(none), proto TCP (SYN), 203.0.113.9:44321->10.0.0.1:22, len 60",
    ),
    (
        "RouterOS bsd-syslog · interface",
        "<132>Aug 15 10:27:30 MikroTik-CCR interface,info ether5 link down",
    ),
    (
        "RouterOS bsd-syslog · pppoe",
        "<134>Aug 15 10:28:00 MikroTik-CCR pppoe,ppp,info <pppoe-cliente1>: terminating... - peer is not responding",
    ),
    // --- RouterOS SEM `bsd-syslog=yes` -------------------------------------
    // Formato próprio: `<pri>` colado nos tópicos, sem timestamp e sem
    // hostname. Não é RFC nenhuma, e é o caso que decide se o snippet de
    // configuração pode ou não deixar a flag opcional.
    (
        "RouterOS cru (sem bsd-syslog)",
        "<134>system,info,account user admin logged in from 192.168.88.50 via winbox",
    ),
    // --- OpenWRT -----------------------------------------------------------
    (
        "OpenWRT · kernel",
        "<4>Aug 15 10:30:12 OpenWrt kernel: [12345.678901] br-lan: port 2(eth0.1) entered disabled state",
    ),
    (
        "OpenWRT · dnsmasq com pid",
        "<30>Aug 15 10:30:45 OpenWrt dnsmasq-dhcp[1834]: DHCPACK(br-lan) 192.168.1.140 aa:bb:cc:dd:ee:02 notebook",
    ),
    // --- Linux / rsyslog ---------------------------------------------------
    (
        "Linux RFC 5424",
        "<165>1 2026-08-15T10:31:02.123456Z servidor sshd 4711 ID47 - Accepted publickey for app from 10.0.0.9 port 55214",
    ),
    (
        "Linux RFC 3164",
        "<38>Aug 15 10:31:40 servidor sshd[4711]: Failed password for invalid user root from 203.0.113.7 port 51022 ssh2",
    ),
    // --- Ubiquiti ----------------------------------------------------------
    (
        "Ubiquiti EdgeOS",
        "<29>Aug 15 10:32:11 EdgeRouter dhcpd: DHCPREQUEST for 192.168.1.55 from aa:bb:cc:dd:ee:03 via eth1",
    ),
    (
        "Ubiquiti UniFi AP",
        "<14>Aug 15 10:32:50 U6-LR hostapd: ath0: STA aa:bb:cc:dd:ee:04 IEEE 802.11: associated",
    ),
    // --- Lixo --------------------------------------------------------------
    // Prova o contrato do parser permissivo: nunca falha, no pior caso a linha
    // inteira vira mensagem.
    ("linha sem formato nenhum", "isto não é syslog de coisa alguma"),
];

/// Escolhe o ano que deixa a data **mais perto** de `referencia`.
///
/// O RFC 3164 não manda o ano. Assumir o ano corrente erra na virada: uma
/// mensagem de 31/dez 23:59 recebida em 01/jan 00:00 iria parar doze meses no
/// futuro, e o log sumiria de qualquer filtro por período. Testar os três anos
/// candidatos e ficar com o mais próximo resolve as duas direções.
///
/// Data inválida no ano candidato (29/fev fora de bissexto) é descartada pelo
/// `single()`; se nenhuma valer, cai no ano da referência.
fn ano_mais_proximo(incompleta: IncompleteDate, referencia: DateTime<Utc>) -> i32 {
    let (mes, dia, hora, minuto, segundo) = incompleta;
    let base = referencia.year();
    [base - 1, base, base + 1]
        .into_iter()
        .filter_map(|ano| {
            Utc.with_ymd_and_hms(ano, mes, dia, hora, minuto, segundo)
                .single()
                .map(|instante| (ano, (instante - referencia).num_seconds().abs()))
        })
        .min_by_key(|(_, distancia)| *distancia)
        .map_or(base, |(ano, _)| ano)
}

/// Os tópicos do RouterOS chegam no lugar do `tag` do BSD.
///
/// A assinatura é `palavra,palavra[,palavra]` — sem espaço, sem `[pid]`. Um
/// `appname` de Linux (`sshd`, `dnsmasq-dhcp`) não tem vírgula, então a regra
/// separa os dois casos sem heurística frouxa.
fn topicos_do_routeros(appname: Option<&str>) -> Option<String> {
    let tag = appname?;
    if !tag.contains(',') {
        return None;
    }
    let valido = tag.split(',').all(|parte| {
        !parte.is_empty()
            && parte
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    });
    valido.then(|| tag.to_owned())
}

/// A severidade verdadeira do RouterOS está nos tópicos, não no `<pri>`.
///
/// O `<pri>` carrega o `syslog-severity` configurado na *action* — que é fixo
/// e, nas versões que não têm `auto`, vale `info` para tudo. Uma falha de login
/// chega como `<134>` (info) com tópicos `system,error,critical`. Sem esta
/// correção, filtrar por severidade não separaria nada num parque MikroTik:
/// todo log seria "info".
///
/// Quando o RouterOS já mandou certo, os dois concordam e a função não muda
/// nada. Vence o mais grave (menor número) entre os tópicos.
fn severidade_do_topico(topics: &str) -> Option<u8> {
    topics
        .split(',')
        .filter_map(|topico| match topico {
            "emergency" => Some(SyslogSeverity::SEV_EMERG as u8),
            "alert" => Some(SyslogSeverity::SEV_ALERT as u8),
            "critical" => Some(SyslogSeverity::SEV_CRIT as u8),
            "error" => Some(SyslogSeverity::SEV_ERR as u8),
            "warning" => Some(SyslogSeverity::SEV_WARNING as u8),
            "info" => Some(SyslogSeverity::SEV_INFO as u8),
            "debug" => Some(SyslogSeverity::SEV_DEBUG as u8),
            _ => None,
        })
        .min()
}

/// Resultado do parse já no formato que a tabela `device_logs` vai gravar.
struct Linha {
    facility: Option<u8>,
    severity: Option<u8>,
    device_time: Option<DateTime<FixedOffset>>,
    hostname: Option<String>,
    app_name: Option<String>,
    pid: Option<i32>,
    topics: Option<String>,
    message: String,
}

/// O parse como ele vai existir em `services/syslog/parser.rs`.
///
/// O `parse_message_with_year` nunca falha: linha que não casa com RFC nenhuma
/// volta com tudo vazio e a entrada inteira em `msg`. É o que se quer — mas
/// custa o `<pri>` do RouterOS cru, que fica preso dentro da mensagem. Daí o
/// resgate: quando o parser não determinou severidade **e** a linha começa com
/// `<n>`, decompomos o pri à mão e reparseamos o resto.
fn parse(bruto: &str, recebido_em: DateTime<Utc>) -> Linha {
    let resolvedor = |incompleta: IncompleteDate| ano_mais_proximo(incompleta, recebido_em);
    let mensagem = syslog_loose::parse_message_with_year(bruto, resolvedor, Variant::Either);

    if mensagem.severity.is_none() {
        if let Some((facility, severity, resto)) = resgata_pri(bruto) {
            let mut linha = converte(syslog_loose::parse_message_with_year(
                resto,
                resolvedor,
                Variant::Either,
            ));
            linha.facility = facility.map(|f| f as u8);
            linha.severity = linha.severity.or_else(|| severity.map(|s| s as u8));
            // Sem timestamp na linha crua, o `msg` do reparse é a linha toda
            // menos o pri: os tópicos ficam no começo dela.
            if linha.topics.is_none() {
                if let Some((tag, resto)) = linha.message.split_once(' ') {
                    if let Some(topicos) = topicos_do_routeros(Some(tag)) {
                        linha.severity = severidade_do_topico(&topicos).or(linha.severity);
                        linha.topics = Some(topicos);
                        linha.message = resto.to_owned();
                    }
                }
            }
            return linha;
        }
    }

    converte(mensagem)
}

/// Extrai `<n>` do início da linha. `191` é o maior pri válido (23 × 8 + 7).
fn resgata_pri(bruto: &str) -> Option<(Option<SyslogFacility>, Option<SyslogSeverity>, &str)> {
    let resto = bruto.strip_prefix('<')?;
    let (numero, resto) = resto.split_once('>')?;
    let pri: u8 = numero.parse().ok()?;
    let (facility, severity) = decompose_pri(pri);
    Some((facility, severity, resto))
}

fn converte(mensagem: Message<&str>) -> Linha {
    let topics = topicos_do_routeros(mensagem.appname);
    let severity = topics
        .as_deref()
        .and_then(severidade_do_topico)
        .or_else(|| mensagem.severity.map(|s| s as u8));
    Linha {
        facility: mensagem.facility.map(|f| f as u8),
        severity,
        device_time: mensagem.timestamp,
        hostname: mensagem.hostname.map(str::to_owned),
        // Tópico do RouterOS não é nome de aplicação: ou é um, ou é outro.
        app_name: if topics.is_some() {
            None
        } else {
            mensagem.appname.map(str::to_owned)
        },
        pid: match mensagem.procid {
            Some(ProcId::PID(pid)) => Some(pid),
            _ => None,
        },
        topics,
        message: mensagem.msg.to_owned(),
    }
}

fn mostra_amostras() {
    let agora = Utc::now();
    println!("== amostras ==\n");
    for (rotulo, bruto) in AMOSTRAS {
        let linha = parse(bruto, agora);
        println!("{rotulo}");
        println!("  bruto     {bruto}");
        println!("  fac/sev   {:?}/{:?}", linha.facility, linha.severity);
        println!(
            "  device_tm {:?}",
            linha.device_time.map(|t| t.to_rfc3339())
        );
        println!("  hostname  {:?}", linha.hostname);
        println!("  app/pid   {:?}/{:?}", linha.app_name, linha.pid);
        println!("  topics    {:?}", linha.topics);
        println!("  message   {}", linha.message);
        println!();
    }

    println!("== resolvedor de ano ==\n");
    let virada = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 30).unwrap();
    // 31/dez 23:59:50 recebido 40 s depois, já em janeiro: tem de virar 2026.
    println!(
        "  31/dez 23:59:50 recebido em {} -> {}",
        virada.to_rfc3339(),
        ano_mais_proximo((12, 31, 23, 59, 50), virada)
    );
    // O contrário: relógio do roteador adiantado, mensagem de 01/jan chegando
    // ainda em dezembro.
    let vespera = Utc.with_ymd_and_hms(2026, 12, 31, 23, 59, 30).unwrap();
    println!(
        "  01/jan 00:00:10 recebido em {} -> {}",
        vespera.to_rfc3339(),
        ano_mais_proximo((1, 1, 0, 0, 10), vespera)
    );
}

/// Mede a inserção em lote no SQLite em WAL, que é o formato exato do escritor.
///
/// 500 linhas × 12 colunas = 6 000 parâmetros — dentro do teto de 32 766 do
/// SQLite ≥ 3.32 e dos 65 535 do PostgreSQL. É a conta que impede o lote de
/// crescer sem pensar.
async fn bench() -> Result<(), Box<dyn std::error::Error>> {
    let arquivo = std::env::temp_dir().join("spike_syslog_bench.sqlite");
    let _ = std::fs::remove_file(&arquivo);
    let url = format!("sqlite://{}?mode=rwc", arquivo.display());
    let db = Database::connect(&url).await?;

    for pragma in [
        "PRAGMA auto_vacuum = INCREMENTAL;",
        "PRAGMA journal_mode = WAL;",
    ] {
        db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite, pragma))
            .await?;
    }

    db.execute_unprepared(
        "CREATE TABLE device_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id INTEGER,
            source_ip TEXT NOT NULL,
            received_at TEXT NOT NULL,
            device_time TEXT,
            facility INTEGER,
            severity INTEGER,
            hostname TEXT,
            app_name TEXT,
            pid INTEGER,
            topics TEXT,
            message TEXT NOT NULL
        );
        CREATE INDEX \"idx-device_logs-device-received\" ON device_logs (device_id, received_at);
        CREATE INDEX \"idx-device_logs-received\" ON device_logs (received_at);
        CREATE INDEX \"idx-device_logs-severity-received\" ON device_logs (severity, received_at);",
    )
    .await?;

    const LOTE: usize = 500;
    const LOTES: usize = 40;
    let agora = Utc::now();
    let modelo = parse(AMOSTRAS[0].1, agora);

    let inicio = Instant::now();
    for _ in 0..LOTES {
        let mut sql = String::from(
            "INSERT INTO device_logs (device_id, source_ip, received_at, device_time, \
             facility, severity, hostname, app_name, pid, topics, message) VALUES ",
        );
        for indice in 0..LOTE {
            if indice > 0 {
                sql.push(',');
            }
            sql.push_str("(?,?,?,?,?,?,?,?,?,?,?)");
        }
        let mut valores: Vec<sea_orm::Value> = Vec::with_capacity(LOTE * 11);
        for _ in 0..LOTE {
            valores.push(1_i64.into());
            valores.push("192.168.88.1".into());
            valores.push(agora.to_rfc3339().into());
            valores.push(modelo.device_time.map(|t| t.to_rfc3339()).into());
            valores.push(modelo.facility.map(i32::from).into());
            valores.push(modelo.severity.map(i32::from).into());
            valores.push(modelo.hostname.clone().into());
            valores.push(modelo.app_name.clone().into());
            valores.push(modelo.pid.into());
            valores.push(modelo.topics.clone().into());
            valores.push(modelo.message.clone().into());
        }
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            &sql,
            valores,
        ))
        .await?;
    }
    let decorrido = inicio.elapsed();

    let linhas = LOTE * LOTES;
    let bytes = std::fs::metadata(&arquivo).map(|m| m.len()).unwrap_or(0);
    println!("== inserção em lote ==\n");
    println!("  {linhas} linhas em {LOTES} lotes de {LOTE}");
    println!("  tempo total     {decorrido:?}");
    println!("  por lote        {:?}", decorrido / LOTES as u32);
    #[allow(clippy::cast_precision_loss)]
    let por_segundo = linhas as f64 / decorrido.as_secs_f64();
    println!("  linhas/s        {por_segundo:.0}");
    #[allow(clippy::cast_precision_loss)]
    let por_linha = bytes as f64 / linhas as f64;
    println!("  arquivo         {bytes} bytes ({por_linha:.0} B/linha, com 3 índices)");
    println!(
        "\n  A 200 msg/s de pico o escritor gasta {:.2}% do tempo.",
        200.0 / por_segundo * 100.0
    );

    let _ = std::fs::remove_file(&arquivo);
    Ok(())
}

/// Critério de aceite da Fase 3: filtro composto em 1 M de linhas.
///
/// A pergunta não é se o SQLite aguenta a tabela, é se a **tela** responde. Por
/// isso mede os três caminhos que a página realmente usa: janela de tempo pura
/// (o caso comum), janela + severidade (o filtro que abre a tela) e janela +
/// `LIKE` (a busca textual, a única que não usa índice em cima da mensagem).
///
/// ```sh
/// cargo run --release --example spike_syslog_parse -- query
/// ```
async fn query() -> Result<(), Box<dyn std::error::Error>> {
    const TOTAL: usize = 1_000_000;
    const LOTE: usize = 1_000;

    let arquivo = std::env::temp_dir().join("spike_syslog_query.sqlite");
    let _ = std::fs::remove_file(&arquivo);
    let url = format!("sqlite://{}?mode=rwc", arquivo.display());
    let db = Database::connect(&url).await?;
    for pragma in [
        "PRAGMA auto_vacuum = INCREMENTAL;",
        "PRAGMA journal_mode = WAL;",
    ] {
        db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite, pragma))
            .await?;
    }
    db.execute_unprepared(
        "CREATE TABLE device_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id INTEGER, source_ip TEXT NOT NULL, received_at TEXT NOT NULL,
            device_time TEXT, facility INTEGER, severity INTEGER, hostname TEXT,
            app_name TEXT, pid INTEGER, topics TEXT, message TEXT NOT NULL
        );
        CREATE INDEX \"device_logs_device_received_index\" ON device_logs (device_id, received_at);
        CREATE INDEX \"device_logs_received_at_index\" ON device_logs (received_at);
        CREATE INDEX \"device_logs_severity_received_index\" ON device_logs (severity, received_at);",
    )
    .await?;

    // O índice de texto da Fase 5. Criado **antes** da semeadura para os
    // gatilhos alimentarem o índice junto com a inserção, como em produção —
    // assim o tempo de semeadura mede também o custo de escrita que ele impõe.
    db.execute_unprepared(
        "CREATE VIRTUAL TABLE device_logs_fts USING fts5(
             message, content='device_logs', content_rowid='id', tokenize='unicode61'
         );
         CREATE TRIGGER device_logs_fts_insert AFTER INSERT ON device_logs BEGIN
             INSERT INTO device_logs_fts(rowid, message) VALUES (new.id, new.message);
         END;",
    )
    .await?;

    // Sete dias de retenção espalhados em 1 M de linhas — a densidade real de
    // ~1,7 msg/s por 30 dispositivos.
    println!("semeando {TOTAL} linhas...");
    let base = Utc::now() - chrono::Duration::days(7);
    let passo_ms = 7 * 24 * 3_600_000 / TOTAL as i64;
    let inicio = Instant::now();
    for bloco in 0..(TOTAL / LOTE) {
        let mut sql = String::from(
            "INSERT INTO device_logs (device_id, source_ip, received_at, severity, topics, message) VALUES ",
        );
        for indice in 0..LOTE {
            if indice > 0 {
                sql.push(',');
            }
            sql.push_str("(?,?,?,?,?,?)");
        }
        let mut valores: Vec<sea_orm::Value> = Vec::with_capacity(LOTE * 6);
        for indice in 0..LOTE {
            let n = bloco * LOTE + indice;
            let instante = base + chrono::Duration::milliseconds(n as i64 * passo_ms);
            valores.push(((n % 30) as i64 + 1).into());
            valores.push(format!("192.168.88.{}", n % 30 + 1).into());
            valores.push(instante.to_rfc3339().into());
            // Distribuição parecida com a real: quase tudo é info, erro é raro.
            valores.push(if n.is_multiple_of(200) { 3_i32 } else { 6_i32 }.into());
            valores.push("system,info".into());
            valores.push(
                format!(
                    "linha {n} interface ether{} status update para o enlace",
                    n % 8
                )
                .into(),
            );
        }
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            &sql,
            valores,
        ))
        .await?;
    }
    println!("  semeadura em {:?}\n", inicio.elapsed());

    let bytes = std::fs::metadata(&arquivo).map(|m| m.len()).unwrap_or(0);
    #[allow(clippy::cast_precision_loss)]
    let mb = bytes as f64 / 1024.0 / 1024.0;
    println!("== consulta em 1 M de linhas ({mb:.0} MB) ==\n");

    let desde = (Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
    let casos: [(&str, String); 5] = [
        (
            "janela de 24 h, 1ª página",
            format!(
                "SELECT * FROM device_logs WHERE received_at >= '{desde}' \
                 ORDER BY received_at DESC, id DESC LIMIT 51"
            ),
        ),
        (
            "janela + dispositivo",
            format!(
                "SELECT * FROM device_logs WHERE received_at >= '{desde}' AND device_id = 7 \
                 ORDER BY received_at DESC, id DESC LIMIT 51"
            ),
        ),
        (
            "janela + severidade (erro e acima)",
            format!(
                "SELECT * FROM device_logs WHERE received_at >= '{desde}' AND severity <= 3 \
                 ORDER BY received_at DESC, id DESC LIMIT 51"
            ),
        ),
        (
            // O pior caso realista: o índice `(severity, received_at)` cobre
            // mal um `<=` largo, porque a faixa na primeira coluna impede usar
            // a segunda para a ordenação. "Erro e acima" pega 0,5% das linhas;
            // "informação e acima" pega quase todas.
            "janela + severidade larga (info e acima)",
            format!(
                "SELECT * FROM device_logs WHERE received_at >= '{desde}' AND severity <= 6 \
                 ORDER BY received_at DESC, id DESC LIMIT 51"
            ),
        ),
        (
            "janela + LIKE (busca textual)",
            format!(
                "SELECT * FROM device_logs WHERE received_at >= '{desde}' \
                 AND message LIKE '%ether3%' ESCAPE '\\' \
                 ORDER BY received_at DESC, id DESC LIMIT 51"
            ),
        ),
    ];

    // O caso que decide a Fase 5: `LIKE` na janela **cheia**, sem o recorte de
    // 24 h que torna a busca barata. Aqui o índice não ajuda em nada e a
    // varredura é a tabela inteira.
    let tudo = (Utc::now() - chrono::Duration::days(8)).to_rfc3339();
    let casos_largos: [(&str, String); 4] = [
        (
            "7 dias + LIKE (com casamento)",
            format!(
                "SELECT * FROM device_logs WHERE received_at >= '{tudo}' \
                 AND message LIKE '%ether3%' ESCAPE '\\' \
                 ORDER BY received_at DESC, id DESC LIMIT 51"
            ),
        ),
        (
            "7 dias + LIKE sem casamento",
            format!(
                "SELECT * FROM device_logs WHERE received_at >= '{tudo}' \
                 AND message LIKE '%termo-que-nao-existe%' ESCAPE '\\' \
                 ORDER BY received_at DESC, id DESC LIMIT 51"
            ),
        ),
        (
            "7 dias + FTS5 (com casamento)",
            format!(
                "SELECT * FROM device_logs WHERE received_at >= '{tudo}' \
                 AND id IN (SELECT rowid FROM device_logs_fts \
                 WHERE device_logs_fts MATCH '\"ether3\"*') \
                 ORDER BY received_at DESC, id DESC LIMIT 51"
            ),
        ),
        (
            "7 dias + FTS5 sem casamento",
            format!(
                "SELECT * FROM device_logs WHERE received_at >= '{tudo}' \
                 AND id IN (SELECT rowid FROM device_logs_fts \
                 WHERE device_logs_fts MATCH '\"termo-que-nao-existe\"*') \
                 ORDER BY received_at DESC, id DESC LIMIT 51"
            ),
        ),
    ];

    for (rotulo, sql) in &casos {
        // Duas medições: a primeira paga o cache frio das páginas do índice.
        let mut melhor = std::time::Duration::MAX;
        for _ in 0..3 {
            let inicio = Instant::now();
            let linhas = db
                .query_all_raw(Statement::from_string(DatabaseBackend::Sqlite, sql.clone()))
                .await?;
            let decorrido = inicio.elapsed();
            melhor = melhor.min(decorrido);
            assert!(!linhas.is_empty(), "{rotulo} não devolveu nada");
        }
        println!("  {rotulo:<38} {melhor:?}");
    }

    println!();
    for (rotulo, sql) in &casos_largos {
        let mut melhor = std::time::Duration::MAX;
        for _ in 0..3 {
            let inicio = Instant::now();
            let _ = db
                .query_all_raw(Statement::from_string(DatabaseBackend::Sqlite, sql.clone()))
                .await?;
            melhor = melhor.min(inicio.elapsed());
        }
        println!("  {rotulo:<38} {melhor:?}");
    }

    println!("\n  Alvo: cada caso abaixo de ~200 ms para a tela não parecer travada.");
    let _ = std::fs::remove_file(&arquivo);
    Ok(())
}

/// Escuta UDP e imprime o IP de origem **como o processo o vê**.
///
/// É a medição que decide se a regra "fonte desconhecida não grava" é viável
/// atrás do Docker. Rodar dentro do container e mandar de outra máquina:
///
/// ```sh
/// logger -n <ip-do-host> -P 514 -d "teste do spike"
/// ```
///
/// Se o IP impresso for o gateway da bridge (172.x.0.1) em vez do IP da
/// máquina de origem, a publicação de porta está mascarando a origem e o
/// arranjo precisa de `network_mode: host`.
async fn listen(porta: u16) -> Result<(), Box<dyn std::error::Error>> {
    let socket = tokio::net::UdpSocket::bind(("0.0.0.0", porta)).await?;
    println!(
        "escutando UDP em {} — Ctrl+C para sair\n",
        socket.local_addr()?
    );
    let mut buffer = vec![0_u8; 8192];
    loop {
        let (tamanho, origem) = socket.recv_from(&mut buffer).await?;
        let bruto = String::from_utf8_lossy(&buffer[..tamanho]);
        let linha = parse(bruto.trim(), Utc::now());
        println!("origem {origem}");
        println!("  sev {:?} topics {:?}", linha.severity, linha.topics);
        println!("  {}\n", linha.message);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    match argumentos.first().map(String::as_str) {
        Some("bench") => bench().await,
        Some("query") => query().await,
        Some("listen") => {
            let porta = argumentos
                .get(1)
                .and_then(|valor| valor.parse().ok())
                .unwrap_or(5514);
            listen(porta).await
        }
        _ => {
            mostra_amostras();
            Ok(())
        }
    }
}
