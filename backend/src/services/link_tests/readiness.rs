//! A conferência antes do teste (ADR 015 §3, ação `check`): o equipamento tem
//! o programa que o papel precisa? Só **lê** — quem instala é o operador, ao
//! aceitar o passo "Instalar o iperf3 em …" no plano (o `prepare` do plugin).
//! O teste nunca instala nada sozinho no meio da medida.
//!
//! O plano confere as pontas em paralelo, com prazo curto; a primeira etapa
//! da perna confere de novo (o estado pode mudar entre o plano e o
//! "Iniciar").

use std::time::Duration;

use futures::future::join_all;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::{
    env::PluginInvoker,
    model::{LinkTestFix, LinkTestRole},
};
use crate::services::{
    network_tools::throughput::ThroughputEngine,
    plugins::{
        invoke::{RunStatus, ServiceRunRequest},
        runs::Origin,
    },
};

/// Prazo de cada conferência no plano: um `command -v` não leva mais que
/// isso, e o plano não pode ficar esperando um equipamento que não responde.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
/// Depois de cancelar por prazo, quanto ainda se espera a execução fechar.
const CANCEL_GRACE: Duration = Duration::from_secs(5);

/// O que o `check` devolve: `{ ready, missing, canInstall, message }`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    pub ready: bool,
    /// Os pacotes que faltam, como o plugin os nomeia (`iperf3`).
    #[serde(default)]
    pub missing: Vec<String>,
    /// O `prepare` do plugin sabe instalar o que falta.
    #[serde(default)]
    pub can_install: bool,
    /// A frase para a tela ("AP-x não tem o iperf3").
    #[serde(default)]
    pub message: String,
}

impl Readiness {
    /// Lê a saída do `check`.
    ///
    /// # Errors
    ///
    /// A saída não segue o contrato (sem `ready`, de outro tipo).
    pub fn from_output(output: Option<&Value>) -> Result<Self, String> {
        output
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .ok_or_else(|| {
                "a conferência não respondeu no formato do contrato (ready, missing, \
                 canInstall, message)"
                    .to_owned()
            })
    }

    /// Por que não está pronto: a frase do plugin ou, sem ela, o que falta.
    #[must_use]
    pub fn reason(&self, device_name: &str, engine: ThroughputEngine) -> String {
        let message = self.message.trim();
        if !message.is_empty() {
            message.to_owned()
        } else if self.missing.is_empty() {
            format!(
                "{device_name} não está pronto para o teste ({})",
                engine.as_str()
            )
        } else {
            format!("{device_name} não tem {}", self.missing.join(", "))
        }
    }
}

/// Os parâmetros fixos do `check` e do `prepare` (ADR 015 §3).
#[must_use]
pub fn params(engine: ThroughputEngine, role: LinkTestRole) -> Value {
    json!({ "engine": engine.as_str(), "role": role.as_str() })
}

/// Uma conferência: qual ação, de qual plugin, em qual equipamento e para
/// qual papel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckTarget {
    pub device_id: i64,
    pub device_name: String,
    pub plugin_id: i64,
    pub plugin_slug: String,
    /// O `throughput.actions.check`.
    pub action: String,
    /// O `throughput.actions.prepare` (quem instala), quando houver.
    pub install_action: Option<String>,
    pub engine: ThroughputEngine,
    pub role: LinkTestRole,
}

impl CheckTarget {
    /// O passo "instalar o pacote" para o que a conferência achou faltando,
    /// quando o plugin sabe instalar. Sem instalação possível, `None`: o
    /// motivo vai por extenso (o Linux, onde o NetMonitor não instala).
    #[must_use]
    pub fn install_fix(&self, readiness: &Readiness) -> Option<LinkTestFix> {
        if readiness.ready || !readiness.can_install {
            return None;
        }
        Some(LinkTestFix::InstallPackage {
            device_id: self.device_id,
            device_name: self.device_name.clone(),
            plugin_id: self.plugin_id,
            plugin_slug: self.plugin_slug.clone(),
            action: self.install_action.clone()?,
            engine: self.engine,
            role: self.role,
            packages: readiness.missing.clone(),
            message: readiness.reason(&self.device_name, self.engine),
        })
    }
}

/// Como terminou uma conferência.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    Ready,
    NotReady(Readiness),
    /// Não deu para conferir (equipamento sem resposta, erro, prazo). Não
    /// impede o plano: a primeira etapa da perna confere de novo.
    Unknown(String),
}

/// Roda o `check` com prazo. Estourado o prazo, cancela a execução (o
/// transcript fica, como o de qualquer execução cancelada).
pub async fn check(
    invoker: &dyn PluginInvoker,
    target: &CheckTarget,
    timeout: Duration,
) -> CheckOutcome {
    let cancel = CancellationToken::new();
    let request = ServiceRunRequest {
        device_id: target.device_id,
        plugin_slug: target.plugin_slug.clone(),
        action: target.action.clone(),
        params: params(target.engine, target.role),
        origin: Origin::LinkTest,
        user_id: None,
        reason: Some(format!(
            "Plano do teste de enlace: conferir o {} ({})",
            target.engine.as_str(),
            target.role.as_str()
        )),
        progress: None,
        cancel: Some(cancel.clone()),
    };
    let run = invoker.execute(request, None);
    tokio::pin!(run);
    let result = tokio::select! {
        result = &mut run => result,
        () = tokio::time::sleep(timeout) => {
            cancel.cancel();
            let _ = tokio::time::timeout(CANCEL_GRACE, run).await;
            return CheckOutcome::Unknown(format!("sem resposta em {} s", timeout.as_secs()));
        }
    };
    match result {
        Err(error) => CheckOutcome::Unknown(error.to_string()),
        Ok(outcome) if outcome.status != RunStatus::Succeeded => CheckOutcome::Unknown(
            outcome
                .error
                .unwrap_or_else(|| "a conferência terminou com falha".to_owned()),
        ),
        Ok(outcome) => match Readiness::from_output(outcome.output.as_ref()) {
            Ok(readiness) if readiness.ready => CheckOutcome::Ready,
            Ok(readiness) => CheckOutcome::NotReady(readiness),
            Err(error) => CheckOutcome::Unknown(error),
        },
    }
}

/// Todas as conferências em paralelo; o resultado vem na ordem dos alvos.
pub async fn check_all(
    invoker: &dyn PluginInvoker,
    targets: &[CheckTarget],
    timeout: Duration,
) -> Vec<CheckOutcome> {
    join_all(targets.iter().map(|target| check(invoker, target, timeout))).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(install_action: Option<&str>) -> CheckTarget {
        CheckTarget {
            device_id: 3,
            device_name: "AP-x".into(),
            plugin_id: 7,
            plugin_slug: "openwrt-link-test".into(),
            action: "tp_check".into(),
            install_action: install_action.map(str::to_owned),
            engine: ThroughputEngine::Iperf3,
            role: LinkTestRole::Serve,
        }
    }

    #[test]
    fn le_a_resposta_do_check_com_tolerancia() {
        let lida = Readiness::from_output(Some(&json!({
            "ready": false, "missing": ["iperf3"], "canInstall": true,
            "message": "AP-x não tem o iperf3"
        })))
        .unwrap();
        assert!(!lida.ready);
        assert_eq!(lida.missing, ["iperf3"]);
        assert!(lida.can_install);
        // Só `ready` é obrigatório.
        let curta = Readiness::from_output(Some(&json!({ "ready": true }))).unwrap();
        assert!(curta.ready && curta.missing.is_empty() && !curta.can_install);
        assert!(Readiness::from_output(None).is_err());
        assert!(Readiness::from_output(Some(&json!({ "ok": true }))).is_err());
    }

    #[test]
    fn motivo_vem_do_plugin_ou_do_que_falta() {
        let com_frase = Readiness {
            ready: false,
            missing: vec!["iperf3".into()],
            can_install: true,
            message: "  AP-x não tem o iperf3 ".into(),
        };
        assert_eq!(
            com_frase.reason("AP-x", ThroughputEngine::Iperf3),
            "AP-x não tem o iperf3"
        );
        let sem_frase = Readiness {
            message: String::new(),
            ..com_frase.clone()
        };
        assert_eq!(
            sem_frase.reason("AP-x", ThroughputEngine::Iperf3),
            "AP-x não tem iperf3"
        );
        let nada = Readiness {
            missing: Vec::new(),
            ..sem_frase
        };
        assert_eq!(
            nada.reason("AP-x", ThroughputEngine::Btest),
            "AP-x não está pronto para o teste (btest)"
        );
    }

    #[test]
    fn passo_de_instalar_so_quando_o_plugin_instala() {
        let falta = Readiness {
            ready: false,
            missing: vec!["iperf3".into()],
            can_install: true,
            message: "AP-x não tem o iperf3".into(),
        };
        let fix = target(Some("tp_prepare")).install_fix(&falta).unwrap();
        assert!(matches!(
            &fix,
            LinkTestFix::InstallPackage { action, packages, role: LinkTestRole::Serve, .. }
                if action == "tp_prepare" && packages == &["iperf3".to_owned()]
        ));
        assert_eq!(fix.message(), "AP-x não tem o iperf3");
        // Sem `prepare`, ou o plugin diz que não instala (Linux): sem passo.
        assert_eq!(target(None).install_fix(&falta), None);
        let linux = Readiness {
            can_install: false,
            ..falta.clone()
        };
        assert_eq!(target(Some("tp_prepare")).install_fix(&linux), None);
        let pronto = Readiness {
            ready: true,
            ..falta
        };
        assert_eq!(target(Some("tp_prepare")).install_fix(&pronto), None);
    }

    #[test]
    fn parametros_do_contrato() {
        assert_eq!(
            params(ThroughputEngine::Btest, LinkTestRole::Drive),
            json!({ "engine": "btest", "role": "drive" })
        );
    }
}
