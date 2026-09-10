# Correção 05 — Timeout do cliente para as chamadas SNMP

- **Data:** 2026-09-09
- **Severidade:** média (a tela mentia sobre o que estava acontecendo)
- **Arquivos:** `frontend/src/services/apiService.ts`, `frontend/src/stores/deviceDetail.ts`,
  `frontend/tests/stores/deviceDetail.spec.ts`, `frontend/tests/services/apiService.spec.ts`

## O defeito

`applySnmpMonitors` usava o teto padrão do `apiService`:

```ts
const DEFAULT_TIMEOUT_MS = 15000
```

Esse número é dimensionado para API que só lê banco. `POST /snmp/apply-monitors` faz
**dois** walks SNMP completos — um para decidir o que aplicar e outro no poll final —
mais uma escrita por interface. Cada requisição SNMP tem 4 s de timeout e retry, sobre
UDP.

Quando estoura, o `AbortController` cancela a requisição **no navegador**. O backend não
sabe disso e continua escrevendo. O resultado é exatamente o relato: *"a tela fica
carregando, não acontece nada"* — e a gravação seguiu do outro lado sem ninguém saber.

O mesmo valia para `/snmp/scan`, `/snmp/poll` e para o PATCH que liga o monitoramento de
uma interface — este último com um comentário no código admitindo o problema ("o backend
já executa uma coleta — por isso a chamada demora e tem indicador próprio") sem tratá-lo.

### A assimetria que empurrava de volta ao padrão

Só `post` (e `download`/`postStream`) aceitavam `ApiRequestOptions`. `get`, `put`,
`patch` e `delete` não tinham como declarar timeout — então uma operação longa que não
fosse POST ficava presa nos 15 s sem alternativa. A assimetria era acidental, não uma
decisão.

## O que foi feito

### `ApiRequestOptions` em todos os verbos

`get`, `put`, `patch` e `delete` passaram a aceitar `options`, como `post` já fazia. O
default continua o mesmo, então nenhuma chamada existente muda de comportamento.

### `SNMP_REQUEST_TIMEOUT_MS = 120_000`

Constante exportada em `deviceDetail.ts`, no mesmo formato do `PROVISION_REQUEST_TIMEOUT_MS`
que `logs.ts` já usa. Aplicada nas quatro chamadas que esperam o equipamento responder:

| Chamada | Trabalho do backend |
|---|---|
| `POST /snmp/apply-monitors` | dois walks + escrita por interface |
| `POST /snmp/scan` | um walk |
| `POST /snmp/poll` | um walk + gravação de métricas |
| `PATCH /interfaces/{id}/monitoring` | escrita + coleta quando habilita |

### Erro residual em `applySnmpMonitors`

Era a única das quatro que não fazia `error.value = null` antes de tentar. A mensagem de
uma tentativa que falhou sobrevivia à seguinte, mesmo bem-sucedida.

## Testes

`frontend/tests/stores/deviceDetail.spec.ts` (novo, 4 casos) afirma que o teto declarado
é maior que o padrão global e que cada uma das quatro chamadas o repassa.

`frontend/tests/services/apiService.spec.ts` ganhou o caso do `patch`: com timers falsos,
a requisição **não** é abortada aos 15 s e é abortada no teto declarado. O teste do `post`
já existia e cobria só ele.

## Validação

```
pnpm --prefix frontend run typecheck   # limpo
pnpm --prefix frontend run lint        # 0 erros
pnpm --prefix frontend run build       # ok
pnpm --prefix frontend run test        # 25 arquivos, 94 testes, 0 falhas
```

## Observação sobre `pnpm run format`

`prettier`/`eslint --fix` reindentam 11 arquivos sem relação com esta mudança — é drift
entre o código commitado e as versões atuais das ferramentas. Esses arquivos foram
revertidos para manter o diff legível. `eslint src` sem `--fix` reporta 0 erros e 30
warnings de indentação, todos pré-existentes.

## O que este item **não** resolve

120 s é margem, não solução. Se `apply-monitors` chega perto disso, o problema é o
endpoint fazer dois walks sequenciais para um trabalho que pede um. Fica anotado como
melhoria: o poll final poderia reaproveitar a varredura que a guarda da correção 01 já
validou, cortando metade do tempo de parede.
