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
| `["device", "fleet"]` | os dois | Rede Wi-Fi: a rede toda e o ajuste de cada roteador |

- **Membros** são os equipamentos onde o plugin está instalado: "adicionar o
  roteador à rede" é instalar o plugin nele (pela aba Equipamentos do
  aplicativo ou pelo Catálogo do dispositivo). O **acesso** (SSH/HTTP) de cada
  um continua no cadastro do próprio dispositivo, em Plugins → Credenciais.
- **O sistema é a fonte da verdade.** O plugin guarda o estado desejado em
  `settings` (`plugin_settings`): `fleet` vale para todos, `device` é o ajuste
  de um equipamento e sobrepõe o geral. Salvar a configuração não toca nos
  equipamentos; o script compara o desejado com o que lê e mostra a diferença.
  Campo `secret` é gravado cifrado e volta como `********` (deixar assim mantém
  o valor). A tela é gerada do esquema; `order` fixa a ordem dos campos, porque
  nem o JSON nem o `jsonb` do PostgreSQL guardam a ordem das chaves.
- **Títulos**: a saída do script usa chaves estáveis (`clients`, `state`); a
  ação declara em `labels` o título de cada uma ("Clientes", "Estado") para as
  tabelas, os relatórios e a grade.
- **Visão geral**: a ação de estado (`statusAction`) roda em todos e a grade
  (`matrix`) cruza equipamentos × itens (roteadores × SSIDs) com o estado de
  cada célula — `sincronizado`, `pendente`, `ausente`…
- **Ações de frota** viram um **lote** (`plugin_batches`): a mesma ação de
  dispositivo em cada membro escolhido, até 4 ao mesmo tempo, cada uma com a
  sua execução auditada e as mesmas portas da execução avulsa (efeito,
  aprovação, confirmação de escrita, máscara). O andamento chega pelo SSE
  (`plugin:batch_updated`). Cancelar pula quem ainda não começou e interrompe
  as execuções em andamento.
- **Consolidado** (`reduce`): função pura do script que recebe o resultado de
  todos e devolve um relatório. Pode sugerir ajustes por equipamento
  (`settings_patch`); **Aceitar sugestão** grava esses ajustes na configuração
  — nada muda nos equipamentos até a configuração ser aplicada.
- Um plugin reaproveita outro com `uses` + `device.use_plugin` (o Wi-Fi usa o
  Gerenciador de pacotes para trocar o `wpad` e instalar o `usteer`).

## Rede Wi-Fi (OpenWrt)

`openwrt-wifi` gerencia o Wi-Fi de vários OpenWrt juntos ou separados, por SSH
e UCI.

- **Multi-SSID**: cada rede escolhe bandas (2,4 / 5 / 6 GHz), criptografia
  (WPA2/WPA3/misto/aberta), senha, interface de rede, oculta, isolamento de
  clientes. Um roteador pode **desligar** redes da frota nos ajustes dele.
- **Roaming** por rede: 802.11r (mobility domain derivado do SSID, igual em
  todos os roteadores), 802.11k e 802.11v. **Preparar roaming/mesh** troca o
  `wpad-basic` pelo `wpad-mbedtls` (pelo Gerenciador de pacotes) e instala o
  **usteer** quando ligado.
- **Mesh 802.11s** entre os roteadores, numa banda, com senha SAE.
- **Canais**: canal, largura e potência por rádio nos ajustes de cada roteador.
  **Planejar canais** varre os vizinhos de cada roteador (`iwinfo scan`) e
  sugere o canal menos disputado sem repetir entre os vizinhos da frota
  (2,4 GHz: 1/6/11; 5 GHz: 36/44/149/157); aceitar grava a sugestão nos ajustes.
- **Aplicação segura**: **Pré-visualizar** mostra o que cada roteador ganha,
  muda e perde. **Aplicar** guarda uma cópia (`/tmp/netmonitor-wireless.bak`),
  grava tudo de uma vez, faz `wifi reload`, confere os rádios e **restaura a
  cópia** se algum rádio que estava no ar caiu.
- O plugin só mexe nas seções que ele criou (`nm_…`); redes que já existiam no
  roteador aparecem como "não gerenciadas" e ficam como estão.

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
