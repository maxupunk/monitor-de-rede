//! Monitoramento de sinais e clientes Wi-Fi (`docs/monitoramento_wifi_sinais.md`).
//!
//! * [`source`] — qual plugin do equipamento declara o bloco `wifi`;
//! * [`contract`] — o formato que todo plugin Wi-Fi entrega;
//! * [`station`] e [`quality`] — normalização e classificação, num lugar só;
//! * [`collector`] — a leitura em segundo plano, publicada pelo SSE;
//! * [`ping`] — a latência do AP até uma estação;
//! * [`watchlist`] e [`watch`] — os aparelhos vigiados e o vigia que os
//!   procura a cada ciclo e alimenta os alertas.

pub mod collector;
pub mod contract;
pub mod ping;
pub mod quality;
pub mod source;
pub mod station;
pub mod watch;
pub mod watchlist;

pub use collector::{refresh, SNAPSHOT_EVENT};
pub use ping::ping;
