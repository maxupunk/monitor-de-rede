pub mod app;
pub mod controllers;
pub mod dtos;
pub mod initializers;
pub mod mailers;
pub mod registries;
/// O modelo de dados mora no núcleo (`crates/core`); o caminho antigo continua valendo.
pub use netmonitor_core::models;
pub mod services;
pub mod spa;
pub mod tasks;
pub mod views;
