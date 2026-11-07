# ADR 019 — Papel por área

- **Status:** aceito — 2026-11-06 (entrevista com o responsável pelo produto).
- **Data:** 2026-11-06
- **Relacionado:** [ADR 017](017-arquitetura-modular.md) (Áreas do produto),
  [roadmap da arquitetura modular](../roadmap_arquitetura_modular.md) (Fase 15).

## Contexto

O ADR 017 separou o produto em quatro áreas (Núcleo, Rede, Servidores, VPN) e
deixou claro que **área não é controle de acesso**: desligar uma área esconde e
pausa, mas quem autoriza é o papel global do usuário (`admin`, `operator`,
`viewer`). O roadmap deixou "papel por escopo" fora do plano, à espera de
pedido. O pedido veio: equipes em que alguém opera os servidores e só observa a
rede, ou cuida da rede e não deve ver os túneis da VPN.

## Decisão

1. **O papel passa a valer por área.** Cada usuário que não é administrador tem,
   em cada uma das áreas Rede, Servidores e VPN, um destes níveis:
   `operator`, `viewer` ou **sem acesso**. O Núcleo segue o papel global.
2. **Sem papel definido numa área = sem acesso.** Cada área é liberada de
   propósito. Para não virar trabalho, a tela de usuário oferece atalhos
   ("mesmo papel em todas as áreas") e mostra a matriz inteira de uma vez.
3. **O administrador global vale em tudo** e é o único que atribui papéis. Não
   existe admin por área: ninguém se tranca fora e a gestão de usuários tem
   um dono só.
4. **"Sem acesso" a uma área significa, para aquele usuário:**
   - a área some do menu e das rotas; link direto mostra a página de "sem
     permissão" (no padrão da `AreaOffPage`, com o motivo e quem procurar);
   - as ferramentas da IA daquela área saem do registro dele — o mesmo filtro
     de `ToolPolicy` que já esconde as de área desligada;
   - os widgets da área não aparecem no dashboard dele e o SSE não entrega os
     eventos dela;
   - ele não recebe notificação (push, e-mail, canais) de alertas daquela área.
5. **Área continua não sendo controle de acesso global.** Desligar a área (ADR
   017) e negar o acesso a um usuário são coisas distintas, combinadas por
   interseção: o usuário vê uma área se ela está ligada **e** ele tem acesso.

## Como se encaixa (SOLID e DRY)

- **Uma regra só, no `core`:** `AreaAccess` (papel efetivo de um usuário numa
  área) é calculado num lugar — `netmonitor_core::roles` — e consumido por quem
  já consulta a área: o guard da API, a `ToolPolicy`, o filtro do SSE, o
  catálogo de widgets e o despacho de notificações. Nenhum `if área` novo fora
  dos registros (AGENTS §13).
- **A rota declara a área uma vez.** O guard do controller lê a área do grupo
  de rotas registrado pela área, não de cada handler.
- **No frontend, o mesmo filtro que esconde área desligada** passa a receber o
  conjunto "áreas visíveis para mim" (`enabledAreas ∩ minhasÁreas`), vindo do
  snapshot SSE da sessão. Menu, rotas, widgets e handlers SSE não mudam de
  forma; mudam de fonte.
- **Armazenamento:** coluna `users.area_roles` (JSON, `{"servers": "operator"}`),
  na migration existente de usuários (sistema ainda sem produção). *Ajustado na
  implementação:* com três chaves fixas, uma coluna é mais simples que a
  tabela `user_area_roles` pensada antes, não entra na lista de truncagem e é
  lida junto do usuário que o guard já carrega.
- **Onde a regra vale (implementado):** o guard único da API (`require_jwt`)
  descobre a área da rota pelos prefixos que cada `AreaPolicy` declara e aplica
  a política de sempre com o papel **naquela área**; o SSE filtra por
  `EventAudience` (prefixos de evento da área; `user:*` só ao próprio
  usuário, e o acesso novo vale na mesma conexão); a IA usa
  `ToolPolicy::with_access` (só áreas visíveis; ações só onde o papel escreve);
  o Web Push de alerta vai só a quem vê a área do alerta (a área sai do modelo
  da regra; sem modelo, o Núcleo). E-mail, Telegram, Discord e webhook são
  destinos globais configurados pelo administrador e não mudam.
- **Achado:** escrever em `/api/docker` e tudo em `/api/databases` continua
  exigindo **administrador** (`can_manage_docker`, regra anterior a este ADR).
  Por isso o operador de Servidores, hoje, só lê ali — liberar ações de
  container ao operador da área é decisão de segurança à parte.

## Usabilidade

- Tela de usuário: uma matriz compacta área × nível com chips coloridos
  (operator `primary`, viewer `info`, sem acesso `error` contornado), atalho
  "mesmo papel em todas" e aviso claro quando o usuário fica sem nenhuma área.
- Quem perde o acesso no meio da sessão vê a mudança na hora (SSE), com um
  aviso discreto, sem recarregar a página.
- A mensagem de "sem permissão" diz qual área e quem é o administrador.

## Consequências

- `require_admin`/`can_write` ganham uma variante por área; os testes de
  requisição passam a cobrir viewer e sem-acesso por área.
- A IA e as notificações dependem da sessão/destinatário saber as áreas dele:
  o contexto de execução das ferramentas carrega o `AreaAccess`.
- Rotina sem usuário (scheduler, alertas) não é filtrada por papel; só a
  entrega da notificação é.
