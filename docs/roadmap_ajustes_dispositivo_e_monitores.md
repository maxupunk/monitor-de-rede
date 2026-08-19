# Roadmap — Ajustes de dispositivo, regras e abertura de monitores

> **Objetivo**: corrigir quatro comportamentos da interface que hoje contradizem
> o que o produto já decidiu em outro lugar — um monitor que não faz sentido
> existir, um escopo de regra travado onde deveria ser uma escolha, um resumo de
> saúde apertado e um detalhe de monitor que só abre pelo nome.
>
> **Regra que decide todo impasse deste roadmap**: um comportamento vale para
> **todos os pontos que o exibem**, não só para aquele em que o problema foi
> notado. Se a correção não puder ser feita no componente compartilhado, é
> porque a derivação é que está no lugar errado — e é ela que se corrige.
>
> **Estado**: `🟢 Concluído`. As quatro fases estão implementadas e a matriz de
> validação da seção 4 passa inteira. O que foi encontrado fora do plano
> original está na seção 6 — nenhum achado ficou sem correção.
>
> **Antecessor**: [`roadmap_servidor_netmonitor_como_dispositivo.md`](roadmap_servidor_netmonitor_como_dispositivo.md).
> Os quatro itens abaixo nasceram do uso real do que aquele roadmap entregou;
> nenhum deles revoga uma decisão dele.

## 1. Decisões de produto

- **O dispositivo do sistema não é alcançado pela rede.** Nenhum monitor de
  alcance — ping, TCP, HTTP, DNS — pode existir apontando para ele. O que mede a
  saúde do servidor é a coleta local `system_health`, e um ping do host para si
  mesmo responde sempre e não informa nada.
- **Escopo de regra é escolha, não herança.** Abrir o formulário de dentro de um
  dispositivo pré-seleciona aquele dispositivo, mas continua permitindo criar
  uma regra global — há condições genuinamente de parque, e o operador que está
  olhando um equipamento é justamente quem percebe isso.
- **O resumo de saúde é a informação principal da Visão Geral**, e a largura
  precisa acompanhar essa hierarquia.
- **A linha inteira abre o detalhe.** Exigir o clique no nome é uma armadilha de
  precisão: o alvo tem a largura do texto, e o resto da linha — que é a maior
  parte dela — não faz nada. Onde a linha abre o detalhe, o botão que fazia só
  isso deixa de ter função.
- **Botão de ação nunca é engolido pela linha.** Testar, editar, excluir e o
  interruptor de ativação continuam funcionando exatamente como hoje.

## 2. O que estava no código antes desta entrega

Levantamento feito contra o código-base, não contra memória. **É a foto de
antes**: os treze achados abaixo estão todos endereçados pelas fases da seção 3
e não descrevem mais o estado atual — a seção 6 registra o que apareceu além
deles.

| # | Achado | Onde |
|---|---|---|
| 1 | `sync_device_monitor` **tem** a guarda do dispositivo do sistema | `controllers/devices.rs` |
| 2 | `vpn::monitor_provisioner::provision` cria ping **sem** guarda | `services/vpn/monitor_provisioner.rs` |
| 3 | `POST /api/monitors` aceita qualquer tipo para qualquer `deviceId`, **sem** guarda | `controllers/monitors.rs` |
| 4 | Nada remove um ping que já exista no dispositivo do sistema | — |
| 5 | Os dois criadores de ping usam `ip_address` **ou o nome** como alvo | `devices.rs`, `monitor_provisioner.rs` |
| 6 | `escopoFixo` desabilita o seletor inteiro | `components/AlertRuleFormDialog.vue` |
| 7 | A aba Regras filtra por `regra.deviceId === deviceId` — regra global não aparece | `components/devices/DeviceRulesTab.vue` |
| 8 | Cards de saúde em `cols=12 sm=6 md=4` (três por linha) | `components/devices/DeviceHealthSummary.vue` |
| 9 | `MonitorsTable` usa `:clickable="false"`; só o nome e a timeline abrem | `components/MonitorsTable.vue` |
| 10 | Botões de linha **não** têm `@click.stop` — hoje não precisam | `components/MonitorsTable.vue` |
| 11 | O widget `network_monitors` é um `v-list` próprio, não usa `MonitorsTable` | `pages/DashboardPage.vue` |
| 12 | `UnstableTargetsWidget` navega por `router.push` para o monitor | `components/widgets/UnstableTargetsWidget.vue` |
| 13 | `ResponsiveDataTable` já suporta `clickable` + `@click:row`, no desktop e no mobile | `components/ResponsiveDataTable.vue` |

O achado **5** é a causa raiz do **1**: o ping do servidor nasceu de um alvo que
cai para o nome do dispositivo quando não há IP. A guarda resolveu o sintoma num
dos três caminhos.

A pergunta que os achados 1 a 5 respondiam em três lugares mora hoje num só:
`backend/src/services/monitoring/reachability.rs` define o que é um monitor de
alcance, para quem ele pode existir, qual é o alvo de um provisionamento
automático e como se desfaz o que versões anteriores gravaram. Os quatro
caminhos de criação chamam esse módulo; nenhum decide por conta própria.

## 3. Fases de implementação

### Fase 1 — O dispositivo do sistema não recebe monitor de alcance `🟢 Concluído`

- [x] Definir, em um único lugar do domínio, o que é um **monitor de alcance**
  (`ping`, `tcp`, `http`, `https`, `dns`) e uma função que responde se um tipo é
  válido para um dispositivo. A pergunta é do domínio, não do controller: hoje
  ela seria respondida em três lugares e já diverge em dois.
- [x] Aplicar essa validação em **todos** os caminhos que criam monitor:
  `POST /api/monitors`, `PUT /api/monitors/{id}` (quando muda `deviceId` ou
  tipo), `sync_device_monitor` e `vpn::monitor_provisioner::provision`.
  O erro é de negócio, em português, e diz por quê — não um 422 de validação
  genérica.
- [x] **Remover, no boot, um monitor de alcance que já exista no dispositivo do
  sistema.** A guarda de hoje impede criar, mas não desfaz o que já está no
  banco de quem atualizou no meio do caminho. A remoção é idempotente e roda no
  mesmo `Initializer` que garante o dispositivo e a coleta de saúde.
- [x] Registrar a remoção em log com o id e o nome do monitor removido: apagar
  dado do operador em silêncio não é aceitável, mesmo quando o dado é inútil.
- [x] **Corrigir a raiz: alvo que cai para o nome do dispositivo.** Os dois
  criadores de ping usam `ip_address` **ou** `device.name` como host. Para
  qualquer dispositivo sem IP — não só o servidor — isso gera uma checagem que
  só pode falhar. Sem IP, o monitor de alcance não é criado, e o motivo aparece
  no cadastro.
- [x] Cobrir por teste: criar ping para o dispositivo do sistema é recusado
  pelos quatro caminhos; um ping preexistente é removido no boot; um
  dispositivo comum sem IP não ganha monitor de alcance; um dispositivo comum
  com IP continua ganhando.

**Aceite**: a aba Monitores do Servidor NetMonitor mostra apenas a coleta de
saúde; tentar criar um ping para ele pela API devolve erro de negócio; uma
instalação que já tinha o ping o perde no primeiro boot, com o registro no log.

### Fase 2 — Escopo da regra: pré-selecionado, não travado `🟢 Concluído`

- [x] No formulário compartilhado, `fixedDeviceId` deixa de **desabilitar** o
  seletor e passa a **restringir** as opções a duas: o dispositivo de origem e
  "Todos os dispositivos". Trocar o dono da regra por engano continua impedido —
  a lista não oferece os outros equipamentos —, mas a decisão legítima
  ("isto vale para o parque inteiro") deixa de exigir sair da tela.
- [x] Ajustar a dica do campo para descrever as duas escolhas, e não só a atual.
- [x] **Decidir o que a aba Regras do dispositivo mostra quando a regra é
  global.** Hoje ela filtra por `regra.deviceId === deviceId`: uma regra criada
  ali com escopo "todos" **desaparece da tela em que foi criada**, o que é
  indistinguível de a criação ter falhado. A aba passa a listar também as regras
  globais, marcadas como tal, com a contagem separando as duas origens.
- [x] Uma regra global listada na aba do dispositivo não pode sugerir que
  pertence a ele: o rótulo de escopo e a ação de exclusão precisam deixar claro
  que apagá-la afeta todo o inventário.
- [x] Quando as métricas oferecidas vierem restritas pelas capacidades do
  dispositivo (`availableFields`) e o operador escolher "Todos os dispositivos",
  a restrição deixa de valer — o vocabulário volta a ser o completo, porque o
  escopo deixou de ser aquele equipamento.
- [x] Cobrir por teste de convenção que o formulário continua sendo um só nas
  duas telas.

**Aceite**: em `/devices/{id}?tab=rules`, "Criar personalizada" abre o diálogo
com o dispositivo pré-selecionado e "Todos os dispositivos" disponível; criar
uma regra global dali a deixa visível na mesma aba, identificada como global.

### Fase 3 — Cards de saúde em duas colunas `🟢 Concluído`

- [x] Os cards do resumo de saúde passam a ocupar **metade da largura** a partir
  de `md`, duas por linha — CPU e memória lado a lado na primeira, o restante
  seguindo a mesma grade.
- [x] Manter `cols="12"` no celular: dois cards lado a lado num telefone
  espremeriam o sparkline até ele deixar de ser legível, que é a única coisa que
  o card tem de próprio.
- [x] Verificar que o alvo clicável do card (o diálogo de histórico) continua
  respondendo a mouse, teclado e toque depois da mudança de grade.

**Aceite**: na Visão Geral, CPU e memória ocupam metade da largura cada em telas
médias e maiores; nada quebra no celular.

### Fase 4 — A linha do monitor abre o detalhe, em todo lugar `🟢 Concluído`

- [x] Ligar `clickable` no `MonitorsTable` e abrir o diálogo de detalhe pelo
  `@click:row`. O `ResponsiveDataTable` já suporta isso no desktop e no mobile —
  não há componente novo a construir.
- [x] **Colocar `@click.stop` em toda ação de linha**: testar, editar, excluir e
  o interruptor de ativação, nas duas variantes (desktop e cartão mobile). Sem
  isso, cada clique num botão abriria o diálogo por baixo da ação.
- [x] **Remover o botão de gráfico** das duas variantes. Ele existia para abrir
  o detalhe; com a linha inteira fazendo isso, ele vira uma segunda porta para
  a mesma sala.
- [x] Decidir o que fica do link do nome. Ele mantém o `href` — abrir em nova
  aba e copiar o endereço continuam valendo —, mas deixa de ser o único alvo.
- [x] **Aplicar o mesmo comportamento ao widget "Monitores de Rede" do
  dashboard**, que não usa o `MonitorsTable`: é um `v-list` próprio. A linha
  inteira abre o mesmo diálogo.
- [x] **Converter as demais derivações** que hoje navegam para o monitor em vez
  de abri-lo: `UnstableTargetsWidget` faz `router.push` para o alvo instável.
- [x] Avaliar se o `v-list` do widget deveria passar a usar o `MonitorsTable`.
  Duas listas de monitor com regras de clique próprias é a origem exata deste
  item; se a fusão não couber, registrar por que — mas registrar.
- [x] Estender o teste de convenção existente para cobrir as derivações novas,
  não só a rota.

**Aceite**: clicar em qualquer ponto livre da linha de um monitor abre
"Detalhes do monitor", na lista de monitores, na aba do dispositivo e no
dashboard; clicar em testar, editar, excluir ou no interruptor executa **apenas**
a ação; não existe mais botão de gráfico na linha.

## 4. Matriz obrigatória de validação

```bash
# backend/
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release

# raiz do projeto
npm --prefix frontend run typecheck
npm --prefix frontend run format
npm --prefix frontend run lint
npm --prefix frontend run build
```

> `cargo test` regenera os bindings do `ts-rs`. Rode `npm --prefix frontend run
> format` **depois** dele, nunca antes.

Além da suíte automatizada:

- o Servidor NetMonitor não tem monitor de alcance, nem depois de um reinício;
- uma instalação que já tinha o ping do servidor o perde no primeiro boot, com
  registro no log;
- criar um dispositivo sem IP não gera monitor de alcance, e a tela diz por quê;
- criar um dispositivo com IP continua gerando o ping de sempre;
- o formulário aberto de dentro de um dispositivo oferece duas opções de escopo;
- uma regra global criada pela aba do dispositivo aparece nessa mesma aba,
  marcada como global;
- CPU e memória ocupam metade da largura cada em telas médias;
- a linha inteira do monitor abre o diálogo nas três telas que o listam;
- os quatro controles de linha continuam funcionando sem abrir o diálogo.

## 5. Fora de escopo e itens proibidos

- **Coletar qualquer métrica de saúde nova.** A Fase 3 é de layout: ela muda a
  largura dos cards que já existem. Uma série nova exigiria fonte de coleta,
  nome de série, campo de alerta e template próprios — é um roadmap inteiro, não
  um ajuste de grade.
- Criar um segundo componente de detalhe de monitor. A rota e os diálogos usam a
  mesma `MonitorDetailView`, e continuam usando.
- Criar um segundo formulário de regra, ou um caminho de criação que não passe
  pelo componente compartilhado.
- Esconder o dispositivo do sistema de qualquer listagem para evitar o problema
  do monitor de alcance. Ele é um dispositivo de primeira classe; o que muda é o
  que se pode criar para ele.
- Identificar o dispositivo do sistema por nome, posição na lista ou ID fixo. A
  resposta é `isSystem`, do backend.
- Resolver o clique da linha desligando os botões de ação, ou movendo-os para um
  menu suspenso só para evitar a sobreposição.
- Manter o botão de gráfico "por segurança" em algum dos pontos convertidos.

## 6. Achados fora do plano, e o que foi feito com eles

O levantamento da seção 2 foi feito contra o código; a implementação encontrou
mais seis coisas que o levantamento não tinha visto. Nenhuma virou dívida: a
regra do topo deste roadmap — "um comportamento vale para **todos** os pontos
que o exibem" — é o que decide cada uma delas.

| # | Achado | Onde | O que foi feito |
|---|---|---|---|
| 14 | Mais duas derivações navegavam para `/monitors/{id}` além do `UnstableTargetsWidget`: o gráfico de latência e a lista de túneis da VPN | `widgets/LatencyTimeSeriesWidget.vue`, `pages/vpn/VpnDevicesPage.vue` | Convertidas ao mesmo diálogo. O teste de convenção passou a proibir `name: 'monitor-detail'` em qualquer `.vue`, e não só dois `:to` literais |
| 15 | "Editar" na aba Regras do dispositivo levava o operador para a Central de Alertas — com o formulário compartilhado já montado ali, e um `abrirFormulario(regra)` que nunca era chamado | `components/devices/DeviceRulesTab.vue` | Passa a abrir o formulário compartilhado na própria aba. É a mesma regra da Fase 2: o escopo se decide sem sair da tela |
| 16 | Excluir uma regra pela aba do dispositivo não pedia confirmação nenhuma | `components/devices/DeviceRulesTab.vue` | Diálogo de confirmação, com aviso explícito quando a regra é global. É o que a Fase 2 pede ao dizer que apagar uma global "afeta todo o inventário" |
| 17 | O `ResponsiveDataTable` emitia `click:row` no desktop mesmo com `clickable` falso — a tabela de monitores só não reagia porque não escutava o evento | `components/ResponsiveDataTable.vue` | O emissor respeita `clickable`, e o cursor de linha clicável acompanha a prop. Sem isso, "ligar o clique" seria escutar um evento que já vazava |
| 18 | `sync_device_monitor` não tinha o que fazer com um ping que já existia num dispositivo que **perdeu** o alvo (IP apagado na edição) | `controllers/devices.rs` | O monitor é desativado em vez de seguir checando o nome. Reinformar o IP o reativa — coberto por teste |
| 19 | Um monitor de alcance removido do servidor deixaria para trás resultados, métricas, eventos e regras órfãos | `services/monitoring/reachability.rs` | A limpeza de boot passa pelo `ResourceCleanupService`, o mesmo caminho do `DELETE /api/monitors/{id}` |
| 20 | "Ver dispositivo", no cabeçalho do detalhe do monitor, era o único botão sem `color`: o `variant="tonal"` caía na cor de superfície e ele saía cinza ao lado de quatro irmãos coloridos — cinza é a linguagem de "desabilitado" | `components/monitors/MonitorDetailView.vue` | `color="primary"` explícito. Encontrado ao conferir o diálogo aberto pela linha, que é o caminho que a Fase 4 acabou de tornar o principal |

### A avaliação do `v-list` do widget — a decisão, e por quê

A Fase 4 pede para avaliar se o widget "Monitores de Rede" deveria passar a
usar o `MonitorsTable`, e para **registrar** o motivo caso a fusão não caiba.
Ela não coube, e o motivo é que os dois componentes respondem a perguntas
diferentes:

- o `MonitorsTable` é a superfície de **gestão** — cabeçalhos ordenáveis,
  coluna de ID, interruptor de ativação, edição e exclusão. Montá-lo no painel
  levaria o `MonitorFormDialog` junto (o `@edit` sobe como evento) e colocaria
  a exclusão de um monitor a um clique de distância dentro de um painel de
  leitura;
- o widget é um **resumo** dentro de uma célula de grade com altura fixa e
  rolagem própria.

Mas a duplicação que originou o item — "duas listas de monitor com regras de
clique próprias" — foi eliminada onde ela de fato estava: em **como se abre um
monitor**. Isso virou o composable `useMonitorDetail`, e hoje as cinco
superfícies que listam monitor (a tabela compartilhada, o widget do painel, o
ranking de alvos instáveis, o gráfico de latência e a lista de túneis) pegam
dele o estado do diálogo e a função de abrir. Um teste de convenção afirma
isso. Fundir os componentes teria trocado duas regras de clique por um
componente carregando responsabilidade que ele não tem; extrair a regra
resolveu o problema real.

## 7. Verificação visual, no aplicativo em execução

A matriz da seção 4 é automatizada; os aceites das quatro fases são visuais, e
foram conferidos com o binário de release servindo o `dist` do frontend, contra
um banco descartável (a instalação real do operador não foi tocada). O que foi
observado, nesta ordem:

- **Fase 1** — a aba Monitores do Servidor NetMonitor lista **só** "Saúde do
  sistema", e o aviso mostra o texto que veio do backend
  (`reachMonitorBlockedReason`). Os dois dispositivos com IP ganharam o ping de
  sempre; o cadastrado sem IP não ganhou nenhum.
- **Fase 2** — o seletor "Aplicar a", aberto de dentro do Roteador da Matriz,
  oferece exatamente duas opções: o próprio roteador e "Todos os dispositivos".
  Uma regra global criada ali aparece na mesma aba, com o chip "Todos os
  dispositivos", e a contagem passa de "10 globais" para "11 globais". O
  diálogo de exclusão avisa que apagá-la atinge o inventário inteiro.
- **Fase 3** — medido em quatro larguras: 390 px → **1** card por linha
  (326 px de largura, o sparkline de 220 px cabe); 700, 1000 e 1440 px →
  **2** por linha (306, 440 e 660 px).
- **Fase 4** — nenhum botão de gráfico restou; o cursor da linha é `pointer`;
  clicar num ponto livre abre "Detalhes do monitor" na lista de monitores, na
  aba do dispositivo e no widget do painel; "Editar" abre **só** o formulário
  de edição e o interruptor não abre diálogo nenhum. No celular, tocar no
  cartão abre o mesmo diálogo.
