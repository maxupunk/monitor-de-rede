# Regras de alerta — funcionamento e criação

## Como um alerta nasce
1. Cada checagem (monitor, coleta SNMP, túnel VPN, padrão de log) publica **fatos** com nomes fixos (`latencyMs`, `status`, `cpuUsagePercent`...).
2. Cada **regra** ativa cujo escopo cobre o alvo compara um fato: `{field, operator, value}`.
3. Condição verdadeira por `duration_seconds` seguidos → abre um **alerta** (`alert_events`) ligado à regra (`alert_rule_id`) e ao alvo (`scope_key`: `monitor:12`, `interface:34`, `vpn_peer:7`). Um alvo tem no máximo um alerta aberto por regra.
4. Notifica pelos canais configurados (Web Push, Telegram, Discord, webhook), respeitando silêncio, janela de manutenção e cooldown.

## Ciclo de vida
- `active` → `acknowledged` (alguém está ciente) ou `silenced` (sem notificar por N min).
- Alvo voltou ao normal → `recovering` durante `recovery_window_seconds`; recaída volta a `active`; sem recaída → `resolved`.
- Recaídas ≥ `flap_threshold` dentro de `flap_window_seconds` → `flapping` (instável crônico, notifica uma vez só).

## Campos da regra
- `name` (obrigatório) e `severity`: `critical`, `warning` (padrão) ou `info`.
- Escopo: `device_id`, `monitor_id` e/ou `site_id`. Nenhum = **global** (vale para todo o parque). Prefira escopo específico para limiares que dependem do equipamento.
- `duration_seconds`: quanto tempo a condição precisa se manter (0 = dispara na primeira amostra). Use 60–300 s para evitar alarme por pico isolado.
- `recovery_window_seconds`: estabilidade exigida antes de resolver (0 = resolve na primeira amostra boa).
- `flap_threshold` / `flap_window_seconds`: detecção de oscilação (0 = desligada; janela padrão 900 s).
- `notification_cooldown_seconds`: intervalo mínimo entre notificações do mesmo par regra/alvo.
- `inhibit_when_parent_down`: não notificar se o equipamento pai já está em alerta (evita avalanche atrás de um uplink caído).

## Operadores
`eq`, `neq`, `gt`, `gte`, `lt`, `lte` (números ou textos numéricos) e `contains` (texto).

## Fatos (condition.field)
- Monitor: `status` (`up`|`down`|`warning`|`unknown`), `latencyMs`, `packetLoss` (%), `statusCode` (HTTP), `durationMs`, `connectTimeMs`, `resolutionTimeMs` (DNS), `reachabilityCause`.
- SNMP pontual do monitor: `ifOperStatus` (1 up, 2 down), `ifSpeed`, `snmpUptime`, `inBps`, `outBps`.
- Saúde do equipamento: `cpuUsagePercent`, `memoryUsedPercent`, `storageUsedPercent` (0–100), `loadAverage1m`.
- Interfaces SNMP: `interfaceName`, `interfaceOperStatus`, `interfaceStatusTransition` (`up_to_down`|`down_to_up`), `interfaceSpeedBps`, `interfaceSpeedTransition` (`downgrade`|`upgrade`), `interfaceSpeedDropPercent`.
- Túneis VPN: `vpnPeerName`, `vpnPeerStatus` (`connected`|`unstable`|`disconnected`|`awaiting`), `vpnStatusTransition` (`connected_to_disconnected`|`connected_to_unstable`|`reconnected`), `vpnSecondsSinceActivity`.
- Logs (syslog): `logPatternKey`, `logMatchCount` (ocorrências na janela), `logWindowSeconds`, `logSeverity` (0 = emergência … 7 = debug), `logMessage`.
- Anomalia estatística (comparação com o normal do próprio alvo): `latencyZScore`, `latencyDeviationPercent`, `latencyUpperBandMs`, `packetLossZScore`, `packetLossDeviationPercent`, `uptimeZScore`, `uptimeDeviationPercent`, `syslogVolumeZScore`, `trafficInZScore`, `trafficOutZScore`. Z-score ≥ 3 é um desvio forte.

## Receitas
- Host fora do ar: `status eq "down"`, duração 60 s, `critical`.
- Latência alta: `latencyMs gt 150`, duração 120 s, `warning`, no dispositivo.
- Perda de pacotes: `packetLoss gte 20`, duração 120 s, `warning`.
- CPU alta: `cpuUsagePercent gte 90`, duração 300 s, `warning`.
- Disco cheio: `storageUsedPercent gte 90`, `critical`.
- Uplink caiu: `interfaceStatusTransition eq "up_to_down"`, `critical`.
- Link renegociou mais lento: `interfaceSpeedTransition eq "downgrade"`, `warning`.
- Túnel VPN caiu: `vpnStatusTransition eq "connected_to_disconnected"`, `critical`.
- Latência fora do normal: `latencyZScore gte 3`, duração 300 s, `info`.
- Site HTTP com erro: `statusCode gte 500`, `critical`.

## Diagnosticar de onde vem um alerta
1. `explain_alert` com o id: traz a regra que disparou, a condição, o alvo (monitor, interface, túnel), os fatos que casaram e quantas vezes a regra disparou nas últimas 24 h.
2. Regra ruidosa (muitos disparos, duração curta)? Sugira aumentar `duration_seconds`, `recovery_window_seconds` ou ligar o flapping — não apagar.
3. Problema real? Siga com causa raiz, linha do tempo e testes ativos no alvo.
4. Alvo em manutenção programada? Proponha janela de manutenção.

## Criar e excluir pela IA
- `create_alert_rule` e `delete_alert_rule` sempre pedem confirmação do usuário no chat.
- **Excluir apaga também o histórico de alertas da regra** (cascata no banco). Quando o objetivo é só parar de ser avisado, desativar a regra na tela /alerts preserva o histórico — mencione essa alternativa.
- Antes de criar, confira com `list_alert_rules` se já existe regra igual para o mesmo escopo.
