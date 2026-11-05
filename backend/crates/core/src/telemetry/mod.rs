//! Histórico de métricas por host Docker (agente): host e containers, em
//! baldes de 1 minuto.
//!
//! O rollup chega inteiro pelo canal do agente (`AgentEvent::MetricsRollup`)
//! e é gravado de uma vez; a limpeza segue a retenção do núcleo. Por isso a
//! persistência mora aqui, e não na área Servidores: métricas de host são do
//! núcleo (servidor é dispositivo), e a parte de containers viaja no mesmo
//! rollup.

pub mod store;
pub mod views;
