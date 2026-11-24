//! Servidor MCP (Model Context Protocol — Streamable HTTP) do NetMonitor (ADR 024).
//!
//! Expõe o catálogo de ferramentas do núcleo e das áreas para agentes externos
//! (Antigravity, Cursor, Claude Code, VS Code) sem exigir IA local habilitada:
//!
//! * [`settings`] — ativação global e trava de borda pública em `system_settings`;
//! * [`scopes`] — política de escopos fechados e autorização em 3 camadas ao vivo;
//! * [`tokens`] — ciclo de vida de tokens pessoais (`nmm_*`, hash SHA-256, TTL obrigatório);
//! * [`auth`] — validação de origem, Bearer token e rate limiting por IP e por token;
//! * [`protocol`] — endpoint Streamable HTTP em `/mcp` (JSON-RPC 2.0);
//! * [`controllers`] / [`dtos`] — API REST de gestão sob `/api/mcp/*`;
//! * [`events`] — publicação SSE `mcp:changed` (zero polling).

pub mod auth;
pub mod controllers;
pub mod dtos;
pub mod events;
pub mod migrations;
pub mod protocol;
pub mod scopes;
pub mod settings;
pub mod tokens;
