# 🧪 Diretrizes de Teste & Boas Práticas (Agentes & Devs)

> [!IMPORTANT]
> Todo novo recurso, refatoração ou correção de bug **DEVE** vir acompanhado de testes unitários ou funcionais. Nunca declare uma tarefa concluída sem rodar `node ace test` e `npx tsc --noEmit`.

---

## 📌 1. Estrutura de Testes no Projeto

O projeto utiliza o **Japa Test Runner** integrado ao AdonisJS v6:

| Tipo | Localização | Escopo & Regras |
| :--- | :--- | :--- |
| **Unitários** | `tests/unit/**/*.spec.ts` | Testam funções puras, checkers, conversores e mergers isoladamente. **Não dependem do banco de dados nem da internet.** |
| **Funcionais** | `tests/functional/**/*.spec.ts` | Testam endpoints HTTP da API REST e fluxos completos de banco de dados. Usam o cliente HTTP do Japa. |

---

## ⚡ 2. Regras de Ouro para Criação de Testes

### 1. Limpeza do Banco em Testes Funcionais
Sempre utilize `group.each.setup(() => testUtils.db().truncate())` em suítes funcionais para evitar contaminação de estado entre os testes.

```ts
import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'

test.group('Sites API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('POST /api/sites deve criar site', async ({ client, assert }) => {
    const response = await client.post('/api/sites').json({ name: 'Matriz' })
    response.assertStatus(201)
  })
})
```

### 2. Evite Dependências de Rede Externa
> [!WARNING]
> Nunca faça requisições para serviços externos (como `httpbin.org` ou `google.com`) em testes unitários automatizados. Isso gera instabilidade e falhas por latência/timeout.

- Utilize `127.0.0.1`, `localhost` ou mock para testes de timeout.
- Se o teste envolver execução assíncrona de processos do sistema (como `ping` nativo), ajuste o limite de tempo do Japa com `.timeout(5000)`.

```ts
test('PingChecker deve medir latência local', async ({ assert }) => {
  const checker = new PingChecker()
  const result = await checker.execute({ host: '127.0.0.1', packetCount: 1, timeoutMs: 1000 })
  assert.exists(result.startedAt)
}).timeout(5000)
```

### 3. Independência de Ambiente (Docker / Local)
- **Garantia de pastas**: Nunca assuma que diretórios temporários existem. Ao utilizar SQLite ou manipular arquivos locais em novos módulos, garanta a criação do diretório via `fs.mkdirSync(dir, { recursive: true })`.
- **Status Codes Estritos**: Valide explicitamente os códigos HTTP de resposta (ex: `200` para OK, `201` para Criado, `204` para Exclusão sem conteúdo).

---

## 🛠️ 3. Checklist de Verificação Antes do Commit

Antes de considerar uma alteração pronta, execute os seguintes passos:

1. **Checagem de Tipos (TypeScript)**:
   ```bash
   npx tsc --noEmit
   ```
2. **Suíte Completa de Testes**:
   ```bash
   node ace test
   ```
3. **Format & Lint**:
   ```bash
   npm run lint
   ```
