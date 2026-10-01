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
- Embutidos (`openwrt-packages`, `linux-ssh-status`, `http-page-info`) não são
  editados: duplique para personalizar. Embutido que sai do binário (renomeado
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
