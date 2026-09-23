# ADR 011 — Agente remoto por canal de saída sobre a VPN

## Status

Aceito em 2026-09-22. Implementado em seis fases (F0–F6, ver `roadmap.md`):
canal e enrollment, Docker remoto, telemetria e histórico, absorção do probe,
pull/update/compose e provisionamento pela VPN.

Validação ponta a ponta feita com a central em PostgreSQL e o agente em
container (imagem `--target agent`) contra o Docker Desktop: enrollment,
reconexão com o token salvo, ciclo de vida remoto com auditoria `agent-<id>/…`,
recusa de ação sobre o próprio container, recreate com troca real de imagem e
preservação de ambiente e labels, snapshots ao vivo e rollups no histórico.

## Contexto

O NetMonitor enxerga só a Docker Engine do próprio host (ADR 010). Os
servidores de clientes já se ligam à central pela VPN WireGuard (seção 10 de
`arquitetura.md`), e o objetivo é, a partir da central:

- ver o que roda no Docker de cada servidor e gerenciar seus containers
  (ciclo de vida, pull/update e compose);
- coletar métricas de host e de containers para análise histórica;
- executar monitores e discovery a partir do site remoto, papel que hoje é do
  probe (`backend-cli task probe_run`).

O probe atual não serve de base para isso sem mudanças:

- faz short-polling HTTP a cada 5 s e não tem canal de comando da central para
  o agente;
- não transmite streams (logs em follow, progresso de pull);
- sobe um Loco completo só para reaproveitar `run_monitor`.

Quem fala com `docker.sock` tem root no host. Isso vale para o agente em cada
servidor remoto.

## Decisão

- **Um agente só**, evolução do probe: continua sendo uma linha de `probes`,
  agora com capacidades (Docker, métricas de host, monitores, discovery).
- **Canal de saída.** O agente abre um WebSocket persistente para a central,
  pelo IP dela no túnel. O agente **não escuta porta nenhuma**. A central nunca
  chama o agente, porque uma porta em cada host com poder de `docker.sock`
  seria exatamente o alvo a proteger. O WireGuard já cifra e autentica o
  transporte.
- **Protocolo tipado com correlação.** Os envelopes carregam `id` (UUID),
  request/response, stream e evento, com prazo em todo request. Os comandos
  Docker formam um enum fechado. **Não existe proxy genérico** para a API da
  Engine.
- **A central interpreta, o agente só busca.** A camada Docker se divide em:
  - `services/docker/source.rs`: o trait `DockerEngine`, com os métodos crus
    que devolvem o JSON da Engine;
  - `services/docker/engine.rs`: mapeamento, redação de segredos e regras.

  O host local usa `LocalEngine`, o `bollard` pelo socket. Um servidor remoto
  usa uma fonte que faz RPC pelo canal, e do lado de lá o agente executa o
  **mesmo** `LocalEngine`. A redação de ambiente e labels continua num único
  lugar.
- **Checks sem `AppContext`.** `run_monitor_with` e `scan_network_with`
  recebem `CheckDeps`, que hoje contém só o cliente ICMP. O agente é um binário
  que não sobe o Loco nem abre banco e reaproveita os mesmos checkers.
- **A política local do agente é soberana.** A lista de ações permitidas
  (`read`, `lifecycle`, `update`, `compose`, `monitor`, `discovery`) é
  configurada no servidor remoto e anunciada à central. A central não
  consegue ampliá-la, então comprometer a central não entrega update/compose
  em hosts que não os liberaram.
- **Token único por agente**, obtido por um código de enrollment de uso único
  e com validade curta. O `DEFAULT_VPN_PROBE_TOKEN` segue exclusivo do
  `vpn-probe`. Quando o agente está vinculado a um dispositivo VPN, a conexão
  precisa vir do IP daquele túnel.
- **O CLI `docker compose` só roda no agente**, no host remoto. O processo da
  API continua sem executar `docker` (ADR 010 e AGENTS §7).
- **O tempo real segue a regra do SSE.** O navegador continua recebendo tudo
  pelo stream global. A central só pede snapshots ao vivo aos agentes enquanto
  há assinantes. As métricas históricas chegam em rollups de 1 minuto,
  agregados no agente, com buffer offline.

## Consequências

- A camada Docker passa a ter uma fonte injetável, e os testes cobrem
  mapeamento e regras com uma fonte falsa, sem Engine real.
- O agente é um segundo binário do mesmo crate. Ele não duplica código, mas
  herda o tamanho do link. Extrair crates é otimização futura.
- O host remoto confia no agente como confia em qualquer software com acesso
  ao `docker.sock`. A mitigação é não abrir porta, manter a política local, o
  vínculo com o IP do túnel e a auditoria de toda mutação.
- Compose em modo container exige os diretórios dos projetos montados no mesmo
  caminho. Sem isso a ação falha com a mensagem do próprio CLI (que chega à
  tela pelo evento da operação), e o caminho recomendado é o binário via
  systemd.
- Os endpoints de polling do probe (`/api/probes/heartbeat|tasks|results`),
  `probe_run`, o registrador e o fallback local do agendador **permanecem**
  (AGENTS §6).
