# ADR 002 — Port scanner: algoritmo do RustScan, não a crate

- **Spike:** SPIKE-02 (§3.4 do `roadmap_backend_rust.md`)
- **Status:** aceito — Fase 0
- **Data:** 2026-08-10

## Contexto

A §3.3 exige substituir o `PortScannerService` (concorrência fixa de 16, sobre
`net.Socket`) pela estratégia do RustScan: batch derivado do `ulimit`,
`for_each_concurrent` e timeout adaptativo. A pergunta do spike é se dá para
**embutir a crate `rustscan`** em vez de reimplementar.

## Decisão

**Implementar a estratégia em `src/services/network_tools/port_scanner.rs`.
Não depender da crate `rustscan`.**

## Evidência

A crate `rustscan` 2.4.1 **tem** um alvo de biblioteca — a resposta ingênua
seria "dá para usar". Três achados dizem o contrário, e o primeiro sozinho
encerra a discussão:

### 1. Licença incompatível (decisivo)

```toml
# ~/.cargo/registry/src/*/rustscan-2.4.1/Cargo.toml
license = "GPL-3.0-only"
```

O projeto é MIT (`backend/package.json`). Linkar uma biblioteca GPL-3.0-only
obrigaria a relicenciar o backend inteiro sob GPL-3.0. Isso é uma decisão de
produto, não de engenharia, e não está no escopo de uma migração.

### 2. A API não entrega resultado incremental

```rust
// rustscan-2.4.1/src/scanner/mod.rs
pub async fn run(&self) -> Vec<SocketAddr>
```

`run` devolve tudo no fim. O §7.15 exige **NDJSON porta a porta** — o frontend
(`PortScanDialog`) desenha cada porta assim que ela chega. E a §3.3.6 exige
cancelamento por `CancellationToken` ligado ao `on_disconnect` da resposta: a
assinatura não aceita nem canal de saída, nem token.

### 3. O que ela devolve é menos do que o contrato precisa

`Vec<SocketAddr>` só diz "estas abriram". O contrato atual (§3.3.4) distingue
`open` / `closed` / `open|filtered` no UDP, e o timeout adaptativo da §3.3.3
precisa do **RTT por conexão bem-sucedida** — nenhum dos dois sobrevive ao tipo
de retorno. `Scanner::new` também carrega parâmetros de CLI (`greppable`,
`accessible`) que não fazem sentido embutidos num serviço HTTP.

## Consequências

- O algoritmo é reimplementado, não copiado: lotes de 1.024,
  `for_each_concurrent`, limite global configurável e limite por host.
- O timeout adaptativo usa média móvel, variação e perda; perfis confiável e
  completo mantêm pisos conservadores. Falhas transitórias recebem backoff com
  jitter e `ConnectionRefused` nunca é repetido.
- Sem dívida de licença e sem acoplamento a uma API que a montante pode quebrar
  entre patches (a crate é publicada para o binário; o `[lib]` é efeito
  colateral).
- O comportamento externo é o descrito na §3.3 — este ADR não muda o roadmap.
- Os perfis `rápido`, `confiável` e `completo` controlam concorrência, timeout e
  retries sem duplicar o scanner.

**Critério de aceite:** nenhuma porta aberta da fixture local pode ser perdida;
a comparação externa com `nmap -sT` roda no ambiente Linux de precisão.
