//! O aviso que atravessa os processos da central (ADR 024).
//!
//! No PostgreSQL cada processo publica no canal [`CHANNEL`] com `pg_notify` e
//! escuta o mesmo canal. O aviso diz **o que** chegou — um evento, um snapshot,
//! o esquecimento de um snapshot, ou o pedido de reenvio dos snapshots — e
//! **de quem** (`origin`), para o emissor não reprocessar o próprio aviso.
//!
//! O `NOTIFY` aceita menos de 8000 bytes. O que cabe vai inteiro
//! ([`MAX_INLINE_BYTES`]); o que não cabe vai pela linha do `event_outbox`, e o
//! aviso leva só o `id` dela. Tudo aqui é puro: codificar, decodificar e
//! decidir entre levar o evento ou só a linha.

use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use super::DomainEvent;

/// O canal do `LISTEN`/`NOTIFY`. É por banco: duas instalações no mesmo
/// servidor PostgreSQL não se ouvem.
pub const CHANNEL: &str = "netmonitor_events";

/// Teto do aviso inteiro, com folga sobre o limite de 8000 bytes do
/// PostgreSQL (`NAMEDATALEN` e o cabeçalho da fila não entram na conta dele,
/// mas a folga evita depender disso).
pub const MAX_INLINE_BYTES: usize = 7_900;

/// O que um processo avisa aos outros.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Notice {
    /// Um evento: de domínio (sempre com `id`, que é a linha do outbox) ou
    /// efêmero (sem linha quando cabe no aviso).
    Event {
        origin: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        event: Option<DomainEvent>,
    },
    /// O estado atual de um recurso, para o cache de snapshots de quem recebe.
    Snapshot {
        origin: String,
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        event: Option<DomainEvent>,
    },
    /// O emissor esqueceu os snapshots desta chave (host desconectado, por
    /// exemplo): quem recebe esquece os que vieram dele.
    Forget { origin: String, key: String },
    /// Um processo acabou de (re)começar a escutar: os outros reenviam os
    /// snapshots que eles mesmos produziram.
    Resync { origin: String },
}

impl Notice {
    #[must_use]
    pub fn origin(&self) -> &str {
        match self {
            Self::Event { origin, .. }
            | Self::Snapshot { origin, .. }
            | Self::Forget { origin, .. }
            | Self::Resync { origin } => origin,
        }
    }

    /// O texto do `pg_notify`, se couber.
    #[must_use]
    pub fn encode(&self) -> Option<String> {
        serde_json::to_string(self)
            .ok()
            .filter(|text| text.len() <= MAX_INLINE_BYTES)
    }

    /// Lê o texto recebido. Aviso ilegível é descartado por quem chama.
    ///
    /// # Errors
    ///
    /// JSON fora do contrato.
    pub fn decode(text: &str) -> serde_json::Result<Self> {
        serde_json::from_str(text)
    }

    /// O mesmo aviso sem o evento, apontando a linha `id` do outbox — o que
    /// vai no `NOTIFY` quando o evento inteiro não cabe. `Forget` e `Resync`
    /// não carregam evento e ficam como estão.
    #[must_use]
    pub fn by_row(self, row_id: i64) -> Self {
        match self {
            Self::Event { origin, .. } => Self::Event {
                origin,
                id: Some(row_id),
                event: None,
            },
            Self::Snapshot { origin, key, .. } => Self::Snapshot {
                origin,
                key,
                id: Some(row_id),
                event: None,
            },
            other => other,
        }
    }
}

/// Os `id` de outbox já entregues neste processo, com memória limitada.
///
/// Depois de uma reconexão o processo relê as linhas recentes; as que já
/// tinham chegado pelo aviso não podem ir duas vezes à tela.
#[derive(Debug)]
pub struct RecentIds {
    capacity: usize,
    order: VecDeque<i64>,
    known: HashSet<i64>,
}

impl RecentIds {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            order: VecDeque::with_capacity(capacity),
            known: HashSet::with_capacity(capacity),
        }
    }

    /// Registra o `id`; `false` se ele já tinha sido entregue.
    pub fn insert(&mut self, id: i64) -> bool {
        if !self.known.insert(id) {
            return false;
        }
        self.order.push_back(id);
        if self.order.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.known.remove(&oldest);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(size: usize) -> DomainEvent {
        DomainEvent {
            event_type: "vpn:peers".into(),
            payload: serde_json::json!({ "blob": "x".repeat(size) }),
            occurred_at: "2026-11-25T00:00:00Z".into(),
        }
    }

    #[test]
    fn aviso_pequeno_vai_inteiro_e_volta_igual() {
        let notice = Notice::Snapshot {
            origin: "a".into(),
            key: "vpn:1".into(),
            id: None,
            event: Some(event(10)),
        };
        let text = notice.encode().expect("cabe no NOTIFY");
        assert!(text.contains("\"kind\":\"snapshot\""));
        assert!(!text.contains("\"id\""), "campo ausente não vai no aviso");
        assert_eq!(Notice::decode(&text).unwrap(), notice);
    }

    #[test]
    fn aviso_grande_nao_cabe_e_vai_pela_linha() {
        let notice = Notice::Event {
            origin: "a".into(),
            id: Some(9),
            event: Some(event(MAX_INLINE_BYTES)),
        };
        assert!(notice.encode().is_none());
        let by_row = notice.by_row(9);
        let text = by_row.encode().expect("só o id cabe");
        assert_eq!(
            Notice::decode(&text).unwrap(),
            Notice::Event {
                origin: "a".into(),
                id: Some(9),
                event: None
            }
        );
    }

    #[test]
    fn snapshot_pela_linha_mantem_a_chave() {
        let notice = Notice::Snapshot {
            origin: "a".into(),
            key: "agent-3".into(),
            id: None,
            event: Some(event(1)),
        }
        .by_row(4);
        assert_eq!(
            notice,
            Notice::Snapshot {
                origin: "a".into(),
                key: "agent-3".into(),
                id: Some(4),
                event: None
            }
        );
        let forget = Notice::Forget {
            origin: "a".into(),
            key: "k".into(),
        };
        assert_eq!(forget.clone().by_row(1), forget);
    }

    #[test]
    fn aviso_fora_do_contrato_e_recusado() {
        assert!(Notice::decode("{\"kind\":\"outro\"}").is_err());
        assert!(Notice::decode("lixo").is_err());
        assert_eq!(
            Notice::decode("{\"kind\":\"resync\",\"origin\":\"b\"}")
                .unwrap()
                .origin(),
            "b"
        );
    }

    #[test]
    fn ids_recentes_nao_repetem_e_esquecem_os_mais_antigos() {
        let mut recent = RecentIds::with_capacity(2);
        assert!(recent.insert(1));
        assert!(!recent.insert(1));
        assert!(recent.insert(2));
        assert!(recent.insert(3));
        // O 1 saiu da janela: voltaria a ser aceito.
        assert!(recent.insert(1));
        assert!(!recent.insert(3));
    }
}
