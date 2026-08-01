# Diretrizes do Projeto para Agentes IA

## 🧪 Padrões Obrigatórios de Teste & Estabilidade

1. **Execução de Testes Obrigatória**:
   - Todo código novo ou refatorado deve conter testes correspondentes em `tests/unit/` ou `tests/functional/`.
   - Antes de considerar a tarefa finalizada, execute:
     ```bash
     npx tsc --noEmit
     node ace test
     ```

2. **Independência de Ambiente (Docker / Local)**:
   - Em operações I/O com arquivos temporários (como SQLite ou buffers local de probes), garanta a criação do diretório via `fs.mkdirSync(path, { recursive: true })` para evitar erros de diretório inexistente em contêineres Docker.
   - Suporte sempre as variáveis de banco de dados `DB_CONNECTION` (`sqlite` ou `pg`).

3. **Práticas de Teste no Japa**:
   - **Isolamento de Banco**: Em testes funcionais (`tests/functional`), sempre inclua `group.each.setup(() => testUtils.db().truncate())`.
   - **Timeouts**: Para testes que executam comandos de rede nativos (`ping`, `socket`), ajuste o timeout com `.timeout(5000)`.
   - **Evitar APIs Externas**: Utilize apenas `127.0.0.1` ou `localhost` para testes de rede automatizados.

4. **Documentação & Roadmap**:
   - Sempre atualize `docs/roadmap.md` marcando os itens concluídos com `[x]` e badge `🟢 Concluído`.
   - Consulte [diretrizes_testes.md](file:///d:/Projetos/Master%20sistemas/opensource/monitor%20de%20rede/docs/diretrizes_testes.md) para convenções de teste.
