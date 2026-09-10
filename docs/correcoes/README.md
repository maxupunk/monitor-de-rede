# Correções

Um arquivo por defeito corrigido, com o que estava errado, por que estava errado, o que
foi feito e como foi validado. Diferente dos ADRs (`docs/adr/`), que registram **decisões
de arquitetura**: aqui ficam os **defeitos** — inclusive os que só apareceram em produção.

## 2026-09-09 — Investigação a partir do relato de deslogamento e da duplicata `pppoe-wan`

O relato inicial ("ao logar, poucos segundos e desloga") e o segundo ("duas `pppoe-wan` no
dispositivo, e salvar as configurações não faz nada") destravaram cinco defeitos
independentes. A ordem abaixo é a de execução.

| # | Correção | Severidade |
|---|---|---|
| [01](2026-09-09-01-apply-monitors-guarda-snmp.md) | `apply_monitors` apagava monitores e histórico quando a varredura SNMP falhava | crítica |
| [02](2026-09-09-02-fusao-interfaces-duplicadas.md) | Fusão das interfaces duplicadas por (dispositivo, nome) | alta |
| [03](2026-09-09-03-casamento-de-interface-por-nome.md) | Interface casada por identidade estável, não por `ifIndex` | alta |
| [04](2026-09-09-04-pool-de-conexoes-e-401-por-falha-de-banco.md) | Pool de uma conexão, e o 401 que na verdade era falha de banco | alta |
| [05](2026-09-09-05-timeout-das-chamadas-snmp.md) | Timeout do cliente para as chamadas SNMP | média |

### Como os cinco se encaixam

- **02 e 03 são o mesmo defeito em dois tempos.** A 03 conserta a causa (o casamento por
  índice, que criava a duplicata); a 02 limpa o dado que a causa já produziu.
- **01 e 05 explicam o "salvar não faz nada".** A 01 impede que a gravação destrua o
  histórico quando o walk falha; a 05 impede que o navegador desista antes de o backend
  terminar.
- **04 explica o "deslogando sem motivo"** e os 500 em `GET /api/auth/setup`.

### Defeito relacionado, ainda em aberto

O token de sessão vai para o `sessionStorage` quando "Permanecer logado" está desmarcado
(`stores/auth.ts`), mas o `apiService` lê **só** do `localStorage`. Sem o checkbox, a
primeira requisição autenticada sai sem `Authorization`, leva 401 e derruba a sessão. É
independente das cinco acima e não estava no escopo combinado.
