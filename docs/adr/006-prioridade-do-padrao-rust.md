# ADR 006 — Quando o contrato HTTP e o padrão Loco/Rust colidem, o Rust ganha

- **Status:** aceito — Fase 0
- **Data:** 2026-08-10
- **Decisão do responsável pelo projeto**, registrada aqui porque altera a
  leitura da §1.3.1 e da §12 do `roadmap_backend_rust.md`.

## Contexto

O roadmap foi escrito sob um princípio forte (§1.3.1): *"o contrato HTTP é
sagrado; o frontend não sabe que o backend mudou"*. Isso faz sentido enquanto o
objetivo é trocar o motor sem parar o carro.

Mas o frontend Vue 3 + Vuetify **está no mesmo repositório e é nosso**. Quando
manter um formato herdado do AdonisJS custa contorcer o backend Rust — um
`serde` com `rename` manual campo a campo, um wrapper para imitar um envelope,
uma serialização que o `sea-orm` não produz naturalmente — o preço é pago para
sempre, do lado que vai receber todo o desenvolvimento futuro.

## Decisão

**Aproveitar o frontend existente e apenas adaptá-lo. Onde houver conflito
entre preservar o formato do AdonisJS e escrever Rust/Loco idiomático, vale o
padrão do backend Rust, e o frontend é ajustado.**

Regras de aplicação, em ordem:

1. **Preservar por padrão.** Se o formato herdado não custa nada em Rust, ele
   fica. Continuidade de graça é continuidade que se aceita.
2. **Adaptar quando custa.** Se preservar exige um artifício no backend, muda-se
   o backend para o idiomático e o frontend acompanha.
3. **Registrar sempre.** Toda adaptação de frontend vira uma linha na §12 do
   roadmap, com o arquivo e o motivo. Nenhuma mudança silenciosa.
4. **Adaptação é cirúrgica.** Ajustar o tipo lido, o nome de um campo, o caminho
   de uma rota. **Não** é redesenhar tela, trocar biblioteca nem "aproveitar
   para melhorar" — isso continua fora de escopo (§1.2).

## Consequências

### Já aplicado na Fase 0

| Mudança | Onde | Por quê |
| :--- | :--- | :--- |
| `LoginResponse.is_verified` → `isVerified` | `src/views/auth.rs` | A §5.1 manda `camelCase` em **todo** DTO. Abrir exceção para o scaffold quebraria a regra que o teste de convenção fiscaliza. |
| Bindings `ts-rs` passam a ser gerados em `frontend/src/bindings/` | `src/dtos/common.rs`, `src/services/shared/pagination.rs` | O scaffold exportava para `backend-rust/frontend/`, um diretório que ninguém consome. Agora o tipo do backend é a fonte da verdade do tipo do frontend. |
| `useInfiniteList` importa `LucidMeta` gerado | `frontend/src/composables/useInfiniteList.ts` | O `meta` deixa de ser redigitado à mão no TypeScript. Se o backend mudar um campo, o `vue-tsc` acusa — em vez de a lista infinita parar sozinha em produção. |
| Prefixo `/api` sai do controller e vai para o `AppRoutes` | `src/controllers/auth.rs`, `src/app.rs` | `AppRoutes::prefix` é o mecanismo do Loco para agrupar rotas. O scaffold embutia `/api/auth` no controller. As URLs finais não mudaram. |

### O que **não** muda por causa desta decisão

O envelope de paginação do Lucid (§5.4) e o corpo de erro `{message}` (§5.5)
continuam como estão. Não são dívida herdada: são o contrato que cinco telas já
consomem, e reproduzi-los em Rust custou um struct e um `IntoResponse` — nada
de artifício. A regra 1 se aplica.

### Efeito colateral aceito

Frontend e backend passam a subir juntos. Um deploy que atualize só um dos dois
pode quebrar uma tela. Isso já era verdade na prática (o proxy do Vite aponta
para uma porta fixa); agora está escrito.
