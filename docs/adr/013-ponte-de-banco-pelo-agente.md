# ADR 013 — Backup de banco que só o agente alcança: ponte TCP tipada

## Status

**Aceito** (2026-10-08) e implementado — alternativa A. Ver "Como ficou" no
fim: alguns detalhes mudaram ao implementar.

## Contexto

O backup nativo de bancos (`services/databases/`) conecta **da central** ao
servidor de banco com o `sqlx`. Um PostgreSQL ou MySQL que só existe na rede de
uma filial — onde roda um agente remoto (ADR 011) — não é alcançável: a central
não enxerga aquela rede, e o agente "só disca para fora".

Requisito: fazer esse backup de forma eficiente e funcional **sem** levar o
sistema de dump para o agente — evitar um segundo componente de dump para
manter, atualizar e versionar em cada filial.

### O que o canal do agente oferece hoje

Medido no código (`services/agents/`, `services/agent_runtime/`):

| Aspecto | Hoje |
|---|---|
| Transporte | Um WebSocket aberto **pelo agente**; só frames de texto JSON, até 4 MiB (`MAX_FRAME_BYTES`) |
| Formas de mensagem | `Request` → `Chunk`* → `Response`, `Cancel`, `Event` |
| Direção dos dados | Só **agente → central**. O agente ignora `Chunk` vindo da central |
| Controle de fluxo | Nenhum: canais de `Chunk` sem limite nos dois lados; a única fila limitada (64 envelopes) é compartilhada por todos os pedidos da conexão |
| Concorrência | 16 pedidos simultâneos por agente (`MAX_CONCURRENT_REQUESTS`) |
| Política | `AGENT_ALLOW` local e soberana; `device_io` fora do padrão; alvos só em rede privada/CGNAT/link-local, `AGENT_DEVICE_CIDRS` restringe mais |
| Binário do agente | `src/bin/agent.rs`, **mesmo crate** da central; sem atualização automática |

E do lado do dump:

- O `sqlx` só conecta por TCP ou socket Unix — não aceita um canal injetado.
- Um dump faz ~3 consultas por tabela (estrutura, colunas, dados) mais ~15 ao
  catálogo; o resto é um fluxo contínuo (`COPY` / `SELECT`).
- O `.sql.gz` real medido comprimiu **3,4×** (1.004.811 → 296.502 bytes);
  dado de negócio costuma comprimir de 4× a 10×.

## Alternativas

### A. Ponte TCP tipada pelo canal do agente *(recomendada)*

A central abre, só durante o backup, um listener em `127.0.0.1:<efêmera>` e
aponta o `sqlx` para ele. Cada byte vai pelo WebSocket já aberto até o agente,
que abre a conexão TCP com `host:porta` do banco **na rede dele** e devolve os
bytes. O dump continua 100% na central, com o mesmo código, os mesmos testes e
o mesmo arquivo; a restauração usa a mesma ponte no sentido inverso.

```
sqlx ──TCP──▶ 127.0.0.1:efêmera ══ WS (do agente) ══▶ agente ──TCP──▶ db:5432
 (central)        (ponte)                               (rede da filial)
```

**Vantagens**

- **Nenhum dump no agente.** O agente só copia bytes: não interpreta SQL, não
  conhece PostgreSQL nem MySQL, não precisa ser atualizado quando o dump muda.
- **Sem descompasso de versão.** Uma correção no dump vale para todas as
  filiais no deploy da central.
- **A senha do banco não precisa chegar ao agente.** O SCRAM do PostgreSQL e o
  `caching_sha2_password` do MySQL (com RSA, já habilitado) não mandam a senha
  em claro; com TLS ligado no banco, o agente só vê bytes cifrados de ponta a
  ponta. Na alternativa B o agente recebe a credencial.
- **Backup e restauração de graça**, com a mesma consistência (snapshot,
  transação única no PostgreSQL) e o mesmo andamento ao vivo.
- **Continua sem porta aberta no agente**: a ponte anda no WebSocket que ele
  mesmo abriu (ADR 011 preservado).

**Desvantagens e custos**

- **Fere a letra do "nenhum proxy genérico"** (ADR 011). Mitigação: não é
  genérico — comando fechado, só para destinos que o **agente** lista
  localmente, com permissão própria fora do padrão (ver "Regras" abaixo).
- **Mais bytes na WAN:** o dado atravessa sem compressão. Com o JSON atual
  (base64) seria ~1,33× o tamanho bruto, isto é **4× a 13× mais** que um
  `.sql.gz` gerado no agente. Mitigação: frames binários (1,0×) e, se o link
  for estreito, compressão por frame na ponte (zstd), que devolve a maior
  parte da diferença.
- **Latência por consulta:** cada uma das ~3·N consultas paga a ida e volta da
  VPN. Com 200 tabelas e 50 ms, ~30 s a mais — irrelevante perto do tempo de
  copiar os dados, mas sensível em bancos com milhares de tabelas.
- **Exige evoluir o protocolo** (é o grosso do trabalho):
  1. dados nos **dois sentidos** dentro de um pedido em andamento;
  2. **frames binários** para os dados (id da ponte + bytes), texto só para
     controle;
  3. **controle de fluxo por ponte** (janela de crédito, ex.: 256 KiB com
     `Ack`): sem ele, uma central lenta (gzip + envio ao S3) acumula memória
     sem limite, e um dump grande entope a fila compartilhada e atrasa os
     monitores e o Docker daquele agente;
  4. **versão do protocolo 2**: agente antigo continua funcionando para o resto,
     e a tela diz "atualize o agente para usar a ponte".
- O banco vê a conexão vinda do **IP do agente**: o usuário de backup precisa de
  permissão a partir dele (`pg_hba.conf`, `GRANT ... @'ip-do-agente'`).
- O listener local da central aceita **uma** conexão e fecha; no container o
  processo da API é o único que poderia se conectar nessa janela.

### B. Dump no agente

O agente roda o mesmo `services/databases` (é o mesmo crate) e devolve o
`.sql.gz` em `Chunk`s, como o `FollowLogs`.

- **Vantagens:** menos tráfego na WAN (vai comprimido); uma consulta por vez
  não paga latência; o protocolo de **ida** já existe (agente → central).
- **Desvantagens:** é exatamente o "segundo componente de dump": correção ou
  SGBD novo exigem **reinstalar o agente em cada filial** (não há atualização
  automática) e lidar com versões diferentes ao mesmo tempo; a **senha do banco
  viaja até o agente**; o binário do agente cresce com os drivers; o agente
  passa a executar SQL contra bancos da filial (superfície maior); a
  **restauração** precisa mandar o arquivo central → agente — o mesmo trabalho
  de protocolo da alternativa A, sem ganhar a simplicidade dela.

### C. Agente envia direto ao destino (S3/SFTP)

Descartada: o agente precisaria das credenciais do destino, e a central perde o
controle da retenção, do histórico e da soma de verificação.

### D. Rota pela VPN (site a site)

Sem código: o host do agente vira roteador da rede da filial no WireGuard
(`AllowedIPs` com a sub-rede, `ip_forward` e `MASQUERADE` no host).

- **Vantagens:** desempenho nativo, zero mudança no sistema; já possível hoje
  para quem administra os dois lados.
- **Desvantagens:** configuração manual de rede, com root, em cada filial; expõe
  **a sub-rede inteira** à central, não só a porta do banco; só serve quando o
  host do agente é peer da VPN. É uma saída operacional, não um recurso.

## Comparação

| | A. Ponte | B. Dump no agente | D. Rota VPN |
|---|---|---|---|
| Código de dump no agente | **Nenhum** | Todo | Nenhum |
| Atualizar agentes quando o dump muda | **Não** | Sim, todos | Não |
| Senha do banco no agente | **Não** | Sim | Não |
| Tráfego na WAN | Bruto (1×; menos com compressão por frame) | **Comprimido** | Bruto |
| Restauração | **Mesma ponte** | Protocolo novo central → agente | Nativa |
| Exposição na filial | Só `host:porta` listados no agente | Só `host:porta` | Sub-rede inteira |
| Trabalho | Protocolo v2 + ponte | Protocolo de volta + empacotar drivers | Zero (manual) |

## Decisão proposta

Adotar **A**, com estas regras:

1. **Comando fechado** `Command::DatabaseTunnel { host, port }` — não um proxy
   genérico: abre **uma** conexão TCP para um destino e só copia bytes.
2. **Permissão `database`**, fora do padrão do `AGENT_ALLOW`.
3. **Lista local obrigatória** `AGENT_DATABASE_TARGETS=10.0.0.20:5432,...` no
   agente: sem ela a ponte é recusada. Destino só em rede privada/CGNAT/link-local
   (a mesma regra do `device_io`). A central não amplia a lista.
4. **Vida curta:** a ponte existe só durante um backup ou uma restauração;
   prazo de ociosidade; no máximo 2 pontes por agente, para não ocupar os 16
   pedidos simultâneos.
5. **Auditoria** de cada abertura de ponte (quem, qual conexão, qual agente).
6. Na conexão de banco, um campo **"Acessar a partir de"** (central ou agente
   X), no padrão de `device_credentials.via_probe_id`: FK anulável para
   `probes`, `ON DELETE SET NULL`, opção desabilitada quando o agente está
   offline, sem a permissão ou com protocolo antigo.

## Plano de implementação

1. **Protocolo v2** (beneficia também o `FollowLogs`): frames binários de dados,
   dados nos dois sentidos, janela de crédito por fluxo, fila de controle
   separada da de dados para os monitores não esperarem o dump.
2. **Agente:** `DatabaseTunnel` no `LocalExecutor`, com a permissão, a lista e
   `tokio::io::copy` limitado pela janela; cancelamento fecha o socket.
3. **Central:** `TunnelListener` (127.0.0.1:0, uma conexão), e
   `DatabaseTarget` ganhando a origem (`via`) — os drivers não mudam; a dica de
   "127.0.0.1 é o container" (`runtime::loopback_hint`) não se aplica à ponte.
4. **Dados e tela:** `database_connections.via_probe_id`, seletor "Acessar a
   partir de" no formulário, aviso sobre `pg_hba`/`GRANT` pelo IP do agente.
5. **Testes:** a ida e volta de `tests/requests/database_roundtrip.rs` pela ponte,
   com agente em processo (como em `tests/requests/agents.rs`) e PostgreSQL /
   MySQL locais; teste de que destino fora da lista é recusado; teste de que um
   dump grande não atrasa um monitor do mesmo agente.
6. **Opcional:** compressão zstd por frame na ponte, se o link da filial for
   estreito.

## Consequências

- O ADR 011 ganha um adendo: o "nenhum proxy genérico" continua valendo; a ponte
  é um comando fechado, por destino listado no agente, como o `device_io` é por
  operação.
- O backup de banco passa a funcionar igual com ou sem agente; o operador só
  escolhe "Acessar a partir de".

## Como ficou (implementação)

O desenho de A foi seguido; o que mudou ao implementar, e por quê:

| Proposto | Implementado | Por quê |
|---|---|---|
| Protocolo **v2** | **v2**, com handshake estrito: versão diferente é recusada | O sistema não tinha ido para produção — não há agente antigo a preservar. Uma versão só, sem campo de recursos opcionais nem caminho "agente antigo" na central e na tela: central e agente sobem juntos |
| "Uma conexão" por listener | Listener aceita várias conexões, cada uma com a sua ponte | A restauração em banco novo abre duas (manutenção + banco); a primeira ponte é aberta **antes** de devolver o endereço, para a recusa do agente chegar ao operador com a razão dele |
| Lista só `IP:porta` | `IP:porta`, `faixa/prefixo:porta` ou `[IPv6]:porta`; o host do banco pode ser nome, resolvido **no agente** e conferido contra a lista | Nome de servidor (`db.filial.local`) é comum e só a rede do agente o resolve |
| Compressão zstd | zstd nível 1 **adaptativo**, por quadro (`DATA_ZSTD`) | O quadro só vai comprimido se ficar ≥ 10% menor; após 16 quadros seguidos sem ganho (banco com TLS, dado já compactado) a ponte para de tentar e reavalia a cada 256. O teto de 64 KiB vale **depois** de descomprimir |
| — | Crédito com custo mínimo de 4 KiB por quadro | A janela conta bytes, mas a fila do outro lado conta quadros: sem o piso, muitos quadros pequenos estouravam a fila dentro da janela |
| Retomar dump após queda | Refazer o banco do zero, até 3 tentativas | Um dump não retoma no meio (o snapshot da transação morre com a conexão); refazer é idempotente. A ponte local segue o **agente**, não a conexão: cada conexão aceita pega a sessão atual do hub |

### Peças

- **Fio** (`services/agents/protocol.rs`, `PROTOCOL_VERSION = 2`):
  `WireFrame::{Text, Binary}`; `TunnelFrame` binário
  `[id 16 B][tipo 1 B][conteúdo]` com dados crus ou zstd (≤ 64 KiB depois de
  descomprimidos), `Ack(crédito)` e `Eof` (meio-fechamento).
  `Command::DatabaseTunnel { host, port }` exige `Permission::Database`
  (`database`).
- **Bomba** (`services/agents/tunnel.rs`): o mesmo código nas duas pontas;
  janela de 256 KiB por ponte (cada quadro custa no mínimo 4 KiB), crédito
  devolvido a cada meia janela; quem estoura a janela perde a ponte. A
  compressão é decidida por quem envia, quadro a quadro; o agente devolve
  `sent`, `sentOnWire` e `received` ao fim de cada ponte.
- **Prioridade**: central e agente têm duas filas de saída — controle (64
  envelopes) e ponte (16 quadros) — e o escritor usa `biased select`. Medido
  em teste: com 48 MB passando por uma ponte e um leitor lento, um pedido comum
  do mesmo agente responde em menos de 500 ms.
- **Central** (`services/agents/bridge.rs`): `OpenTunnel` (pede a ponte e
  espera o "aberta") e `LocalBridge` (`127.0.0.1:<efêmera>`), que guarda o
  agente (`AgentHub` + id) e abre cada conexão pela sessão **atual** dele.
  Soltar a ponte cancela o pedido no agente e devolve a vaga.
- **Agente** (`services/agent_runtime/tunnel.rs` + sessão): `TunnelOpener`
  confere política, `AGENT_DATABASE_TARGETS` (obrigatória; vazia recusa) e rede
  privada; no máximo **2** pontes simultâneas por agente.
- **Backup** (`services/databases/reach.rs`): resolve a rota da conexão
  (`database_connections.via_probe_id`) e devolve o alvo apontando para a
  ponte; os drivers de PostgreSQL e MySQL **não mudaram**. O erro de conexão
  diz "pela ponte do agente X", e a dica de "127.0.0.1 é o container" não
  dispara na ponte.
- **Queda do canal** (`Reached::retrying`): o erro de banco distingue recusa
  do servidor (`Connection`: senha, banco inexistente), rede que não chegou
  (`Unreachable`) e conexão que caiu no meio (`Dropped`). Só as duas últimas,
  e só pela ponte, são refeitas: o backup espera o agente reconectar (até
  60 s, acordado pelo registro da sessão no hub, sem consulta repetida) e
  refaz o banco do zero, até 3 tentativas, na **mesma** linha do histórico —
  que ganha um aviso dizendo em que tentativa concluiu. A restauração não é
  refeita: reaplicar um arquivo pela metade não é seguro.
- **Tela**: "Acessar a partir de" no formulário da conexão, com os agentes que
  servem (mesma lista dos plugins, `agents::routes::route_options`) e as duas
  linhas a configurar no host do agente; a conexão mostra "pelo agente X".
- **Auditoria**: backup manual e restauração registram por qual agente
  passaram; toda abertura de ponte vai ao log.

### Configuração no host do agente

```
AGENT_ALLOW=read,lifecycle,monitor,discovery,database
AGENT_DATABASE_TARGETS=10.0.0.20:5432,10.0.0.0/24:3306
```

Central e agente precisam estar no protocolo 2: o handshake recusa versão
diferente.

### Testes

- Unidade: codec do quadro binário (inclusive quadro comprimido que estoura
  o teto), bomba (dados nos dois sentidos acima da janela, compressão > 3× em
  linhas de `COPY`, espera sem crédito, quadros pequenos sem estourar a fila,
  desistência e nova tentativa de comprimir, cancelamento), espera do hub pela
  reconexão, política de nova tentativa (refaz pela ponte, desiste após 3, não
  repete erro de conteúdo nem conexão direta), lista de destinos, política.
- Integração com agente em memória (`tests/requests/agent_harness.rs`): ida e
  volta completa **pela ponte** contra PostgreSQL, MySQL e MariaDB reais, com o
  mesmo resultado da rota direta; destino fora da lista e agente sem permissão
  recusados com a razão; ponte cheia não atrasa outros pedidos; vaga liberada
  ao fechar; a mesma ponte local segue o agente depois de ele reconectar; API
  com "Acessar a partir de".
