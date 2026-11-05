//! Containers que a interface não derruba (ADR 014).
//!
//! Dois casos, decididos **no host** — pelo agente, antes de qualquer efeito:
//!
//! * o container que executa o próprio processo ([`is_current_container`]):
//!   parar o agente cortaria a central do host sem volta;
//! * os marcados com [`PROTECTED_LABEL`]`=true` pela instalação: a central, o
//!   agente local e o que mais o `docker-compose.yml` marcar. Com o Docker da
//!   central atendido por um agente ao lado dela, um "parar" no container da
//!   API derrubaria a própria tela que pediu.
//!
//! As funções são puras: as mesmas regras servem ao agente (que recusa) e à
//! central (que marca o container na tela e não oferece o botão).

use std::collections::HashMap;

use serde_json::Value;

/// Label que protege um container das ações da interface.
pub const PROTECTED_LABEL: &str = "com.netmonitor.protected";

/// O valor da label liga a proteção: `true`, `1` ou `yes`.
#[must_use]
pub fn is_protected(labels: &HashMap<String, String>) -> bool {
    labels.get(PROTECTED_LABEL).is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes"
        )
    })
}

/// Mesma regra sobre o JSON do `inspect` (`Config.Labels`).
#[must_use]
pub fn is_protected_inspect(inspected: &Value) -> bool {
    inspected
        .pointer("/Config/Labels")
        .and_then(Value::as_object)
        .map(|labels| {
            labels
                .iter()
                .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_string())))
                .collect::<HashMap<_, _>>()
        })
        .is_some_and(|labels| is_protected(&labels))
}

/// Container que executa este processo — o Docker usa o id curto como hostname.
#[must_use]
pub fn is_current_container(id: &str) -> bool {
    std::env::var("HOSTNAME")
        .ok()
        .map(|hostname| hostname.trim().to_ascii_lowercase())
        .filter(|hostname| hostname.len() >= 12)
        .is_some_and(|hostname| id.to_ascii_lowercase().starts_with(&hostname))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_verdadeira_protege_e_o_resto_nao() {
        let labels =
            |value: &str| HashMap::from([(PROTECTED_LABEL.to_string(), value.to_string())]);
        assert!(is_protected(&labels("true")));
        assert!(is_protected(&labels(" TRUE ")));
        assert!(is_protected(&labels("1")));
        assert!(!is_protected(&labels("false")));
        assert!(!is_protected(&HashMap::new()));
    }

    #[test]
    fn inspect_le_as_labels_da_configuracao() {
        let protegido = serde_json::json!({ "Config": { "Labels": { PROTECTED_LABEL: "true" } } });
        assert!(is_protected_inspect(&protegido));
        assert!(!is_protected_inspect(
            &serde_json::json!({ "Config": { "Labels": null } })
        ));
        assert!(!is_protected_inspect(&serde_json::json!({})));
    }
}
