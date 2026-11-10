//! Registry de imagens OCI do NetMonitor (ADR 022), submódulo da área
//! Servidores.
//!
//! * a OCI Distribution Spec em `/v2` ([`api`]), com o token do Docker
//!   ([`auth`]) emitido para tokens de acesso pessoal ([`tokens`]);
//! * o catálogo ([`catalog`]) sobre o storage endereçado por conteúdo
//!   ([`storage`]);
//! * a ponte com o deploy ([`store`]): a imagem que o agente compilou entra
//!   ([`ingest`]) e sai para o `docker load` do host ([`archive`]);
//! * a varredura de vulnerabilidades ([`scan`]) e a retenção ([`retention`]).

pub mod api;
pub mod archive;
pub mod auth;
pub mod browse;
pub mod catalog;
pub mod controllers;
pub mod digest;
pub mod events;
pub mod ingest;
pub mod manifest;
pub mod migrations;
pub mod names;
pub mod retention;
pub mod scan;
pub mod settings;
pub mod storage;
pub mod store;
pub mod tarstream;
pub mod text;
pub mod tokens;
pub mod views;

pub use store::Registry;
