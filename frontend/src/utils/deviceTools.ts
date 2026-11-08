/**
 * Ferramentas do cabeçalho de `/devices/{id}`: o que dá para fazer com o
 * equipamento agora, agrupado pelo propósito. A página só diz o que o
 * equipamento permite; aqui mora o rótulo, o ícone, a explicação e a ordem —
 * um lugar só, testável sem montar a tela.
 */

export type DeviceToolKey =
  'snmpScan' | 'snmpCollect' | 'quickPing' | 'traceroute' | 'ports' | 'playbook'

export interface DeviceTool {
  key: DeviceToolKey
  label: string
  icon: string
  /** O que a ferramenta faz, em uma frase (tooltip). */
  hint: string
}

export interface DeviceToolGroup {
  key: 'snmp' | 'diagnostics'
  title: string
  icon: string
  tools: DeviceTool[]
}

/** O que o equipamento e o usuário permitem — vem das capacidades da API. */
export type DeviceToolAccess = Record<DeviceToolKey, boolean>

const GROUPS: DeviceToolGroup[] = [
  {
    key: 'snmp',
    title: 'Monitoramento SNMP',
    icon: 'mdi-chart-timeline-variant',
    tools: [
      {
        key: 'snmpScan',
        label: 'Configurar',
        icon: 'mdi-radar',
        hint: 'Varre o equipamento via SNMP e escolhe o que monitorar (interfaces, CPU, memória e sensores).',
      },
      {
        key: 'snmpCollect',
        label: 'Coletar agora',
        icon: 'mdi-refresh',
        hint: 'Lê agora as métricas do que já está monitorado, sem mudar a configuração.',
      },
    ],
  },
  {
    key: 'diagnostics',
    title: 'Diagnóstico de rede',
    icon: 'mdi-stethoscope',
    tools: [
      {
        key: 'quickPing',
        label: 'Ping rápido',
        icon: 'mdi-pulse',
        hint: 'Latência, perda e fila em ~4 s (estimativa de baixa precisão).',
      },
      {
        key: 'traceroute',
        label: 'Traceroute',
        icon: 'mdi-routes',
        hint: 'Mostra cada salto do caminho até o equipamento (ICMP).',
      },
      {
        key: 'ports',
        label: 'Portas',
        icon: 'mdi-lan-connect',
        hint: 'Descobre as portas TCP/UDP abertas no equipamento.',
      },
      {
        key: 'playbook',
        label: 'Playbook',
        icon: 'mdi-clipboard-pulse-outline',
        hint: 'Roda o roteiro completo de conectividade e aponta onde está o problema.',
      },
    ],
  },
]

/** Os grupos com as ferramentas permitidas; grupo sem nenhuma some. */
export function deviceToolGroups(access: DeviceToolAccess): DeviceToolGroup[] {
  return GROUPS.map((group) => ({
    ...group,
    tools: group.tools.filter((tool) => access[tool.key]),
  })).filter((group) => group.tools.length > 0)
}
