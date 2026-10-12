//! Quanto uma chamada de ferramenta pode estragar — e, com isso, se o modo
//! automático pode rodá-la sem perguntar.
//!
//! Cada ferramenta avalia a própria chamada ([`AiToolHandler::risk`]): é ela
//! quem sabe que parar um container é pior que iniciá-lo, ou que silenciar
//! por um dia é pior que por dez minutos. O padrão vem do [`ToolKind`].
//!
//! | risco | o que é | no modo automático |
//! |---|---|---|
//! | baixo | não muda nada ou só acrescenta (ping, reconhecer alerta, criar monitor) | roda |
//! | médio | muda algo reversível (reiniciar container, silenciar por pouco tempo) | roda — salvo se o Decisor achar perigoso no contexto |
//! | alto | pode derrubar um serviço, apagar dado ou esconder um problema real | sempre pergunta |
//!
//! [`AiToolHandler::risk`]: super::AiToolHandler::risk

use serde::Serialize;
use ts_rs::TS;

use super::ToolKind;

/// Nível de risco de uma chamada, do menor para o maior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiToolRisk {
    Low,
    Medium,
    High,
}

impl AiToolRisk {
    /// Em inglês, para o estado que o Decisor lê.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// O risco de uma chamada e o motivo, na língua de quem vai ler a tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskAssessment {
    pub level: AiToolRisk,
    pub reason: String,
}

impl RiskAssessment {
    #[must_use]
    pub fn low(reason: impl Into<String>) -> Self {
        Self {
            level: AiToolRisk::Low,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn medium(reason: impl Into<String>) -> Self {
        Self {
            level: AiToolRisk::Medium,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn high(reason: impl Into<String>) -> Self {
        Self {
            level: AiToolRisk::High,
            reason: reason.into(),
        }
    }

    /// O que vale quando a ferramenta não diz nada mais específico.
    #[must_use]
    pub fn for_kind(kind: ToolKind) -> Self {
        match kind {
            ToolKind::Passive | ToolKind::Interactive => Self::low("Só consulta"),
            ToolKind::Active => Self::low("Teste de leitura na rede; não muda nada"),
            ToolKind::Action => Self::medium("Muda o estado do sistema"),
            ToolKind::ContainerAction => Self::medium("Muda o estado de um container"),
            ToolKind::DeviceAccess => Self::high("Acessa um equipamento real"),
        }
    }

    /// Sobe para alto com o motivo dado (o Decisor viu perigo no contexto).
    #[must_use]
    pub fn escalated(reason: impl Into<String>) -> Self {
        Self::high(reason)
    }

    #[must_use]
    pub fn is_high(&self) -> bool {
        self.level == AiToolRisk::High
    }
}

/// O que o laço do agente faz com a chamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    /// Roda agora; aparece no chat e na auditoria.
    Run,
    /// Volta para o usuário como pedido de confirmação.
    Ask,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn niveis_ordenam_do_menor_para_o_maior() {
        assert!(AiToolRisk::Low < AiToolRisk::Medium);
        assert!(AiToolRisk::Medium < AiToolRisk::High);
    }

    #[test]
    fn padrao_por_tipo_so_e_alto_no_equipamento() {
        assert_eq!(
            RiskAssessment::for_kind(ToolKind::Active).level,
            AiToolRisk::Low
        );
        assert_eq!(
            RiskAssessment::for_kind(ToolKind::Action).level,
            AiToolRisk::Medium
        );
        assert!(RiskAssessment::for_kind(ToolKind::DeviceAccess).is_high());
        assert!(RiskAssessment::escalated("x").is_high());
    }

    #[test]
    fn serializa_em_minusculas() {
        assert_eq!(
            serde_json::to_value(AiToolRisk::Medium).unwrap(),
            serde_json::json!("medium")
        );
    }
}
