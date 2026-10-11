//! Pedidos de aprovação da IA esperando o usuário.
//!
//! Quando o agente pergunta antes de agir, o card fica na conversa — mas o
//! usuário pode ter saído da tela ou fechado o navegador. A pendência fica
//! registrada aqui para:
//!
//! - **aparecer em qualquer tela**: vai como snapshot do SSE global
//!   ([`PENDING_EVENT`]); uma aba aberta depois ainda a recebe, sem polling
//!   (AGENTS §9);
//! - **avisar quem não está olhando**: offline (sem stream SSE aberto), o
//!   Web Push sai na hora; online, sai se ninguém responder em
//!   [`REMINDER_AFTER`];
//! - **não rodar duas vezes**: decidida numa aba, a mesma proposta é recusada
//!   na outra.
//!
//! Vive em memória, no processo da API. Um reinício esquece as pendências: o
//! card da conversa continua confirmável, só sem lembrete.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use chrono::Utc;
use loco_rs::app::AppContext;
use serde::Serialize;
use serde_json::{json, Value};
use ts_rs::TS;

use crate::services::{
    ai::harness::tools::{AiToolRisk, RiskAssessment},
    events::{EventBus, Presence},
    shared::errors::{AppError, AppResult},
    webpush::{deliver, Audience},
};

pub const PENDING_EVENT: &str = "ai:approval_pending";
pub const RESOLVED_EVENT: &str = "ai:approval_resolved";

/// Quanto o usuário online tem para responder antes do lembrete no celular.
pub const REMINDER_AFTER: Duration = Duration::from_secs(2 * 60);
/// Depois disso a pendência some sozinha (o card da conversa continua lá).
const EXPIRES_AFTER: Duration = Duration::from_secs(24 * 60 * 60);
/// Decididas lembradas, para recusar a mesma proposta numa segunda aba.
const SETTLED_MEMORY: usize = 512;

/// O pedido como a tela o mostra. Os argumentos ficam só no servidor: quem
/// confirma por fora do chat executa a proposta guardada, não uma cópia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiPendingApproval {
    /// Id da chamada da IA — o mesmo do card na conversa.
    pub call_id: String,
    #[ts(type = "number")]
    pub user_id: i64,
    pub tool: String,
    pub summary: String,
    pub risk: AiToolRisk,
    /// Por que pergunta ("Exclui a regra e o histórico de alertas dela…").
    pub reason: String,
    pub created_at: String,
}

/// A proposta guardada, para executar quando confirmada por fora do chat.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    pub approval: AiPendingApproval,
    pub arguments: Value,
    pub conversation_key: Option<String>,
}

/// Como o usuário decidiu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Confirmed,
    Dismissed,
}

impl Decision {
    const fn id(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Dismissed => "dismissed",
        }
    }
}

/// O que havia para decidir.
#[derive(Debug, Clone, PartialEq)]
pub enum Settled {
    /// Estava pendente e agora não está mais.
    Pending(Box<Proposal>),
    /// Já foi decidida antes (outra aba, outro clique): não rode de novo.
    AlreadyDecided,
    /// Desconhecida (processo reiniciou, expirou, outro usuário): o card da
    /// conversa segue valendo do jeito de sempre.
    Unknown,
}

struct Entry {
    proposal: Proposal,
    opened: Instant,
}

#[derive(Default)]
struct State {
    pending: HashMap<String, Entry>,
    settled: VecDeque<String>,
}

impl State {
    fn sweep(&mut self, now: Instant) -> Vec<String> {
        let expired: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.opened) >= EXPIRES_AFTER)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            self.pending.remove(id);
        }
        expired
    }

    fn remember(&mut self, call_id: String) {
        if self.settled.len() >= SETTLED_MEMORY {
            self.settled.pop_front();
        }
        self.settled.push_back(call_id);
    }

    fn settle(&mut self, user_id: i64, call_id: &str) -> Settled {
        if self.settled.iter().any(|id| id == call_id) {
            return Settled::AlreadyDecided;
        }
        match self.pending.get(call_id) {
            Some(entry) if entry.proposal.approval.user_id == user_id => {
                let entry = self.pending.remove(call_id).map(|entry| entry.proposal);
                self.remember(call_id.to_string());
                entry.map_or(Settled::Unknown, |proposal| {
                    Settled::Pending(Box::new(proposal))
                })
            }
            _ => Settled::Unknown,
        }
    }
}

/// O balcão de pendências do processo.
#[derive(Clone, Default)]
pub struct ApprovalDesk {
    state: Arc<Mutex<State>>,
}

impl ApprovalDesk {
    pub fn install(ctx: &AppContext) {
        ctx.shared_store.insert(Self::default());
    }

    /// A instância do processo; sem ela instalada, uma vazia (nada a lembrar).
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Self {
        ctx.shared_store.get::<Self>().unwrap_or_default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[must_use]
    pub fn is_pending(&self, call_id: &str) -> bool {
        self.lock().pending.contains_key(call_id)
    }

    fn insert(&self, proposal: Proposal) -> Vec<String> {
        let mut state = self.lock();
        let expired = state.sweep(Instant::now());
        // Provedor que reutiliza ids (`call_0` em toda resposta): a proposta
        // nova substitui a decisão antiga, em vez de nascer "já decidida".
        let call_id = &proposal.approval.call_id;
        state.settled.retain(|id| id != call_id);
        state.pending.insert(
            proposal.approval.call_id.clone(),
            Entry {
                proposal,
                opened: Instant::now(),
            },
        );
        expired
    }

    fn settle(&self, user_id: i64, call_id: &str) -> Settled {
        self.lock().settle(user_id, call_id)
    }
}

/// O que o agente sabe do pedido ao perguntar.
pub struct ApprovalRequest<'a> {
    pub call_id: &'a str,
    pub user_id: i64,
    pub tool: &'a str,
    pub arguments: Value,
    pub summary: &'a str,
    pub risk: &'a RiskAssessment,
    pub conversation_key: Option<String>,
}

/// Registra o pedido, mostra-o em todas as telas e agenda o aviso.
pub fn open(ctx: &AppContext, request: ApprovalRequest<'_>) {
    let approval = AiPendingApproval {
        call_id: request.call_id.to_string(),
        user_id: request.user_id,
        tool: request.tool.to_string(),
        summary: request.summary.to_string(),
        risk: request.risk.level,
        reason: request.risk.reason.clone(),
        created_at: Utc::now().to_rfc3339(),
    };
    let desk = ApprovalDesk::from_context(ctx);
    let expired = desk.insert(Proposal {
        approval: approval.clone(),
        arguments: request.arguments,
        conversation_key: request.conversation_key,
    });
    if let Ok(bus) = EventBus::from_context(ctx) {
        for call_id in expired {
            bus.forget_snapshots(&call_id);
        }
        bus.publish_snapshot(
            PENDING_EVENT,
            approval.call_id.clone(),
            serde_json::to_value(&approval).unwrap_or_default(),
        );
    }
    tokio::spawn(remind(ctx.clone(), desk, approval));
}

/// Avisa por Web Push quem não está olhando: na hora, se está offline; depois
/// de [`REMINDER_AFTER`], se está online e ainda não respondeu.
async fn remind(ctx: AppContext, desk: ApprovalDesk, approval: AiPendingApproval) {
    if Presence::from_context(&ctx).is_online(approval.user_id) {
        tokio::time::sleep(REMINDER_AFTER).await;
    }
    if !desk.is_pending(&approval.call_id) {
        return;
    }
    let payload = notification(&approval);
    match deliver(&ctx.db, Audience::User(approval.user_id), &payload).await {
        Ok(delivery) if delivery.sent > 0 => {
            tracing::info!(call_id = %approval.call_id, user_id = approval.user_id, "pedido de aprovação da IA enviado por Web Push");
        }
        Ok(_) => tracing::debug!(
            user_id = approval.user_id,
            "pedido de aprovação da IA sem dispositivo inscrito para Web Push"
        ),
        Err(error) => tracing::warn!(%error, "falha ao avisar o pedido de aprovação da IA"),
    }
}

/// O Web Push do pedido. A notificação abre o chat; o pedido aparece em
/// qualquer tela pelo snapshot do SSE.
fn notification(approval: &AiPendingApproval) -> Value {
    let risk = match approval.risk {
        AiToolRisk::High => "Risco alto",
        AiToolRisk::Medium => "Risco médio",
        AiToolRisk::Low => "Risco baixo",
    };
    json!({
        "title": "Laya aguarda sua aprovação",
        "body": format!("{}\n{risk}: {}", approval.summary, approval.reason),
        "icon": "/pwa-192x192.png",
        "badge": "/pwa-192x192.png",
        "tag": format!("ai-approval-{}", approval.call_id),
        "data": { "url": "/ai-chat", "callId": approval.call_id }
    })
}

/// Registra a decisão do usuário. A proposta decidida sai de todas as telas.
#[must_use]
pub fn settle(ctx: &AppContext, user_id: i64, call_id: &str, decision: Decision) -> Settled {
    let settled = ApprovalDesk::from_context(ctx).settle(user_id, call_id);
    if matches!(settled, Settled::Pending(_)) {
        if let Ok(bus) = EventBus::from_context(ctx) {
            bus.forget_snapshots(call_id);
            bus.publish_ephemeral(
                RESOLVED_EVENT,
                json!({ "callId": call_id, "userId": user_id, "decision": decision.id() }),
            );
        }
    }
    settled
}

/// Recusa a proposta já decidida em outra aba.
///
/// # Errors
///
/// [`Settled::AlreadyDecided`].
pub fn ensure_undecided(settled: &Settled) -> AppResult<()> {
    if *settled == Settled::AlreadyDecided {
        return Err(already_decided());
    }
    Ok(())
}

/// A proposta já foi decidida em outra tela (ou outro clique).
#[must_use]
pub fn already_decided() -> AppError {
    AppError::conflict("Esta ação já foi decidida em outra tela.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposal(call_id: &str, user_id: i64) -> Proposal {
        Proposal {
            approval: AiPendingApproval {
                call_id: call_id.into(),
                user_id,
                tool: "docker_container_action".into(),
                summary: "Parar o container web (central)".into(),
                risk: AiToolRisk::High,
                reason: "O serviço fica fora".into(),
                created_at: String::new(),
            },
            arguments: json!({ "action": "stop", "container": "web" }),
            conversation_key: None,
        }
    }

    #[test]
    fn decidida_uma_vez_e_recusada_na_segunda() {
        let desk = ApprovalDesk::default();
        desk.insert(proposal("c1", 7));
        assert!(desk.is_pending("c1"));

        assert!(matches!(desk.settle(7, "c1"), Settled::Pending(_)));
        assert!(!desk.is_pending("c1"));
        assert_eq!(desk.settle(7, "c1"), Settled::AlreadyDecided);
        assert!(ensure_undecided(&Settled::AlreadyDecided).is_err());
        assert!(ensure_undecided(&Settled::Unknown).is_ok());
    }

    #[test]
    fn id_reutilizado_pelo_provedor_vale_como_proposta_nova() {
        let desk = ApprovalDesk::default();
        desk.insert(proposal("call_0", 7));
        assert!(matches!(desk.settle(7, "call_0"), Settled::Pending(_)));

        desk.insert(proposal("call_0", 7));
        assert!(
            matches!(desk.settle(7, "call_0"), Settled::Pending(_)),
            "a proposta nova não nasce decidida"
        );
    }

    #[test]
    fn so_o_dono_decide() {
        let desk = ApprovalDesk::default();
        desk.insert(proposal("c2", 7));
        assert_eq!(desk.settle(8, "c2"), Settled::Unknown);
        assert!(desk.is_pending("c2"), "outro usuário não tira da fila");
        assert_eq!(desk.settle(7, "desconhecida"), Settled::Unknown);
    }

    #[test]
    fn pendencia_velha_expira() {
        let mut state = State::default();
        state.pending.insert(
            "velha".into(),
            Entry {
                proposal: proposal("velha", 1),
                opened: Instant::now(),
            },
        );
        assert!(state.sweep(Instant::now()).is_empty());
        assert_eq!(
            state.sweep(Instant::now() + EXPIRES_AFTER),
            vec!["velha".to_string()]
        );
        assert!(state.pending.is_empty());
    }

    #[test]
    fn memoria_de_decididas_e_limitada() {
        let mut state = State::default();
        for index in 0..=SETTLED_MEMORY {
            state.remember(index.to_string());
        }
        assert_eq!(state.settled.len(), SETTLED_MEMORY);
        assert_eq!(state.settled.front().map(String::as_str), Some("1"));
    }

    #[test]
    fn notificacao_abre_o_chat_e_diz_o_risco() {
        let payload = notification(&proposal("c3", 1).approval);
        assert_eq!(payload["title"], "Laya aguarda sua aprovação");
        assert_eq!(payload["data"]["url"], "/ai-chat");
        assert_eq!(payload["tag"], "ai-approval-c3");
        assert!(payload["body"]
            .as_str()
            .unwrap()
            .contains("Risco alto: O serviço fica fora"));
    }
}
