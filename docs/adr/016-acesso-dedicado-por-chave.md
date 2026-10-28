# ADR 016 — Acesso dedicado por chave SSH: a senha do administrador só no primeiro acesso

## Status

**Aceito** (2026-10-28).

## Contexto

Até aqui a credencial SSH de um equipamento era a **senha do administrador**
(`root` no OpenWrt, `admin` no RouterOS). Havia dois jeitos de guardá-la
(`services/plugins/credentials.rs`): cifrada no banco (`vault`) ou pedida a
cada sessão (`ask`, só em memória por 30 min). Os dois tinham custo:

- `vault` deixa a senha humana no banco. Quem leva o banco e a
  `ENCRYPTION_KEY` leva o acesso administrativo de toda a frota, e de
  qualquer outro lugar onde a mesma senha se repita;
- trocar a senha do roteador quebra o monitoramento sem aviso;
- `ask` não serve ao agendador e cobra a senha a cada meia hora;
- não há como tirar o NetMonitor de um equipamento sem trocar a senha de
  todo mundo;
- o cliente SSH aceitava qualquer chave de host.

### Alternativas avaliadas

| Opção | Por que não é a principal |
|---|---|
| RouterOS REST (`/rest`, 7.x) e API 8728/8729 | Não há token: é usuário e senha a cada requisição. Troca o transporte e mantém o segredo. |
| OpenWrt ubus JSON-RPC (`POST /ubus`, `session.login` no rpcd) | O token de sessão expira (5 min por padrão, e some quando o rpcd reinicia). Para renovar é preciso a senha guardada. Exige `uhttpd-mod-ubus`, e 6 dos 7 plugins builtin são shell por SSH, que teriam de ser reescritos. Uma conta do rpcd com ACL restrita continua sendo boa opção **futura** para telemetria só de leitura. |
| Chave SSH dedicada | **Escolhida.** Não expira, não é a senha de uma pessoa, pode ser rotacionada sem o administrador e revogada no equipamento. Os plugins seguem iguais, porque o `exec` não muda. |

## Decisão

1. **Modo `key` na credencial.** `SecretStorage::Key` (`"key"`; a coluna
   `storage` já era texto, então não houve migração). `secret_encrypted` guarda
   a chave privada OpenSSH cifrada com `crypto::encrypt`. Em `extra.dedicated`
   ficam a plataforma, a chave pública, as impressões digitais, a marca da
   chave e a data. A chave privada nunca vai à tela, à IA ou ao script. O
   formulário comum não cria nem troca uma credencial `key`, e não volta para
   senha sem revogar antes.
2. **Senha uma vez.** `services/plugins/dedicated_access/` recebe a senha do
   administrador e não a guarda em lugar nenhum. O fluxo é: detectar a
   plataforma, instalar a chave, **provar** a chave numa conexão nova e só
   então gravar. Se a prova falha, o que foi instalado é desfeito e nada é
   gravado.
3. **Uma chave ed25519 por equipamento**, gerada na central a partir de 32
   bytes do gerador do sistema. O comentário `netmonitor-<12 hex>` marca a
   linha para removê-la depois.
4. **Por plataforma** (trait `Platform`: só os comandos e como cada uma
   reporta erro):
   - **RouterOS ≥ 7.12** (versão em que a chave de usuário ed25519 passou a
     ser aceita): usuário `netmonitor` no grupo `netmonitor`, com
     `ssh,read,write,test,policy,sensitive`, que é a união do que os plugins
     builtin usam. `policy` é para o usuário temporário do teste de enlace e
     `sensitive` para conferir a senha de uma rede. O usuário recebe uma senha
     aleatória que é descartada. A chave chega por `/file add` e entra por
     `/user ssh-keys import`. Com chave cadastrada, o padrão
     `password-authentication=yes-if-no-key` já recusa senha no SSH para esse
     usuário, e a configuração global não é alterada.
   - **OpenWrt com Dropbear**: a chave entra no `authorized_keys` do root com
     `no-port-forwarding,no-agent-forwarding,no-X11-forwarding`. É o root
     porque `uci`, `ubus` e Wi-Fi só funcionam como root e o OpenWrt não tem
     `sudo`. `no-pty` fica de fora porque a ativação do syslog abre um shell
     com PTY.
   - O acesso **não** é restrito por IP de origem: a central muda de endereço
     com NAT, VPN e agente remoto.
5. **Chave de host fixada (TOFU).** A impressão digital vista na instalação vai
   para `extra.dedicated.hostKey` e passa a acompanhar o `Login`. Uma chave
   diferente derruba a conexão antes de a chave do NetMonitor ser apresentada.
   O handler (`shared::ssh::PinnedHostKey`) é o mesmo do SFTP dos backups.
6. **Rotação** usa a chave atual, sem o administrador: instala a nova, prova,
   retira a antiga. Se a nova falha, a antiga volta pela sessão que continua
   aberta. **Revogação** retira a chave (no RouterOS, também o usuário e o
   grupo) e apaga a credencial. Equipamento inacessível não conta como
   revogado: o operador escolhe "apagar só no NetMonitor".
7. **Agente remoto.** Os comandos saem pelo mesmo `DeviceTransport` dos
   plugins, sem comando novo no enum. O `Login` do `DeviceIo` passou a levar
   `privateKey` e `hostKey`, e a resposta `Exec` passou a levar `hostKey`, para
   o TOFU funcionar também pelo agente. Um agente antigo descartaria esses
   campos em silêncio, por isso **`PROTOCOL_VERSION` sobe para 5**.
8. **Rotas**: `POST /api/devices/{id}/access/dedicated` (cria),
   `POST …/dedicated/rotate` e `DELETE …/dedicated` (`{ localOnly }`). Todas são
   auditadas como `Execute` no equipamento. O `DELETE` genérico de credencial
   recusa a do acesso dedicado: apagá-la só aqui deixaria a chave esquecida no
   equipamento.
9. **Nenhuma senha de equipamento no banco** (SSH, Telnet e HTTP). O modo
   `vault` deixou de existir; a migração `m20261028_000001` apagou as senhas
   cifradas e passou essas credenciais para `ask`. Sobram dois modos: `key` (o
   acesso dedicado) e `ask`, em que o banco guarda usuário, porta e agente e a
   senha é pedida **numa janela só** — o `DeviceAccessDialog`, aberto de
   qualquer tela por `useDeviceAccess` quando a API responde
   `missing_credential` — e fica 30 minutos em memória. O "salvar" da
   ativação do syslog passou a lembrar o login por esses 30 minutos. HTTP sem a
   senha da sessão segue sem senha (a página pode ser pública).

## Consequências

- A senha humana sai do banco. O que fica é uma chave que só entra naquele
  equipamento, só pelo SSH e, no RouterOS, só com as políticas listadas.
- No OpenWrt a chave ainda é do root. O ganho ali é não guardar a senha,
  sobreviver à troca dela e poder revogar, não reduzir privilégio.
- RouterOS anterior à 7.12 e OpenWrt com OpenSSH no lugar do Dropbear recebem
  uma mensagem clara e continuam com a senha.
- Telnet continua por senha: não existe chave no Telnet — pedida na janela.
- O que roda sozinho (agendador, lote de frota, teste de enlace agendado) só
  alcança equipamento com acesso dedicado: a senha da janela vale para a API,
  nunca para o agendador.
- Validado contra RouterOS CHR 7.23.8 e OpenWrt 24.10.4 reais (Docker): criar,
  usar, rotacionar e revogar; no RouterOS o `netmonitor` recebe só as políticas
  listadas, o SSH recusa senha para ele, e o plugin `mikrotik-routeros` roda
  inteiro com ele (inclusive o usuário temporário do teste de enlace). O teste
  `acesso_dedicado_em_equipamento_real` (ignorado por padrão) repete isso.
- A senha descartável do usuário `netmonitor` passa pelo canal SSH já
  autenticado e não fica em lugar nenhum.
