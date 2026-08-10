# Decisões de arquitetura (ADR)

Registro das decisões técnicas do backend Rust. Cada arquivo tem contexto,
decisão, **evidência medida** e consequências — incluindo o trabalho extra que a
decisão cria.

| # | Decisão | Spike | Status |
| :-: | :--- | :--- | :--- |
| [001](001-snmp-client.md) | Cliente SNMP: `rasn-snmp` com transporte próprio em `tokio` | SPIKE-01 | aceito |
| [002](002-rustscan-embedding.md) | Port scanner: algoritmo do RustScan, não a crate (licença GPL + API sem streaming) | SPIKE-02 | aceito |
| [003](003-icmp-dgram.md) | ICMP por `SOCK_DGRAM`, sem `CAP_NET_RAW` | SPIKE-03 | aceito |
| [004](004-dns-wire.md) | DNS: `hickory-proto` no formato wire, sem cliente pronto | SPIKE-04 | aceito |
| [005](005-scheduler-loco.md) | Scheduler: task de um ciclo, disparada pelo scheduler nativo | SPIKE-05 | aceito |
| [006](006-prioridade-do-padrao-rust.md) | Padrão do backend Rust tem precedência; o frontend adapta | — | aceito |

Contexto geral: [`../roadmap_backend_rust.md`](../roadmap_backend_rust.md).
