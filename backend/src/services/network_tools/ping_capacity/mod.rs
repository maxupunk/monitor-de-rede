//! Diagnóstico rápido por ping (ADR 015 §9).
//!
//! Ecos ICMP pequenos (56 B) e grandes (1472 B) intercalados mostram latência
//! (mín/p50/p95), jitter, perda e sinais de fila do **caminho todo** da central
//! até o equipamento. Da diferença dos mínimos sai, quando o sinal passa do
//! ruído, uma **ordem de grandeza** da capacidade — nunca uma medida de banda:
//! para isso existe o teste de velocidade (`link_tests`).
//!
//! - [`estimator`]: regra pura (amostras → relatório), com os limiares do spike.
//! - [`runner`]: a coleta, com um pinger só sobre o socket do processo.
//! - [`service`]: alvo, trava por endereço e orquestração.

pub mod estimator;
pub mod model;
pub mod runner;
pub mod service;
