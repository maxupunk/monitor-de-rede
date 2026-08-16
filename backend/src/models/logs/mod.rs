//! Entidades do **banco de logs** (`/data/logs.sqlite`), separado do principal.
//!
//! Ficam fora de `models::_entities` de propósito: aquele diretório é gerado
//! por `cargo loco db entities` contra a base principal, e um arquivo escrito à
//! mão lá dentro seria apagado na próxima geração.

pub mod device_logs;
