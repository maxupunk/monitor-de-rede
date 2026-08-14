# Diretrizes do Projeto para Agentes IA

> **O backend é Rust (Loco.rs), em `backend/`. Ponto.** Não existe outro
> backend no repositório, nem "referência de comportamento" a consultar: se a
> pergunta é como o sistema se comporta, a resposta está em `backend/`.
>
> O backend anterior era AdonisJS. Ele saiu do repositório e continua
> recuperável pela tag `adonisjs-final` (`git show adonisjs-final:backend/...`).
> A migração está registrada em
> [docs/historico/](docs/historico/) — leitura histórica, não guia de trabalho.

## 🧪 Padrões Obrigatórios de Teste & Estabilidade

1. **Validação Obrigatória Pré-Finalização**:
   - **Frontend**:
     ```bash
     npm --prefix frontend run typecheck
     npm --prefix frontend run format
     npm --prefix frontend run lint
     npm --prefix frontend run build
     ```
   - **Backend (Rust)**:
     ```bash
     cargo fmt --all --check
     cargo clippy --all-targets -- -D warnings
     cargo test
     cargo build --release
     ```
     Rodados a partir de `backend/`. Os quatro precisam passar — é
     critério de aceite, não sugestão.

2. **Regras de Qualidade Vue / Template HTML**:
   - Fechamento estrito de tags Vuetify (ex: `<v-row></v-row>`).
   - Usar concatenação ou objetos no prop `:to` sem misturar template strings dentro de aspas duplas.
   - Não colocar comentários `<!-- -->` entre diretivas `v-if` e `v-else`.
   - Usar props `:title` e `:subtitle` no `<v-list-item>` do Vuetify 3 para evitar ambiguidades de slot.

3. **Independência de Ambiente (Docker / Local)**:
   - O backend Rust roda em **SQLite** (teste, dev e produção) e **PostgreSQL**
     (instalações grandes, via `DATABASE_URL`). Toda consulta precisa valer nos
     dois dialetos — a produção padrão passou a ser o SQLite, então quebrar o
     dialeto dele agora quebra a instalação real, não só a suíte.
   - **Entidades do `sea-orm` são geradas contra o PostgreSQL**, nunca contra o
     SQLite: o SQLite reporta todo inteiro como `INTEGER` e o `db entities`
     rodado contra ele produz `i64` onde o Postgres tem `INT4` — e aí o `sqlx`
     recusa a leitura em produção.
   - **Alinhamento de Peer Dependencies (frontend)**: ao atualizar/adicionar
     dependências em `package.json`, garanta versões compatíveis para evitar
     `ERESOLVE`, e rode `npm --prefix frontend install` para sincronizar o
     `package-lock.json`.

4. **Práticas de Teste (Rust)**:
   - **Isolamento de Banco**: testes de requisição usam
     `request_with_config::<App, _, _>`; o `Hooks::truncate` limpa as 23 tabelas
     entre eles.
   - **`#[serial]`** em tudo que toca estado global de processo: `ScanSessionService`,
     o cofre de chaves da VPN, o rate limiter e qualquer teste que mexa em
     variável de ambiente.
   - **Timeouts**: 5 s por teste de rede nativo (`ping`, `socket`).
   - **Ambiente Local**: apenas `127.0.0.1` ou `localhost` — nada de alvo externo.
   - Funções puras têm teste unitário no próprio módulo (`#[cfg(test)] mod tests`);
     artefatos textuais (scripts de VPN, `wg0.conf`) usam snapshot `insta`.

5. **Documentação & Roadmap**:
   - Atualize `docs/roadmap.md` marcando itens concluídos com `[x]` e badge
     `🟢 Concluído`. O `roadmap_backend_rust.md` está encerrado e mora em
     `docs/historico/`; não escreva nele.
   - Consulte [arquitetura.md](docs/arquitetura.md) e [diretrizes_testes.md](docs/diretrizes_testes.md).

6. **Preservação e Regras de Negócio do Módulo `vpn-probe`**:
   - **Agente Dedicado (`vpn-probe`)**: mede ICMP/SNMP na faixa `10.8.0.x` de
     dentro do namespace de rede do WireGuard. Na topologia padrão esse
     namespace é o do **próprio container** — a `wg0` é do processo da API, o
     agente não é registrado no boot e os monitores da VPN rodam locais.
     `VPN_PROBE_EXTERNAL=true` reativa o arranjo com o agente à parte. O código
     do agente, do registrador e do provisionador continua obrigatório: é ele
     que sustenta os dois arranjos.
   - **Token Fallback Padrão (`DEFAULT_VPN_PROBE_TOKEN`)**: o registrador
     (`services/vpn/probe_registrar.rs`) e o agente (`services/probes/agent.rs`)
     **DEVEM** usar `DEFAULT_VPN_PROBE_TOKEN = "default_vpn_probe_token"` como
     fallback quando `VPN_PROBE_TOKEN`/`PROBE_TOKEN` não estiverem definidos.
     **NUNCA remova este fallback**: é ele que garante registro e autenticação
     zero-config em containers Docker. É também a razão de
     `probes.token_hash` não ter índice único.
   - **Comando CLI de Registro**: `backend-cli task vpn_probe_register`
     (`src/tasks/vpn_probe_register.rs`) gera ou reutiliza o token do probe e o
     exibe no terminal. **NÃO remover** este comando nem o `probe_registrar`.
   - **Fallback de Execução Local no Agendador** (`src/tasks/scheduler_run.rs`):
     se o probe estiver offline por qualquer motivo, o agendador **DEVE** tentar
     a execução local via `run_monitor` antes de reportar `unknown`. **NÃO
     remover essa tratativa.**

7. **Fronteiras que não se atravessam**:
   - O processo da API **nunca** executa `wg` nem `docker exec`. Ele escreve
     `<iface>.conf` e lê `<iface>.status` em `WG_CONFIG_DIR`; quem aplica é o
     watcher (`docker/wireguard-watcher.sh`), um processo root separado. O
     container ganhou `NET_ADMIN` quando o WireGuard passou a morar nele, mas o
     processo da API **não**: o entrypoint o chama com
     `setpriv --inh-caps=-all` e ele roda com `CapEff` zerado. Mover o `wg`
     para dentro da API atravessaria a fronteira; aproximar os dois processos,
     não.
   - O ping usa socket ICMP `SOCK_DGRAM` (ADR 003) — sem `CAP_NET_RAW` e sem
     `execFile('ping')`. O `sysctl net.ipv4.ping_group_range` está no compose.
   - A chave privada de um peer **nunca** vai ao banco: vive no cofre em memória
     até a primeira leitura. Depois disso, só rotacionando.
   - Controller extrai, valida, delega e serializa. Regra de negócio vive em
     `src/services/`, testável sem HTTP.

8. **Edições Cirúrgicas e Preservação de Formatação / Indentação (Economia de Tokens)**:
   - **Edições cirúrgicas**: Faça apenas modificações pontuais no escopo estrito da tarefa. **NUNCA** reformate arquivos inteiros nem altere a indentação, espaçamento ou quebras de linha de blocos de código não relacionados à mudança funcional.
   - **Evitar retrabalho com formatadores**: O repositório já possui formatadores padronizados (`cargo fmt` no backend e `npm run format` / Prettier no frontend). Alterar indentação arbitrariamente faz com que a etapa de validação (`format`) precise desfazer ou reformatar tudo, inflando diffs e desperdiçando tokens desnecessariamente.
   - **Respeite o estilo local**: Ao escrever código novo, siga rigorosamente o padrão de indentação e espaçamento já presente no arquivo para que o formatador não precise reformatar o bloco na validação final.
