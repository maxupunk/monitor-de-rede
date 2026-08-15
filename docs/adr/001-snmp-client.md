# ADR 001 — Cliente SNMP assíncrono compartilhado

- **Status:** aceito — revisado na Fase 9
- **Data original:** 2026-08-10
- **Revisão:** 2026-08-15
- **Spike atual:** [`async_snmp.rs`](../../backend/examples/spikes/async_snmp.rs)

## Contexto

O primeiro backend Rust usava `rasn-snmp` apenas como codec e mantinha socket,
retry, correlação de resposta, walk e parte do USM dentro do projeto. A auditoria
da Fase 9 encontrou os efeitos desse custo: socket novo por consulta/GETNEXT,
retry fixo, ausência de GETBULK efetivo e SNMPv3 auth/priv incompleto.

## Decisão

Usar `async-snmp` atrás do contrato de domínio `SnmpClient`. Coletores continuam
dependendo somente de `SnmpClient`, `SnmpValue` e `SnmpError`; tipos da biblioteca
não atravessam essa fronteira.

O adaptador configura:

- um `UdpTransport` compartilhado por família IP no processo;
- validação conjunta de endereço de origem e request ID feita pelo transporte;
- retry exponencial com jitter;
- GETBULK em v2c/v3 e GETNEXT em v1;
- fallback para GETNEXT e detecção de OID não crescente em agentes defeituosos;
- v3 `noAuthNoPriv`, `authNoPriv` e `authPriv`;
- cache compartilhado de engine e de chaves-mestre derivadas;
- limite de 20.000 itens por walk.

## Evidência

O teste local sobe um agente UDP real em porta efêmera e consulta o mesmo OID em
v1, v2c e v3 authPriv. O walk v3 percorre a MIB via GETBULK. Toda rede de teste é
restrita a `127.0.0.1`.

## Consequências

- O código do produto deixa de manter ASN.1/BER e remove `rasn`, `rasn-smi` e
  `rasn-snmp` das dependências de produção.
- Credenciais permanecem no modelo de domínio e nunca entram em logs ou
  resultados de discovery.
- `async-snmp` ainda é pré-1.0; o adaptador isolado reduz o custo de uma futura
  atualização ou troca.

## Decisão substituída

A escolha original por `rasn-snmp 0.18` com transporte próprio fica preservada
no histórico Git anterior a esta revisão, mas não descreve mais o runtime.
