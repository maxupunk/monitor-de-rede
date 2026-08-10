# Diretrizes do Projeto para Agentes IA

## 🧪 Padrões Obrigatórios de Teste & Estabilidade

1. **Validação Obrigatória Pré-Finalização**:
   - **Frontend**:
     ```bash
     npm --prefix frontend run typecheck
     npm --prefix frontend run format
     npm --prefix frontend run lint
     npm --prefix frontend run build
     ```
   - **Backend**:
     ```bash
     npm --prefix backend run typecheck
     npm --prefix backend run test
     ```

2. **Regras de Qualidade Vue / Template HTML**:
   - Fechamento estrito de tags Vuetify (ex: `<v-row></v-row>`).
   - Usar concatenação ou objetos no prop `:to` sem misturar template strings dentro de aspas duplas.
   - Não colocar comentários `<!-- -->` entre diretivas `v-if` e `v-else`.
   - Usar props `:title` e `:subtitle` no `<v-list-item>` do Vuetify 3 para evitar ambiguidades de slot.

3. **Independência de Ambiente (Docker / Local) & Sincronização de Lockfile**:
   - Garanta a criação recursiva de diretórios temporários via `fs.mkdirSync(path, { recursive: true })`.
   - Suporte dinâmico para `DB_CONNECTION` (`sqlite` ou `pg`).
   - **Alinhamento de Peer Dependencies**: Ao atualizar/adicionar dependências em `package.json`, garanta que dependências equivalentes (ex: `vue-eslint-parser` e `eslint-plugin-vue`) estejam em versões compatíveis para evitar erros `ERESOLVE` no npm.
   - **Sincronização Obrigatória do `package-lock.json`**: Sempre que alterar qualquer `package.json` (backend ou frontend), você **DEVE obrigatoriamente** rodar `npm --prefix backend install` ou `npm --prefix frontend install` para sincronizar o `package-lock.json`.
   - **Configuração de Dockerfile**: Mantenha os Dockerfiles utilizando o padrão limpo `RUN npm ci`. Garantindo o alinhamento de dependências no `package.json` e a sincronização do `package-lock.json`, o build do Docker roda 100% nativo e performático sem a necessidade de flags ou contornos.

4. **Práticas de Teste no Japa (Backend)**:
   - **Isolamento de Banco**: Inclua `group.each.setup(() => testUtils.db().truncate())` em testes funcionais.
   - **Timeouts**: Defina `.timeout(5000)` para testes de rede nativos (`ping`, `socket`).
   - **Ambiente Local**: Utilize apenas `127.0.0.1` ou `localhost`.

5. **Documentação & Roadmap**:
   - Atualize `docs/roadmap.md` marcando itens concluídos com `[x]` e badge `🟢 Concluído`.
   - Consulte [diretrizes_qualidade_e_checklist.md](file:///d:/Projetos/Master%20sistemas/opensource/monitor%20de%20rede/docs/diretrizes_qualidade_e_checklist.md) e [diretrizes_testes.md](file:///d:/Projetos/Master%20sistemas/opensource/monitor%20de%20rede/docs/diretrizes_testes.md).

6. **Preservação e Regras de Negócio do Módulo `vpn-probe`**:
   - **Agente Dedicado (`vpn-probe`)**: O `vpn-probe` compartilha o namespace de rede do WireGuard (`network_mode: "service:wireguard"`) para realizar medições ICMP/SNMP na faixa `10.8.0.x`.
   - **Token Fallback Padrão (`DEFAULT_VPN_PROBE_TOKEN`)**: `VpnProbeRegistrar` e `ProbeAgent` **DEVEM** utilizar `DEFAULT_VPN_PROBE_TOKEN = 'default_vpn_probe_token'` como fallback padrão se `VPN_PROBE_TOKEN` / `PROBE_TOKEN` não for definido no `.env`. **NUNCA remova este fallback**, pois ele garante registro e autenticação zero-config em containers Docker.
   - **Comando CLI de Registro (`node ace vpn:probe-register`)**: O comando `node ace vpn:probe-register` ([vpn_probe_register.ts](file:///d:/Projetos/Master%20sistemas/opensource/monitor%20de%20rede/commands/vpn_probe_register.ts)) gera (ou reutiliza) um token de registro do probe no banco de dados e o exibe no terminal. **NÃO remover este comando CLI ou suas dependências em `VpnProbeRegistrar`**.
   - **Fallback de Execução Local no Agendador (`scheduler_run.ts`)**: Se o `vpn-probe` estiver offline por qualquer motivo, o agendador **DEVE** tentar a execução local via `monitorRunner.runMonitor` antes de desmarcar o monitor e reportar `UNKNOWN`. **NÃO remover essa tratativa de fallback**.
