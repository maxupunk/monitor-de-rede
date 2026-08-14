//! Regras de autenticação que não cabem no controller.
//!
//! O login, o reset e o magic link continuam sendo os fluxos do Loco, que já
//! moram em [`crate::models::users`]. O que vive aqui é o que o framework não
//! traz pronto: o **bootstrap** — como um sistema recém-instalado, sem nenhum
//! usuário no banco, ganha o primeiro deles sem senha padrão no código-fonte.

pub mod setup;
