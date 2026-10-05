# Laya — plano de treinamento da identificação de dispositivos

> Para quem mantém o NetMonitor e o modelo do Ollaya. Explica por que o Laya
> errou, o que já foi corrigido e como fazê-lo aprender com a rede de cada
> instalação — medindo antes de mudar.

## 1. Como o Laya decide hoje

O Laya é um **classificador**, não um gerador de texto. A descoberta manda ao
Ollaya (`POST /api/decide`):

- um **estado**: os fatos do host em JSON (`DeviceFacts`, em
  `services/ai/laya/decisions/device_identity.rs`) — nome, fabricante, portas,
  serviços, `sysDescr`, `sysObjectID`, página web, UPnP, mDNS, NetBIOS;
- uma **pergunta de escolha**: um rótulo por tipo, cada um com uma descrição
  em inglês (`laya_hint`, em `services/devices/kinds.rs`).

O modelo compara o estado com cada descrição e devolve o rótulo escolhido com
uma probabilidade. Abaixo da confiança mínima (60% por padrão) não há
sugestão.

Dois limites importantes:

1. **Ele só é consultado quando a heurística ficou em dúvida** (tipo `unknown`
   ou `web_device`, ou sistema sem evidência — `discovery/laya_identity.rs`).
   Ou seja, sempre nos casos mais difíceis.
2. **Ele não aprende sozinho.** Não há treinamento contínuo: cada pergunta é
   respondida só com o estado e as descrições. "Treinar" o Laya, hoje,
   significa melhorar o que entra (estado e critérios) e como a saída é
   conferida. Ajustar os pesos do modelo é a última fase deste plano.

## 2. Diagnóstico: por que um OpenWrt virou "NAS 98%"

O host era uma Banana Pi R3 com OpenWrt. O que a descoberta viu:

| Fato | Valor |
|---|---|
| `sysDescr` | `Linux bpi-r3-assistencia 6.12.94 #0 SMP Mon Jun 29 12:59:20 2026 aarch64` |
| `sysObjectID` | `1.3.6.1.4.1.8072.3.2.10` (Net-SNMP — o agente, não o fabricante) |
| Fabricante | "Net-SNMP" (deduzido do `sysObjectID`) |
| Portas | 22, 80, 443 — **mas a varredura no Docker Desktop não as viu** (ver §2.2) |

### 2.1 A cadeia de erros

1. **A heurística não reconheceu o OpenWrt.** O nome "OpenWrt" não aparece em
   lugar nenhum; o `sysDescr` só diz "Linux". O sistema ficou "Linux genérico"
   e o tipo, `unknown` — e por isso o Laya foi consultado.
2. **Os critérios se sobrepunham.** A descrição de "servidor" dizia
   *"Server, NAS or virtualization host (Linux…)"* e a de NAS, *"Network
   storage appliance"*. Para um texto "Linux … aarch64 + Net-SNMP", NAS e
   servidor eram os rótulos mais próximos. A de roteador citava OpenWrt pelo
   nome, que o estado não continha.
3. **Faltava contexto.** Sem as portas (perdidas pela falha de §2.2), o estado
   não tinha nada de roteador: nem DNS, nem SSH, nem página web.
4. **Nada conferia a confiança.** Numa escolha entre poucos rótulos, a
   probabilidade se concentra no menos pior. Com 98%, o palpite passou do
   mínimo e chegou à tela como se fosse certeza.

### 2.2 A falha que agravou: zero portas no Docker Desktop

No Docker Desktop (Windows e macOS), a rede do contêiner passa por um proxy no
sistema anfitrião. Cada SYN para um IP inexistente vira uma conexão real que o
anfitrião só abandona depois de ~20 s. A prova de vida por TCP dos endereços
mudos disparava ~3.300 dessas antes da identificação. Saturado, o proxy fazia
**todas** as conexões seguintes expirarem — inclusive as das portas abertas.

Medido num contêiner descartável: depois da rajada, 6 de 6 portas abertas
pareciam filtradas, e o proxy só se recuperou ~15 s depois.

## 3. O que já foi corrigido (fase 1)

| Problema | Correção | Onde |
|---|---|---|
| OpenWrt sem o nome no `sysDescr` | O OpenWrt compila o kernel com a versão de build `#0` (`… #0 SMP Mon …`); Debian, Ubuntu, Alpine e Arch usam `#1`, `#58-Ubuntu`… Essa assinatura agora identifica o **sistema** (OpenWrt) e o **tipo** (roteador) — o Laya nem é consultado | `DeviceAdapter::kernel_signature`, `adapters/platforms.rs`, `devices/systems.rs` |
| Zero portas no Docker Desktop | A identificação dos hosts que responderam ao ping vem **antes** da prova de vida dos mudos; a prova de vida usa 6 portas (não 16) quando a rede responde ping | `discovery/pipeline.rs`, `fingerprints::LIVENESS_PORTS_LIGHT` |
| Falha silenciosa | O registro da varredura diz o que cada etapa achou e avisa quando não há MAC (contêiner em bridge) | `pipeline.rs` |
| Critérios sobrepostos | Roteador cita as marcas do OpenWrt (kernel `#0`, Net-SNMP, dropbear, LuCI, DNS); servidor não fala mais em NAS; NAS exige evidência de armazenamento | `devices/kinds.rs` |
| Estado pouco legível | Os fatos levam o nome dos serviços ("SSH", "RTSP", "Impressão RAW"), não só o número da porta | `DeviceFacts::services` |
| Confiança sem conferência | **Calibração por evidência:** se a heurística viu evidência de outros tipos e nenhuma do tipo escolhido, o palpite perde 40% da confiança. "NAS 98%" sem nada de NAS vira 58,8% e some | `laya_identity::calibrate` |

Os casos reais estão fixados em testes: `openwrt_pelo_kernel_vira_roteador`,
`kernel_compilado_como_o_do_openwrt_identifica_o_sistema`,
`kernel_de_distribuicao_continua_linux` e
`palpite_sem_evidencia_a_favor_perde_confianca`.

## 4. Princípios do treinamento

- **A heurística decide, o Laya sugere, o operador confirma.** Nenhuma fase
  deste plano aplica tipo automaticamente.
- **Medir antes de mudar.** Toda mudança de critério, estado ou modelo roda
  contra o conjunto de avaliação (§5.1) e só entra se não piorar nada.
- **Corrigir na heurística o que é determinístico.** Se um sinal é certo (o
  kernel `#0` do OpenWrt, o `sysObjectID` de um fabricante), ele vira regra
  em `fingerprints.rs` ou num adaptador — não fica para o modelo adivinhar.
- **Os dados não saem da instalação.** Rótulos e exemplos ficam no banco
  local. Exportar para treinar é decisão explícita do operador, e sem IP, MAC
  ou nome (ver §5.3).

## 5. Fases

### 5.1 Fase 0 — Conjunto de avaliação (medir)

**Objetivo:** saber, com números, quanto o Laya acerta e quão honesta é a
confiança dele.

1. Montar `backend/tests/fixtures/laya/device_identity.jsonl`: uma linha por
   host com `facts` (o `DeviceFacts` serializado) e `expected` (o tipo
   correto). Fontes:
   - dispositivos já cadastrados que vieram da Descoberta (o tipo declarado
     pelo operador é o rótulo);
   - os casos deste documento (bpi-r3, RB922 com OpenWrt, câmeras XMEye, IoT
     Tuya/ESPHome, MikroTik, nobreak Volt).
   Anonimizar: trocar IP, MAC e nomes próprios por marcadores.
2. Teste ignorado por padrão que roda o conjunto contra um Ollaya real:
   `cargo test -- --ignored laya_avaliacao` (com `OLLAYA_URL`). No CI fica só o
   teste com o Ollaya falso, que garante o formato.
3. Métricas, gravadas a cada rodada:
   - **acurácia** geral e por tipo;
   - **cobertura**: quantos hosts em dúvida recebem palpite acima do mínimo;
   - **"confiante e errado"**: palpites ≥ 90% que erraram — a métrica que o
     caso NAS mostra ser a mais cara para a confiança do operador;
   - **calibração (ECE)**: a diferença média entre a confiança declarada e a
     acurácia real, por faixa de confiança.

**Critério de aceite:** relatório com as quatro métricas para o modelo atual.
É a linha de base das fases seguintes.

### 5.2 Fase 1 — Estado e critérios (sem mexer no modelo) — ✅ iniciada

Continuar o que a §3 começou, sempre medindo contra a fase 0:

- **Critérios contrastivos.** Cada descrição diz o que distingue o tipo dos
  vizinhos ("NAS só com evidência de armazenamento"), não só exemplos de
  marca.
- **Evidência derivada no estado.** Levar ao modelo o que a heurística já
  sabe e o texto cru não diz: assinatura de kernel, "é o gateway da rede",
  "MAC aleatório", fabricante pelo registro do IEEE.
- **Ordem dos campos.** O estado é cortado em 3.000 caracteres: os campos mais
  discriminantes (serviços, `sysDescr`, título da página, UPnP) vêm primeiro.

### 5.3 Fase 2 — Retorno do operador (rótulos)

**Objetivo:** cada correção do operador vira um exemplo rotulado.

1. Tabela `laya_feedback` (migration nova, entra no `CREATION_ORDER` e fica
   fora do backup): `facts` (JSON), `heuristic_type`, `laya_type`,
   `laya_confidence`, `chosen_type`, `source` (`cadastro` ou `correcao`),
   `created_at`.
2. **Coleta implícita:** ao cadastrar um equipamento vindo da Descoberta, gravar
   o que a heurística e o Laya disseram e o tipo que o operador escolheu.
3. **Coleta explícita:** um "O Laya errou?" no chip de sugestão, com o tipo
   correto — uma correção vale mais que dez confirmações.
4. **Exportação:** `backend-cli task laya_export_feedback` gera JSONL
   anonimizado (sem IP, MAC ou nome; `sysName` e `hostname` trocados por
   marcadores), para alimentar a fase 0 e a fase 4.

**Critério de aceite:** 100% dos cadastros vindos da Descoberta geram uma
linha; a exportação não contém IP, MAC nem nomes (teste de snapshot `insta`).

### 5.4 Fase 3 — Memória da instalação (few-shot)

**Objetivo:** a rede de cada cliente ensina o Laya daquela rede.

1. Antes de perguntar ao modelo, procurar exemplos confirmados **parecidos**:
   mesmo `sysObjectID`, mesmo bloco do IEEE (OUI) ou o mesmo conjunto de
   portas.
2. Se três ou mais exemplos concordam, sugerir o tipo como "aprendido nesta
   rede" — com a contagem como evidência e sem chamar o modelo.
3. Se não, anexar até três exemplos rotulados ao estado ("na sua rede, hosts
   assim foram cadastrados como …"), dentro do limite de caracteres.

**Critério de aceite:** na fase 0 com memória, acurácia ≥ a da linha de base
e "confiante e errado" menor; nenhum exemplo de outra instalação é usado.

### 5.5 Fase 4 — Ajuste fino do modelo

Só depois das fases 2 e 3, com dados de várias instalações cedidos
explicitamente:

1. **Dados:** os JSONL exportados mais exemplos sintéticos gerados a partir das
   tabelas de assinaturas (`fingerprints.rs`): portas, serviços mDNS, tipos
   UPnP, OIDs. Mínimo de ~50 exemplos reais por tipo, com as classes
   balanceadas.
2. **Treino:** adaptador (LoRA) ou cabeça de classificação sobre o
   `laya:multilingual`, conforme o Ollaya suportar. Separar 20% para
   validação.
3. **Publicação:** um modelo próprio (`laya:netmonitor-device`), escolhido nas
   configurações da IA, com o atual como reserva.

**Critério de aceite:** no conjunto da fase 0, acurácia ≥ linha de base + 10
pontos, ECE ≤ 0,05, zero "confiante e errado" nos casos fixados nos testes e
latência na CPU dentro do limite atual.

### 5.6 Fase 5 — Calibração contínua

- **Temperatura por tipo**, ajustada a partir do retorno da fase 2: se o tipo
  X acerta 70% quando diz 90%, a confiança mostrada é corrigida.
- **Limiar por tipo**, no lugar do mínimo único de 60%.
- **Vigilância de deriva:** a taxa de correções por semana. Se subir, o
  conjunto da fase 0 ganha os casos novos e o ciclo recomeça.

## 6. Como acompanhar

| Fase | Situação |
|---|---|
| 0 — Conjunto de avaliação | Planejada |
| 1 — Estado e critérios | Iniciada (§3) |
| 2 — Retorno do operador | Planejada |
| 3 — Memória da instalação | Planejada |
| 4 — Ajuste fino | Planejada |
| 5 — Calibração contínua | Planejada |

Relacionados: [arquitetura.md §11-C](arquitetura.md),
[roadmap.md](roadmap.md), `services/discovery/fingerprints.rs` (as assinaturas
determinísticas).
