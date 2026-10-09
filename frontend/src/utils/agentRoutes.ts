import type { AgentRouteOption } from '@/bindings/AgentRouteOption'

/** Uma opção do seletor "Acessar a partir de". */
export interface AgentRouteItem {
  title: string
  value: number | null
  props: { disabled: boolean; subtitle: string }
}

/**
 * Itens do seletor "Acessar a partir de": a central e cada agente, com o
 * motivo quando um agente não serve.
 *
 * Desconectado continua escolhível — a rota vale para quando ele voltar —,
 * mas sem a permissão não: escolher levaria a uma falha certa no primeiro uso.
 *
 * `permission` é o nome no `AGENT_ALLOW` (`device_io`, `database`).
 */
export function agentRouteItems(
  agents: AgentRouteOption[],
  permission: string,
  centralSubtitle: string
): AgentRouteItem[] {
  return [
    {
      title: 'Central (este servidor)',
      value: null,
      props: { disabled: false, subtitle: centralSubtitle },
    },
    ...agents.map((agent) => {
      const reason = !agent.allowed
        ? `sem '${permission}' no AGENT_ALLOW do host`
        : agent.connected
          ? 'conectado'
          : 'desconectado agora'
      return {
        title: `Agente ${agent.name}`,
        value: agent.id,
        props: { disabled: !agent.allowed, subtitle: reason },
      }
    }),
  ]
}
