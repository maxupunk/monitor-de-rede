# ADR 012 — Plugins de dispositivo: script Rhai em sandbox, E/S por transporte único

## Status

Aceito em 2026-09-30. Implementado em `backend/src/services/plugins/`, aba
**Plugins** de `/devices/{id}` e biblioteca em `/plugins`.

## Contexto

O NetMonitor só **observava** os equipamentos (ICMP, SNMP, TCP). Agir num
deles — instalar um pacote num OpenWrt por SSH, mudar uma configuração pela
interface web na porta 80 — não tinha caminho. A única execução remota era a
ativação de syslog (`services/syslog/provision.rs`), fixa no código; o
`DeviceAdapter` (ADR 009) também é compilado no binário.

O pedido: "drivers" por modelo/versão de equipamento, reaproveitáveis entre
equipamentos compatíveis ou exclusivos de um, criados à mão, importados,
exportados ou **gerados pela IA do chat**, sempre com testes, com
documentação de uso e — por ser a IA quem escreve — com o usuário aprovando
cada acesso ao equipamento (ou ligando, ciente do risco, um modo automático).

## Decisão

1. **Linguagem: Rhai embutido** (`rhai` com `sync`, `serde`, `no_module`).
   Rust puro, sem runtime externo, e o script só enxerga o que é registrado:
   não há `import` (feature `no_module`), `eval` é desligado, não há acesso a
   disco, processo ou rede. Limites de operações, profundidade, tamanho e um
   prazo de relógio. Shell/Python no próprio equipamento foi descartado: não
   serve à interface web, depende do interpretador do aparelho e não se testa
   nem se audita. Um formato só declarativo foi descartado por não expressar o
   que a IA precisa (condição, parsing, login com cookie).

2. **Toda E/S por um transporte único** (`DeviceTransport`, uma operação
   autocontida `DeviceIoCall`): `LocalTransport` (SSH `exec` com `russh`,
   Telnet do provisionamento, HTTP `reqwest` sem redirecionamento),
   `AgentTransport` e `FakeTransport` (fixtures). É o ponto de auditoria, de
   aprovação e de simulação. HTTP só aceita caminho relativo: o plugin alcança
   o IP cadastrado e nada mais.

3. **O script roda sempre na central.** Equipamento só alcançável por um agente
   remoto recebe operações unitárias (`Command::DeviceIo`, permissão
   `device_io` fora do padrão do `AGENT_ALLOW`, alvo só em rede privada e,
   opcionalmente, em `AGENT_DEVICE_CIDRS`). O agente continua com enum fechado
   e sem interpretador — ADR 011 preservado.

4. **Três portas por chamada**, nesta ordem: transporte declarado no
   manifesto; efeito da chamada (classificação heurística de comando/método)
   não pode exceder o da ação — ação `read` que tenta escrever é **recusada**;
   e o `AccessGate` da execução aprova. Plugin ativo disparado pelo operador
   passa direto (escrita exige confirmação explícita na tela); rascunho,
   testado não ativo e a IA sem modo automático pausam **a cada chamada** e
   publicam `plugin:approval_required` como snapshot no SSE global.

5. **Segredos fora do script e da IA.** Credencial por equipamento e tipo:
   `vault` (cifrada com `crypto::encrypt`) ou `ask` (só em memória, 30 min).
   O script escreve `{{username}}`/`{{password}}`; a troca acontece no último
   instante e o transcript passa por máscara.

6. **Pacote `.nmplugin`** com manifesto, script, `usage` (documentação de uso,
   viaja com o plugin), `compatibility` (validações funcionais registradas) e
   testes unitários (fixtures; chamada sem fixture reprova) e funcionais (no
   equipamento, só leitura). O `checksum` cobre o código e amarra teste e
   revisão ao código testado. A skill de **autoria**
   (`services/ai/knowledge/plugins.md`) é da IA e não viaja com o pacote.

7. **Ciclo de vida**: importado → `quarantine` → (revisão aceita) → `draft` →
   (testes) → `tested` → (operador) → `active`. Importação passa por análise
   estática determinística e por revisão da IA configurada; risco `critical`
   bloqueia, `high` ou sem IA exige declaração do operador.

8. **Modo "Aceitar automaticamente"** da IA: por conversa × equipamento, com
   termo versionado aceito, gravado em `plugin_auto_accept` e na auditoria,
   validade de 2 h e botão "Parar". A chave da conversa é aleatória por sessão
   de chat — sequencial seria reaproveitada depois de recarregar a página.

## Consequências

- Quatro tabelas novas (`plugins`, `device_credentials`, `plugin_runs`,
  `plugin_auto_accept`) e `devices.firmware_version`, gravado pelo `detect`.
- Guardar senha de equipamento passou a ser possível — é escolha por
  credencial, cifrada, e nunca volta para a tela nem para a IA. A ativação de
  syslog continua sem guardar.
- A heurística de efeito pode errar para "escrita" (confirmação a mais); errar
  para o outro lado custaria o equipamento.
- Rotas de escrita de plugin são só de administrador, como o Docker.
- A IA ganha o grupo de ferramentas `devices` (`ToolKind::DeviceAccess`,
  sempre com confirmação por chamada, exceto no modo automático).
