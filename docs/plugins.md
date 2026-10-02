# Plugins de dispositivo

Plugins agem **sobre** o equipamento — por SSH, pela interface web (HTTP/HTTPS)
ou por Telnet: instalar um pacote num OpenWrt, ler a tabela de uma página de
status, alterar um SSID. Decisão de arquitetura: [ADR 012](adr/012-plugins-de-dispositivo.md).

## Onde ficam

- **Uma aba por plugin instalado** em `/devices/{id}`, depois das abas nativas
  (separada por um divisor e destacada em `secondary` quando selecionada): a
  tela do plugin (ver "Tela própria") ou a lista de ações, mais validação,
  código, exportação e "Desinstalar". O título e o ícone vêm do `panel`; o
  `?tab=plugin-<id>` abre direto nela.
- **Aba Plugins** só para gerenciar: **Catálogo** (instalar — não toca o
  equipamento, é escolha de cadastro em `device_plugin_installs`; exclusivo já
  nasce instalado), **Credenciais** e **Histórico** (cada execução com cada
  acesso registrado).
- Ação pela tela só roda com o plugin instalado no equipamento; a IA e a
  validação funcional não exigem a instalação. Leitura sem parâmetro roda
  direto e abre o resultado; escrita sempre passa pela confirmação.
- **Aplicativos** (`/apps/{id}`): plugins que trabalham com vários
  equipamentos de uma vez — no menu **Aplicativos** e no topo de `/plugins`.
  Ver "Aplicativos".
- **Biblioteca** em `/plugins` (menu de administração): todos os plugins, com
  status, origem, risco da revisão e em quantos equipamentos já foram validados.

## Compatibilidade

Um plugin de **modelo** serve a todo equipamento que casa com o `match` do
manifesto (sistema, fabricante, modelo, firmware). Um **exclusivo** serve só ao
seu equipamento. A aba mostra:

| Selo | Significado |
|---|---|
| Validado | já passou nos testes funcionais num equipamento igual (sistema, modelo, firmware) |
| Compatível | as regras do manifesto casam com o que se sabe |
| Talvez compatível | falta evidência — rode **Detectar** para ler o firmware |
| Incompatível | alguma regra contradiz o equipamento |

**O sistema decide, não o hardware.** O fabricante e o modelo do cadastro
descrevem a placa — uma RouterBOARD da MikroTik pode rodar OpenWrt. Por isso o
sistema do equipamento vem, nesta ordem:

1. da **declaração** no cadastro;
2. do que o **equipamento mostrou** (`devices.observed_os`): a identificação
   do servidor SSH (`dropbear` = OpenWrt, lida na porta da credencial SSH), o
   SNMP, ou um plugin de um sistema só cujo **Detectar** leu o sistema;
3. do **Laya**, quando nada disso decidiu — palpite, com a confiança;
4. do fabricante/modelo do cadastro — palpite.

Só a declaração e o que o equipamento mostrou podem tornar um plugin
**Incompatível**; palpite (Laya, cadastro) que discorda vira **Talvez**, com o
porquê. A observação acontece sozinha depois do cadastro (em segundo plano;
o menu Aplicativos é avisado pelo SSE se algum plugin ligou) e no botão
**Verificar sistema** do aplicativo, que também roda o **Detectar** do plugin
quando há acesso cadastrado — é isso que lê o firmware e tira da dúvida. A
lista de dispositivos mostra o mesmo sistema, com a origem.

## Ciclo de vida

```
importado ─► Quarentena ─(revisão aceita)─► Rascunho ─(testes)─► Testado ─(Ativar)─► Ativo
criado / IA ─────────────────────────────► Rascunho
```

- **Rascunho e Testado** executam, mas **cada acesso** ao equipamento pede
  aprovação (diálogo global, em qualquer tela).
- **Ativo** executa direto; ação que altera o equipamento pede confirmação
  explícita antes de começar.
- Editar o código zera os testes. Editar um plugin **importado** o devolve à
  quarentena.
- Embutidos (`openwrt-packages`, `openwrt-wifi`, `linux-ssh-status`,
  `http-page-info`) não são editados: duplique para personalizar. Embutido que sai do binário (renomeado
  ou aposentado) sai do catálogo no próximo boot.
- **Embutidos nascem desligados.** Ligam sozinhos quando um equipamento
  compatível é cadastrado (ou tem o sistema trocado): o plugin precisa citar o
  sistema dele em `match.platforms` — plugin sem regra de sistema (uma página
  HTTP qualquer) só liga à mão. No Catálogo do equipamento, o desligado
  compatível aparece com **Ativar e instalar**. Desligar à mão é definitivo: o
  plugin não volta sozinho no próximo cadastro (`plugins.auto_enable`). O
  **Assistente de Configuração Inicial** tem a etapa **Aplicativos** para ligar
  os de frota de uma vez. Na atualização, embutido que nenhum equipamento usa
  passa a desligado; o que já está instalado em algum continua ligado.

## Tela própria (`panel`)

O manifesto pode declarar a tela do plugin instalado — nenhum código do plugin
roda no navegador:

- `listAction`: ação de leitura com `output: table` que preenche a lista;
  `searchParam`, o parâmetro dela que recebe a busca; `keyColumn`, a coluna que
  identifica a linha;
- `toolbar`: ações sem parâmetro, como botões ("Atualizar lista de pacotes");
- `rowActions`: ações sobre a linha escolhida, com o mapa parâmetro → coluna e
  `showWhen`/`hideWhen` por coluna booleana ("Instalar" some se `installed`).

A lista carrega sozinha ao abrir quando o plugin está ativo; o resultado e a
resposta de cada ação chegam pelo SSE. Ação de linha que termina bem recarrega
a lista uma vez.

## Gerenciador de pacotes (OpenWrt)

`openwrt-packages`: lista os instalados, busca nos disponíveis, instala e
remove por SSH, devolvendo o que o roteador respondeu.

| OpenWrt | Gerenciador | Instalar | Remover | Listar instalados |
|---|---|---|---|---|
| 24.10 e anteriores | `opkg` | `opkg install` | `opkg remove` | `opkg list-installed` |
| 25.x em diante | `apk` | `apk add` | `apk del` | `apk list --installed` |

A versão vem do **Detectar versão** (`/etc/openwrt_release`); sem versão
numérica (SNAPSHOT), vale o binário presente. Instalar é idempotente, roda
`update` antes e confirma o pacote na lista depois. É o caminho da IA para
instalar programas num OpenWrt.

## Aplicativos (vários equipamentos)

O manifesto diz onde o plugin aparece (`surfaces`):

| `surfaces` | Onde aparece | Exemplo |
|---|---|---|
| `["device"]` (padrão) | aba no equipamento | Gerenciador de pacotes |
| `["fleet"]` | só em Aplicativos | um relatório de toda a rede |
| `["device", "fleet"]` | os dois | Rede Wi-Fi: todos os roteadores, ou um só na aba dele |

- **Membros** são os equipamentos onde o plugin está instalado: "adicionar o
  roteador à rede" é instalar o plugin nele (pela aba Equipamentos do
  aplicativo ou pelo Catálogo do dispositivo). O **acesso** (SSH/HTTP) de cada
  um continua no cadastro do próprio dispositivo, em Plugins → Credenciais.
- **Quem é a fonte da verdade** é escolha do plugin:
  - **o equipamento** (o Wi-Fi): nada fica guardado; a visão geral lê os
    equipamentos e cada ação parte do que eles têm na hora. Editar é uma ação
    com parâmetros, pré-visualizada e aplicada nos equipamentos escolhidos;
  - **o sistema**: o plugin guarda o estado desejado em `settings`
    (`plugin_settings`; `fleet` vale para todos, `device` sobrepõe num
    equipamento), e o script compara com o que lê. Campo `secret` é gravado
    cifrado e volta como `********`.
- Formulários (parâmetros e `settings`) são gerados do esquema; `order` fixa a
  ordem dos campos, porque nem o JSON nem o `jsonb` do PostgreSQL guardam a
  ordem das chaves. **Parâmetro `secret`** chega ao script em claro, mas a
  execução e o lote guardam `********` e o runtime o mascara na saída e no
  transcript.
- **Títulos**: a saída do script usa chaves estáveis (`clients`, `state`); a
  ação declara em `labels` o título de cada uma ("Clientes", "Estado") para as
  tabelas, os relatórios e a grade.
- **Visão geral**: a ação de estado (`statusAction`) roda em todos e a grade
  (`matrix`) cruza equipamentos × itens (roteadores × SSIDs) com o estado de
  cada célula. `matrix.edit`/`matrix.remove` põem um menu no título da coluna:
  a ação de frota abre com o formulário preenchido pelo item (mapa parâmetro →
  campo) e só os equipamentos que o têm marcados.
- **Pré-visualizar**: ação de frota com `preview` (uma ação de leitura com os
  mesmos parâmetros) ganha o botão no diálogo; o resultado de cada equipamento
  aparece ali, e mudar o pedido apaga a pré-visualização.
- **O formulário parte do que os equipamentos têm** (a última leitura da ação
  de estado; o aplicativo lê sozinho ao abrir quando a leitura tem mais de 10
  minutos, e o diálogo tem **Ler de novo**):
  - parâmetro com `"source": "lista.chave"` (`"networks.ssid"`) vira uma lista
    com os valores dos equipamentos; escolher um preenche o formulário pelo
    mapa de `matrix.edit`/`remove` e marca só os equipamentos que o têm;
  - ação de frota com `"current": "_current.radios"` mostra **Como está agora**
    (cada equipamento, nas colunas do formulário); com um equipamento só
    marcado, o formulário vem com os valores dele, e só com os campos que os
    equipamentos têm. Com vários, fica em branco — vazio mantém — para o valor
    de um não ir para os outros;
  - chave que começa com `_` na saída é dado para a tela e não aparece nos
    relatórios.
- **Andamento ao vivo**: cada equipamento do lote mostra o acesso que está
  rodando agora (`plugin:run_step`, antes da resposta) e o último que
  respondeu, com o tempo; quando todos respondem, "consolidando o resultado".
- **Ações de frota** viram um **lote** (`plugin_batches`): a mesma ação de
  dispositivo em cada membro escolhido, até 4 ao mesmo tempo, cada uma com a
  sua execução auditada e as mesmas portas da execução avulsa (efeito,
  aprovação, confirmação de escrita, máscara). O andamento chega pelo SSE
  (`plugin:batch_updated`). Cancelar pula quem ainda não começou e interrompe
  as execuções em andamento.
- **Consolidado** (`reduce`): função pura do script que recebe o resultado de
  todos e devolve um relatório. Pode sugerir:
  - `next: { action, title, devices: { "<id>": params } }` — rodar uma ação de
    frota com os parâmetros de cada equipamento (o lote aceita `deviceParams`
    por cima dos comuns). **Aceitar** pede confirmação e roda;
  - `settings_patch` (plugin com `settings`) — grava ajustes na configuração;
    nada muda nos equipamentos até ela ser aplicada.
- Um plugin reaproveita outro com `uses` + `device.use_plugin` (o Wi-Fi usa o
  Gerenciador de pacotes para trocar o `wpad` e instalar o `usteer`).

## Rede Wi-Fi (OpenWrt)

`openwrt-wifi` gerencia o Wi-Fi de vários OpenWrt juntos ou separados, por SSH
e UCI. **O roteador é a fonte da verdade**: nada fica guardado no NetMonitor;
a visão geral lê as redes de cada roteador e cada ação parte do que ele tem na
hora — mudança feita à mão aparece na próxima leitura.

- **Redes (multi-SSID)**: a grade mostra cada SSID em cada roteador (ativa ou
  desativada, clientes). O menu no nome da rede edita ou remove — o formulário
  vem preenchido com o que o roteador tem e só os roteadores que têm a rede
  ficam marcados. **Adicionar ou alterar rede** garante a rede nas bandas
  marcadas (2,4 / 5 / 6 GHz), com segurança (WPA2/WPA3/misto/aberta),
  interface, oculta, isolamento e ativa; **Nome atual** renomeia. Vale para
  qualquer rede do roteador, inclusive as criadas à mão; opções que o plugin
  não conhece ficam como estão.
- **Senha**: vazia mantém a do roteador. As senhas do Wi-Fi **não saem do
  roteador** — a leitura vem com elas mascaradas (`sed` no próprio roteador) e,
  ao levar uma rede para outra banda, a senha é copiada lá dentro
  (`$(uci -q get …)`). Só a senha nova digitada passa pela central, como
  parâmetro secreto.
- **Roaming** por rede: 802.11r (mobility domain derivado do SSID, igual em
  todos os roteadores), 802.11k e 802.11v. **Preparar roaming e usteer** troca
  o `wpad-basic` pelo `wpad-mbedtls` (pelo Gerenciador de pacotes) e instala o
  **usteer** se marcado.
- **Mesh 802.11s** entre os roteadores, numa banda, com senha SAE.
- **Rádios e canais**: canal, largura, potência, ligar/desligar e país; campo
  vazio mantém. **Planejar canais** varre os vizinhos de cada roteador
  (`iwinfo scan`) e sugere o canal menos disputado sem repetir entre os
  roteadores (2,4 GHz: 1/6/11; 5 GHz: 36/44/149/157); aceitar roda **Rádios e
  canais** com o canal de cada um.
- **Acesso por SSH** (e não ubus por HTTP). Medido num OpenWrt: a leitura de
  estado são 2 comandos, ~0,55 s por roteador, numa sessão SSH só (45–120 ms
  por comando). O ubus por HTTP responde em 5–50 ms por chamada, mas depende
  do `uhttpd-mod-ubus` e das ACLs do `rpcd` (no teste, recusou `uci` e
  `network.wireless`) e não acelera o que demora — a varredura de rádio
  (`iwinfo scan`), que agora roda em todos os rádios ao mesmo tempo.
- **Aplicação segura**: toda alteração tem pré-visualização com os comandos
  `uci`. Aplicar guarda uma cópia (`/tmp/netmonitor-wireless.bak`), grava tudo
  de uma vez, faz `wifi reload`, confere que o pedido ficou gravado e que os
  rádios voltaram, e **restaura a cópia** se não.

## Importação e revisão de segurança

Todo pacote de fora entra em **quarentena** e passa por:

1. **Análise estática** (sempre): regravar flash, restaurar padrão de fábrica,
   apagar a raiz, baixar e executar código, trocar senha/chave, mexer em
   SSH/web/firewall/LAN de gerência, derrubar interface, reiniciar, ação de
   leitura que escreve, parâmetro sem `pattern` concatenado em comando,
   conteúdo ofuscado.
2. **Revisão pela IA** configurada: o script faz o que o uso diz? faz algo não
   declarado?

Risco **crítico** bloqueia a instalação. **Alto** — ou revisão sem IA — exige
marcar "Revisei os riscos".

## Credenciais

Uma por tipo (SSH, HTTP, Telnet) em cada equipamento:

- **Guardar cifrada** — cifrada com `ENCRYPTION_KEY`; nunca volta para a tela,
  para a IA ou para o script.
- **Pedir a cada sessão** — nada é gravado; a senha informada vale 30 minutos.

Equipamento em outra rede: escolha **Acessar a partir de → agente remoto**. O
agente daquele site precisa de `device_io` no `AGENT_ALLOW`; `AGENT_DEVICE_CIDRS`
(opcional) restringe as redes. IP público nunca é alvo a partir do agente.

## A IA

No chat, o grupo de ferramentas `devices` permite à IA reconhecer o equipamento,
explorá-lo por SSH/HTTP, escrever, testar, validar e executar plugins. **Cada
acesso** aparece como pedido de confirmação com o comando exato, o efeito e o
motivo, com o aviso de que a IA pode alucinar.

**Aceitar automaticamente** (faixa no topo do chat de um equipamento): executa
sem perguntar, só naquela conversa e naquele equipamento, por até 2 horas,
depois de aceitar o termo de ciência. A faixa vermelha fica visível enquanto
estiver ligado, e **Parar** desliga o modo, interrompe a resposta e cancela o
que estiver rodando. Tudo continua registrado com o motivo.

Operar plugins (IA ou tela) é permitido só a **administradores**.

## Formato do pacote

O `.nmplugin.json` exportado leva manifesto, script Rhai, uso, lista de
compatibilidade e testes. O guia completo — API do script, testes e boas
práticas — é a skill de autoria em
[`backend/src/services/ai/knowledge/plugins.md`](../backend/src/services/ai/knowledge/plugins.md).
