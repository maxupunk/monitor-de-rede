//! Ponto de montagem da central (ADR 017): os `Hooks` do Loco (`app`), os
//! registros que as áreas preenchem (`registries`), os initializers do
//! servidor, as tarefas da CLI e a SPA. O domínio mora nos crates do
//! workspace — núcleo em `netmonitor-nucleo` (e `netmonitor-core`), áreas em
//! `netmonitor-{network,servers,vpn}` e o assistente em `netmonitor-assistant`.

pub mod app;
pub mod initializers;
pub mod registries;
pub mod spa;
pub mod tasks;
