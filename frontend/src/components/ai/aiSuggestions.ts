/** Pergunta pronta oferecida quando a conversa está vazia. */
export interface AiSuggestion {
  title: string
  prompt: string
  icon: string
  color: string
}

/**
 * Sugestões do chat, em ordem de utilidade: o painel lateral mostra as
 * primeiras, a tela cheia mostra todas.
 */
export const AI_SUGGESTIONS: AiSuggestion[] = [
  {
    title: 'Conectividade Internet',
    prompt: 'Testar conectividade e latência com a Internet',
    icon: 'mdi-web',
    color: 'primary',
  },
  {
    title: 'Alertas recentes',
    prompt: 'Resumir os alertas críticos recentes e apontar prováveis causas',
    icon: 'mdi-bell-alert-outline',
    color: 'warning',
  },
  {
    title: 'Interfaces com problema',
    prompt: 'Quais interfaces estão caídas ou saturadas?',
    icon: 'mdi-ethernet',
    color: 'success',
  },
  {
    title: 'Gráfico de latência',
    prompt: 'Mostrar o gráfico de latência da última hora do gateway',
    icon: 'mdi-chart-line',
    color: 'info',
  },
  {
    title: 'Dispositivos críticos',
    prompt: 'Quais equipamentos da rede estão offline ou oscilando agora?',
    icon: 'mdi-router-wireless-off',
    color: 'error',
  },
  {
    title: 'WireGuard VPN',
    prompt: 'Como provisionar e monitorar roteadores remotos via túnel VPN?',
    icon: 'mdi-shield-lock-outline',
    color: 'deep-purple',
  },
]
