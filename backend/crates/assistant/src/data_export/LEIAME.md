# Dados do Assistente IA — NetMonitor

Pacote gerado em **Configurações → Inteligência artificial → Dados do
Assistente**. Serve para treinar ou ajustar um modelo e para revisar onde a IA
errou e onde acertou.

## Arquivos

| Arquivo | Para quê | Formato |
| --- | --- | --- |
| `manifesto.json` | Período, data da geração e contagens do pacote. | JSON |
| `conversas.jsonl` | Revisão: cada conversa completa, como a tela mostrou, com os sinais de erro e acerto. | Uma conversa por linha |
| `treino/sem-falhas.jsonl` | Treino: conversas em que nenhuma resposta deu erro, nenhuma ferramenta falhou e nenhuma resposta foi avaliada como ruim. | `{"messages": [...]}` por linha |
| `treino/com-falhas.jsonl` | Revisão antes de treinar: conversas com ao menos uma falha. | `{"messages": [...]}` por linha |
| `execucoes-em-equipamentos.jsonl` | Comandos que a IA mandou aos equipamentos, com a transcrição de cada acesso. | Uma execução por linha |

## `conversas.jsonl`

Cada linha traz `id`, `title`, `user` (id e nome de quem perguntou),
`createdAt`, `updatedAt`, `outcome` (`noFailures` ou `withFailures`),
`signals` e `messages`.

- `signals` conta perguntas, respostas, ferramentas (sucesso, falha, recusadas
  pelo usuário, esperando confirmação), respostas com erro, avaliações do
  usuário (`ratedGood`/`ratedBad`), tokens e modelos.
- `signals.failures` aponta cada falha: `messageIndex` (a posição em
  `messages`), `kind` (`response`, `tool` ou `rating`), a ferramenta e o erro
  ou o motivo da avaliação.
- No `manifesto.json`, `totals.toolFailures` diz quantas vezes cada ferramenta
  falhou no período — por onde começar a corrigir.
- `messages` é a conversa como foi guardada: `role`, `content`, `toolCalls`
  (nome, argumentos, resultado, status), `usage`, `error`, `notice`,
  `rating` (👍/👎 do usuário, com o motivo) e, nas perguntas, `mentions` (o
  que foi marcado com `@`) e `screen` (a tela em que a pessoa estava).

## `treino/*.jsonl`

Formato de chat com ferramentas (`user`, `assistant` com `tool_calls`, `tool`),
lido por OpenAI fine-tuning, Unsloth, Axolotl e similares. Ficam de fora o
prompt de sistema (montado a cada pergunta, não é guardado), gráficos e uso de
tokens. Ação recusada pelo usuário aparece ao modelo como
`{"status":"declined_by_user"}`.

## Como a classificação é feita

**Com falhas** é a conversa em que alguma resposta terminou em erro, alguma
ferramenta falhou ou o usuário avaliou alguma resposta como ruim (👎). Ação
recusada pelo usuário **não** conta como falha — pode ser sugestão errada ou só
mudança de ideia —, mas aparece em `signals.actionsDeclined` para a revisão.
Respostas com 👍 são os acertos confirmados (`signals.ratedGood`).

## Onde o pós-treinamento acontece

O ajuste dos modelos com estes dados é feito no **Laya-tuning**
(`D:\Projetos\Master sistemas\opensource
et-monitor-ia-decisao-tuning`):
lá ficam os datasets, as receitas de fine-tune, o benchmark e os resultados.
Leve `treino/sem-falhas.jsonl` (exemplos a manter) e `treino/com-falhas.jsonl`
(erros a corrigir, depois de revisados) para aquele repositório.

## Cuidados

O pacote contém perguntas, nomes de usuários e de equipamentos, endereços e
saídas de comandos da sua rede. A senha de acesso aos equipamentos não é
gravada (os comandos guardam `{{password}}`, e campos secretos de plugin saem
como `********`), mas a **saída** de um comando pode trazer outros segredos —
uma chave de Wi-Fi lida da configuração, por exemplo. Revise antes de enviar a
um serviço externo.
