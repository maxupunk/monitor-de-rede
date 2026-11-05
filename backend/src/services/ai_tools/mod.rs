//! Ferramentas da IA contribuídas pelo núcleo (ADR 017, Fase 8).
//!
//! Cada ferramenta é um [`AiToolHandler`] — o contrato mora no núcleo
//! (`netmonitor_core::ai::tools`) e o catálogo, na ordem em que a IA as vê, é
//! montado no ponto de montagem (`crate::registries::ai_tool_catalog`).
//! Ferramenta nova é um tipo novo listado lá — o loop do agente, o controller
//! e as demais ferramentas não mudam. As das áreas vêm de cada área: Docker
//! (`netmonitor_servers::ai_tools`), plugins de dispositivo
//! (`services::plugins::ai_tools`), topologia (`services::topology::ai_tools`)
//! e a pesquisa na web, do próprio assistente.
//!
//! - [`inventory`]: estado atual (dispositivos, interfaces, monitores, alertas).
//! - [`history`]: histórico gravado no banco (uptime, falhas, métricas).
//! - [`logs`]: syslog e logs da aplicação, agrupados por padrão.
//! - [`grep`]: busca em logs, alertas e checagens devolvendo só o necessário.
//! - [`analysis`]: causa raiz, baseline, padrão por hora e linha do tempo.
//! - [`charts`]: séries do banco desenhadas no chat com os gráficos das telas.
//! - [`diagnostics`]: testes ativos de rede (ping, traceroute, portas, DNS).
//! - [`actions`]: ações que mudam o sistema — sempre com confirmação do usuário.
//! - [`alert_rules`]: origem de um alerta, regras cadastradas, guia e — com
//!   confirmação — criar ou excluir regra.
//! - [`platform`]: o próprio NetMonitor — agentes, VPN, descoberta, redes,
//!   manutenção e notificações (VPN e descoberta são seções das áreas).
//! - [`audit`]: a trilha de auditoria — quem mudou o quê e quando, com o
//!   antes/depois, para achar a alteração que precedeu um problema.
//! - [`schedules`]: tarefas agendadas — rodar uma destas ferramentas num
//!   horário (cron), listar, mudar, pausar e excluir. O catálogo do que pode
//!   ser agendado é o [`UnattendedTools`].
//! - [`dashboard`]: o layout do dashboard — ler e, com confirmação a cada
//!   chamada, mudar os cards.
//! - [`ask`]: a IA pergunta ao usuário antes de prosseguir.
//! - [`risk`]: o risco de cada chamada, que decide se o modo automático roda
//!   ou pergunta.

pub mod access;
pub mod actions;
pub mod alert_rules;
pub mod analysis;
pub mod ask;
pub mod audit;
pub mod charts;
pub mod dashboard;
pub mod diagnostics;
pub mod grep;
pub mod history;
pub mod inventory;
pub mod log_digest;
pub mod logs;
pub mod platform;
pub mod schedules;
pub mod timeline;

/// O contrato e as utilidades das ferramentas moram no núcleo.
pub use netmonitor_core::ai::{
    args::ToolArgs,
    lookup,
    risk::{self, AiToolRisk, Approval, RiskAssessment},
    series,
    tools::{
        as_output, preview_or_error, AiToolHandler, ToolGroup, ToolKind, ToolOutput,
        UnattendedTools,
    },
};
