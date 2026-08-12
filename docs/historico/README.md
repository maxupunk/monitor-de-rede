# Histórico

Documentos **encerrados**. Descrevem o que foi feito, num período que já passou,
e não valem como instrução de trabalho.

A regra que separa esta pasta do resto de `docs/`: documento que descreve **como
o sistema é hoje** (arquitetura, diretrizes, roadmap ativo) fica em `docs/` e
precisa falar Rust. Documento que registra **uma decisão ou um procedimento
datado** vem para cá, intacto — reescrever apagaria o registro.

| Documento | O que é |
| :--- | :--- |
| [roadmap_backend_rust.md](roadmap_backend_rust.md) | O plano da reescrita do backend AdonisJS → Rust (Loco.rs), da Fase 0 ao corte. Concluído. |
| [corte_backend_rust.md](corte_backend_rust.md) | O runbook do corte em si: migração de dados, re-cifra dos segredos da VPN, descomissionamento. Executado. |

Ambos falam AdonisJS, Lucid, `node ace` e Japa no presente. Era o presente deles.
Para o comportamento atual do sistema, leia [`../arquitetura.md`](../arquitetura.md).

Os **ADRs** ficam em [`../adr/`](../adr/), não aqui: eles seguem valendo como
justificativa das decisões que ainda estão de pé.

O código do backend AdonisJS não está mais no repositório. Ele é recuperável
pela tag `adonisjs-final`:

```bash
git show adonisjs-final:backend/app/services/monitor_service.ts
git checkout adonisjs-final -- backend/   # traz o diretório inteiro de volta
```
