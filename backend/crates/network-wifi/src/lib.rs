//! Submódulo Wi-Fi da área Rede (ADR 017, Fase 14): monitoramento de sinais e
//! clientes Wi-Fi (`docs/monitoramento_wifi_sinais.md`). Depende só do núcleo
//! e da base da área (`netmonitor-network`).
//!
//! * a fonte (qual plugin do equipamento declara o bloco `wifi`) fica na
//!   base da área: `netmonitor_network::plugins::wifi_source`;
//! * [`contract`] — o formato que todo plugin Wi-Fi entrega;
//! * [`station`] e [`quality`] — normalização e classificação, num lugar só;
//! * [`distance`] — a distância aproximada pelo sinal, no ambiente do AP;
//! * [`settings`] — as Configurações de Wi-Fi (mapa, distância, roaming);
//! * [`rounds`], [`live`] e [`routines`] — a rodada da frota, o coletor do
//!   mapa aberto (só com assinante) e a leitura leve em segundo plano;
//! * [`roaming`] — a troca de AP: histórico, evento ao vivo e ping-pong;
//! * [`collector`] — a leitura em segundo plano, publicada pelo SSE;
//! * [`ping`] — a latência do AP até uma estação;
//! * [`watchlist`] e [`watch`] — os aparelhos vigiados e o vigia que os
//!   procura a cada ciclo e alimenta os alertas;
//! * [`ai_tools`] — a seção `wifi` do `get_platform_status` da IA.

pub mod ai_tools;
pub mod collector;
pub mod contract;
pub mod controllers;
pub mod distance;
pub mod dtos;
pub mod live;
pub mod migrations;
pub mod ping;
pub mod quality;
pub mod roaming;
pub mod rounds;
pub mod routines;
pub mod settings;
pub mod station;
pub mod watch;
pub mod watchlist;
pub mod wifi_client;

pub use collector::{refresh, SNAPSHOT_EVENT};
pub use ping::ping;
