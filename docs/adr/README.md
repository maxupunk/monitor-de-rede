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
| [007](007-scheduler-processo-unico.md) | Scheduler: laço em processo único; deps de processo em `after_context` | — | aceito |
| [008](008-syslog-parser.md) | Syslog: `syslog_loose` com resgate do `<pri>` e severidade por tópico | SPIKE-06 | aceito |
| [009](009-device-adapters.md) | Plataformas: registro único com `DeviceAdapter` e adapters especializados | — | aceito |
