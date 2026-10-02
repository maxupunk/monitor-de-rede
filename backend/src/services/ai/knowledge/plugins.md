# Skill: como criar plugins de dispositivo

Um **plugin de dispositivo** é um "driver" que age sobre um equipamento
(roteador, switch, servidor) por **SSH**, **HTTP** (interface web, porta 80/443)
ou **Telnet**. Ele vale para todos os equipamentos do mesmo modelo/versão
(`scope: model`) ou só para um (`exclusivo`, gravado com `exclusive_device`).

O script roda num **sandbox Rhai** na central. Ele não enxerga disco, processo
nem rede: a única saída é o objeto `device`, que só alcança o IP daquele
equipamento. **Você nunca vê senha**: use `{{username}}` e `{{password}}` onde
precisar, e o sistema troca no último instante.

## Instalar programa no OpenWrt: use o Gerenciador de pacotes

Para instalar, remover ou procurar um programa num OpenWrt, **não escreva
plugin nem rode `opkg`/`apk` à mão**: use o plugin embutido
`openwrt-packages` ("Gerenciador de pacotes") com `run_plugin_action`:

- `list_packages` (`query`: parte do nome) — busca nos disponíveis;
- `install_package` / `remove_package` (`name`: nome exato);
- `update_index` — se a busca vier vazia.

Ele escolhe o gerenciador pela versão: **OpenWrt 24.10 e anteriores usam
`opkg`; 25.x em diante usam `apk`** (sem versão, o binário que existir).

## Você pode alucinar — e o equipamento é real

- Nunca invente comando, caminho, parâmetro ou formato de saída. **Descubra** no
  equipamento, com comandos de leitura, e só então escreva o script.
- Cada acesso seu passa pelo usuário (ou pelo modo automático, que ele ligou
  ciente do risco). Declare sempre o `reason` — curto e verdadeiro — e o
  `effect` correto (`read` ou `write`).
- Explore **só com leitura**. Comando de escrita só quando o usuário pediu
  aquela alteração, e depois de ler o estado atual.
- Na dúvida entre dois comandos, pergunte ao usuário (`ask_user`) em vez de
  tentar os dois.

## Fluxo obrigatório

1. **Reaproveite** (DRY): `list_device_plugins` no equipamento. Se já existe
   plugin compatível que faz o pedido, use-o (`run_plugin_action`). Se existe um
   parecido, leia com `get_plugin` e proponha uma versão nova dele em vez de
   criar outro.
2. **Reconheça** o equipamento: `fingerprint_device` (portas, banner SSH, título
   da página web).
3. **Explore com leitura**: `device_ssh_exec` / `device_http_request` com
   `effect: read`. Anote as respostas reais — elas viram as fixtures dos testes.
4. Escreva **`detect`** primeiro (obrigatória, só leitura): devolva um mapa com
   `firmware` e, se souber, `model`. É ela que prova a compatibilidade.
5. Escreva as ações de leitura, depois as de escrita.
6. Escreva os **testes unitários** com as saídas reais do passo 3 e os
   **testes funcionais** das ações de leitura.
7. `save_plugin_draft` — o sistema já roda os testes e devolve os problemas.
   Corrija e salve de novo até passar.
8. Ofereça ao usuário `validate_plugin` (roda os testes funcionais no
   equipamento e registra a compatibilidade) e rodar a ação pedida
   (`run_plugin_action`). **Ativar** o plugin é decisão do usuário, na tela.

## O pacote

```json
{
  "format": 1,
  "manifest": {
    "slug": "openwrt-ssid",
    "name": "OpenWrt – SSID",
    "version": "1.0.0",
    "description": "Lê e altera o SSID do Wi-Fi via UCI.",
    "transports": ["ssh"],
    "match": { "platforms": ["openwrt"], "firmware": ">=21.02" },
    "actions": [
      { "id": "detect", "title": "Detectar versão", "effect": "read", "output": "kv" },
      { "id": "list_wifi", "title": "Listar redes Wi-Fi", "effect": "read", "output": "table" },
      { "id": "set_ssid", "title": "Alterar SSID", "effect": "write", "output": "text",
        "params": { "type": "object",
          "properties": {
            "iface": { "type": "string", "pattern": "[a-z0-9_]+", "title": "Seção UCI" },
            "ssid":  { "type": "string", "pattern": "[A-Za-z0-9 ._-]{1,32}", "title": "Novo SSID" }
          },
          "required": ["iface", "ssid"] } }
    ]
  },
  "script": "fn detect(device, params) { ... }",
  "usage": "# OpenWrt – SSID\n## Detectar versão\n...\n## Listar redes Wi-Fi\n...\n## Alterar SSID\n...",
  "tests": {
    "unit": [ { "action": "detect", "fixtures": [ { "ssh": "cat /etc/openwrt_release", "stdout": "DISTRIB_RELEASE='23.05.2'\n" } ],
                "expect": { "firmware": "23.05.2" } } ],
    "functional": [ { "action": "detect", "expectKeys": ["firmware"] } ]
  }
}
```

Regras que o sistema confere (e recusa se faltar):

- `slug`: minúsculas, números e hífen; `version`: `1.2.3`. Mudou o código de um
  plugin publicado? Suba a versão.
- `transports`: só os que o script usa. Chamada por transporte não declarado é
  recusada.
- `match`: `platforms` (ids do catálogo: `openwrt`, `routeros`, `ubiquiti`,
  `linux`, `windows`, `embedded`, `other`), `vendorRegex`, `modelRegex`,
  `firmware` (`">=21.02, <24"`, `"23.*"`) e `httpFingerprint` (regex no título
  ou no cabeçalho `Server`).
- `actions`: `id` = nome da função no script (`fn id(device, params)`),
  `effect` `read`/`write`, `output` `text` | `table` (lista de mapas) | `kv`
  (mapa) | `json` | `report` (mapa com seções: valores simples viram cartões,
  listas de mapas viram tabelas com título, listas de texto viram bloco de
  comandos; chaves `state`/`status`/`change` viram selo colorido). `safeToRetest: true` só em escrita idempotente que pode
  entrar no teste funcional.
- `params`: subconjunto de JSON Schema — `type: object` e propriedades
  `string`/`integer`/`number`/`boolean` com `pattern`, `enum`, `minLength`,
  `maxLength`, `minimum`, `maximum`, `default`, `title`, `description`.
  **Todo texto que vai para um comando precisa de `pattern` ou `enum`** (o
  `pattern` é ancorado). `"order": ["campo", …]` no objeto define a ordem dos
  campos na tela — sem ele a ordem é alfabética (o JSON não guarda a ordem das
  chaves).
- `labels` (opcional, por ação): títulos das chaves da saída na tela,
  `{ "clients": "Clientes", "pending_changes": "Alterações pendentes" }`. As
  chaves continuam em inglês/snake_case (são o contrato com testes e
  `matrix`); o operador lê os títulos. Use uma chave por significado — a
  mesma chave com sentidos diferentes na mesma saída não tem título que sirva.
- `usage`: markdown com uma seção por ação (use o `title`), dizendo o que faz,
  o que altera e os cuidados. É o que o operador lê antes de executar.
- Toda ação tem ao menos um teste unitário; toda ação de leitura, um funcional.

## A API do script

Cada ação é `fn <id>(device, params)` e devolve o resultado (mapa, lista,
texto). Erro: `throw "mensagem"`.

| Chamada | Devolve |
|---|---|
| `device.run(cmd)` | stdout (texto); **lança erro** se o código de saída ≠ 0 |
| `device.ssh(cmd)` | `#{ stdout, stderr, exit }` — não lança |
| `device.telnet(cmd)` | saída (texto) |
| `device.get(path)` | `#{ status, headers, body }` |
| `device.post_form(path, #{ campo: valor })` | idem |
| `device.post_json(path, valor)` | idem |
| `device.http(#{ method, path, headers, body, form, json, login, https, basic_auth, timeout_ms })` | idem |
| `device.info` | `#{ id, name, ip, vendor, model, platform, firmware }` |
| `device.settings` | `#{ fleet, device }` — a configuração guardada (ver "Aplicativos") |
| `device.use_plugin(slug, ação, #{...})` | o resultado da ação de outro plugin (ver "Reaproveitar outro plugin") |
| `device.log(texto)` / `print(texto)` | registra no transcript |

HTTP: `path` é **relativo** (`/cgi-bin/luci`); URL absoluta é recusada. Não há
redirecionamento automático (você vê o `302` e o `location`). **Cookies**
(`Set-Cookie`) são guardados e reenviados sozinhos. `headers` volta com nomes
em minúsculas. `login: true` marca o POST de autenticação como leitura — use só
nele. `basic_auth: true` manda a credencial HTTP como Basic Auth.

Utilitários: `trimmed(s)`, `lines(s)` (linhas não vazias, aparadas),
`words(s)`, `parse_kv(s, "=")` (tira aspas), `regex_match(s, re)`,
`regex_capture(s, re)` (1º grupo ou `()`), `regex_captures(s, re)` (lista),
`regex_groups(s, re)` (todos os grupos do 1º casamento, ou `()`),
`join_with(lista, sep)`, `hash_hex(s, n)` (n primeiros hex do SHA-256: um id
estável derivado de um texto, igual em todo equipamento),
`json_parse(s)`, `json_encode(v)`, `shell_quote(s)`, `url_encode(s)`.

### Pegadinhas do Rhai

- `s.trim()` altera a string **no lugar** e devolve `()`. Use `trimmed(s)`.
- Palavras reservadas (`package`, `import`, `export`, `as`, `private`, `call`,
  `fn`, `this`, `global`, `match`, `new`, `static`…)
  não podem ser chave de mapa sem aspas (`#{ "package": nome }`) nem `id` de
  ação.
- Chave ausente num mapa devolve `()`; teste com `if x == () { ... }`.
- Concatenação é `+`; `s.split(" - ")` devolve lista; `for i in 0..lista.len()`.
- Fixtures repetidas para o mesmo comando respondem em sequência (a última se
  repete): é assim que se testa o "antes" e o "depois" de uma alteração.
  `"firmware": "24.10.0"` no teste simula a versão do equipamento.
- Funções não enxergam `const` nem variáveis de fora: use literais dentro da
  função.
- Funções não enxergam variáveis de fora: passe `device` adiante
  (`fn ler(device) { ... }`) — isso também é como você reaproveita código entre
  ações (DRY).

## Efeito: leitura × escrita

O sistema classifica cada chamada. Ação `read` que tenta um comando de escrita
(`uci set/commit`, `opkg install`, `rm`, `reboot`, redirecionamento `>`,
`/ip ... set` do RouterOS, HTTP `POST/PUT/DELETE` sem `login`) é **recusada**.
Se a ação altera algo, declare `effect: write` — o operador confirma antes.

## Boas práticas (obrigatórias)

- **Idempotência**: leia o estado antes; se já está como pedido, não altere e
  diga isso no resultado.
- **Parâmetros**: sempre `pattern`/`enum`, e `shell_quote(params.x)` ao montar
  comando.
- **OpenWrt/UCI**: altere com `uci set`, confira com `uci changes`, aplique com
  `uci commit <config>` e recarregue só o serviço afetado. Em falha, `uci revert`.
- **Nada destrutivo** sem pedido explícito do usuário: `sysupgrade`,
  `firstboot`, `mtd`, `reboot`, troca de senha, firewall, IP da LAN de gerência.
  A revisão de segurança marca isso como crítico/alto.
- **Pequeno e focado** (responsabilidade única): uma ação faz uma coisa; um
  plugin cobre uma família de equipamento. Funções auxiliares evitam repetição.
- **Saída estruturada**: `table` para listas, `kv` para estado, `text` para o
  que o equipamento respondeu.

## Testes

Unitário = o script de verdade contra respostas gravadas:

```json
{ "name": "altera o SSID", "action": "set_ssid", "params": { "iface": "default_radio0", "ssid": "Loja" },
  "fixtures": [
    { "ssh": "uci get wireless.'default_radio0'.ssid", "stdout": "OpenWrt\n" },
    { "ssh": "uci set wireless.'default_radio0'.ssid='Loja'", "stdout": "" },
    { "ssh": "uci commit wireless", "stdout": "" },
    { "ssh": "wifi reload", "stdout": "" }
  ],
  "expectContains": "Loja" }
```

- A chave (`ssh`/`telnet`/`http` = `"GET /caminho"`) casa com o comando exato;
  `"regex": true` para padrão. Chamada sem fixture **reprova** o teste.
- HTTP: `{ "http": "POST /login", "status": 302, "headers": { "Set-Cookie": "sid=abc" }, "body": "" }`.
- Asserções: `expect` (subconjunto do resultado), `expectContains` (texto) ou
  `expectError` (a ação deve falhar — ótimo para provar que um parâmetro
  malicioso é recusado).
- Funcional (no equipamento real, só leitura): `expectKeys`, `expectMinRows`,
  `expectContains`.

## Exemplo: roteador com interface web e login por formulário

```rhai
fn login(device) {
    let r = device.http(#{ method: "POST", path: "/cgi-bin/luci", login: true,
                           form: #{ luci_username: "{{username}}", luci_password: "{{password}}" } });
    if r.status != 302 && r.status != 200 { throw "login recusado (HTTP " + r.status + ")"; }
}

fn detect(device, params) {
    login(device);
    let page = device.get("/cgi-bin/luci/admin/status/overview");
    #{ firmware: regex_capture(page.body, "OpenWrt ([0-9.]+)"), model: regex_capture(page.body, "Model</[^>]+>\\s*<[^>]+>([^<]+)") }
}
```

Os plugins embutidos (`openwrt-packages`, `openwrt-wifi`, `linux-ssh-status`,
`http-page-info`) são exemplos completos: leia-os com `get_plugin`. O
`openwrt-wifi` é o modelo de aplicativo de frota (configuração, `use_plugin`,
`reduce`, `report`).

## Tela própria (`panel`)

Plugin instalado num equipamento ganha uma aba. Sem `panel`, ela lista as
ações. Com `panel`, a aba vira uma tela: uma lista vinda de uma ação de leitura
com `output: table`, busca opcional, botões de barra e ações sobre a linha
escolhida — tudo declarado, nenhum código roda no navegador.

```json
"panel": {
  "title": "Gerenciador de pacotes", "icon": "mdi-package-variant-closed",
  "listAction": "list_packages", "searchParam": "query", "keyColumn": "name",
  "toolbar": ["update_index"],
  "rowActions": [
    { "action": "install_package", "label": "Instalar pacote", "icon": "mdi-download",
      "params": { "name": "name" }, "hideWhen": "installed" }
  ]
}
```

- `listAction`: só pode exigir o parâmetro de busca (`searchParam`).
- `toolbar`: ações sem parâmetro obrigatório.
- `rowActions[].params`: parâmetro da ação → coluna da linha; todo parâmetro
  obrigatório precisa vir da linha. `showWhen`/`hideWhen`: coluna booleana.

## Reaproveitar outro plugin (`uses`)

Não reescreva o que outro plugin já faz. Declare-o e chame a ação dele no mesmo
equipamento:

```json
"uses": ["openwrt-packages"]
```

```rhai
let base = device.use_plugin("openwrt-packages", "detect", #{});
device.use_plugin("openwrt-packages", "install_package", #{ name: "usteer" });
```

- Só plugins declarados em `uses` e ativos; os parâmetros passam pela validação
  da ação chamada.
- O efeito da ação chamada não pode passar o da sua: ação `read` não chama
  `install_package`.
- Os acessos entram no mesmo transcript. Nos testes unitários, as fixtures
  cobrem também os comandos do plugin chamado.

## Aplicativos: plugins de vários equipamentos (frota)

Um plugin pode ter tela no equipamento, tela de frota (menu **Aplicativos**) ou
as duas. A frota são os equipamentos onde o plugin está instalado; o acesso de
cada um continua no cadastro do dispositivo.

```json
"surfaces": ["device", "fleet"],
"fleet": {
  "title": "Rede Wi-Fi", "icon": "mdi-wifi-cog", "statusAction": "status",
  "matrix": { "field": "networks", "key": "ssid", "state": "state", "detail": "clients",
              "edit":   { "action": "network", "params": { "ssid": "ssid", "original_ssid": "ssid" } },
              "remove": { "action": "remove_network", "params": { "ssid": "ssid" } } },
  "actions": [
    { "id": "network", "title": "Adicionar ou alterar rede", "action": "set_network",
      "preview": "preview_network" },
    { "id": "remove_network", "title": "Remover rede", "action": "remove_network",
      "preview": "preview_remove" },
    { "id": "plan_channels", "title": "Planejar canais", "action": "scan", "reduce": "plan_channels" }
  ]
}
```

- **Prefira o equipamento como fonte da verdade** (é o modelo do
  `openwrt-wifi`): a ação de estado lê o que ele tem; cada mudança é uma ação
  com parâmetros que parte do estado atual, muda só o pedido e deixa o resto
  como está. A grade `matrix` cruza equipamentos × itens; `edit`/`remove`
  abrem a ação de frota com o formulário preenchido pelo item (parâmetro →
  campo) e só os equipamentos que o têm marcados.
- **Formulário a partir do equipamento**: no parâmetro, `"source":
  "networks.ssid"` oferece os valores da ação de estado (lista `networks`,
  chave `ssid`); na ação de frota, `"current": "_current.radios"` aponta os
  valores atuais no formato do formulário (o diálogo mostra e preenche). Chave
  que começa com `_` na saída é dado para a tela e não aparece no relatório.
- **Compatibilidade é pelo sistema** (`match.platforms`), não pelo hardware:
  `vendorRegex`/`modelRegex` só quando o plugin depende mesmo da placa.
- **`preview`**: toda escrita de frota tem uma ação de leitura com os mesmos
  parâmetros que mostra o que vai mudar (`#{ summary, changes, commands }`).
  Escreva um `plan_<op>(estado, params)` puro e use-o nas duas (DRY); a escrita
  guarda uma cópia, aplica de uma vez, **confere rodando o mesmo plano sobre o
  resultado (precisa sair vazio)** e restaura a cópia se não.
- **Segredos do equipamento não vêm para a central**: leia a configuração já
  mascarada no equipamento (`… | sed -E 's/(\.key)=.*/\1=******/'`) e, para
  reaproveitar uma senha existente, copie-a lá dentro
  (`uci set x.key="$(uci -q get y.key)"`). Senha nova é parâmetro
  `"secret": true` — o script recebe em claro, o histórico guarda `********` e o
  runtime a mascara na saída.
- **Plugins nascem desligados**: um plugin desligado não roda nem serve de
  `uses`. Ele liga ao cadastrar um equipamento compatível, no "Ativar e
  instalar" do Catálogo ou à mão — se precisar de um desligado, peça ao
  operador para ativá-lo.
- `settings` (estado desejado guardado no sistema) continua disponível quando
  o sistema deve ser a verdade: esquema dos `params` mais `array` (um nível;
  item ganha `id`), `secret` cifrado, lido em `device.settings.fleet/.device`.
- **`reduce`**: `fn <nome>(results, settings)` — pura, sem `device`. Recebe a
  lista dos membros (`#{ deviceId, deviceName, status, output, error }`);
  devolve o consolidado (`report`). Para sugerir uma ação, devolva
  `next: #{ action: "<ação de frota>", title, devices: #{ "<deviceId>": #{ param: valor } } }`
  — o operador confirma e o lote roda com os parâmetros de cada equipamento.
  (Com `settings`, `settings_patch` grava ajustes na configuração.)
- Ação de frota também aceita `labels`, para os títulos do consolidado.
- Testes: `"settings": { "fleet": {...}, "device": {...} }` no teste unitário
  simula a configuração; teste de `reduce` usa `"action": "<id da ação de
  frota>"` com `"input": [ ...resultados... ]` e sem fixtures.

## Antes de salvar

- [ ] `detect` existe, é leitura e devolve `firmware`.
- [ ] Cada ação tem `title`, `effect` certo, `usage` com a seção dela.
- [ ] Todo parâmetro de texto tem `pattern`/`enum` e vai com `shell_quote`.
- [ ] Escritas leem antes e são idempotentes.
- [ ] Fixtures copiadas das respostas reais, não inventadas.
- [ ] Nenhum comando destrutivo que o usuário não pediu.
- [ ] Já existe plugin que faz parte disso? Use `uses` + `device.use_plugin`.
- [ ] Aplicativo de frota: cada escrita com `preview`, cópia, conferência pelo
      mesmo plano e volta atrás; muda só o pedido; senha do equipamento não sai
      dele; senha nova com `secret: true`.
