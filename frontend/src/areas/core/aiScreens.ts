import type { AiScreenMeta } from '@/utils/aiScreen'

/**
 * Como as telas do Núcleo se apresentam ao chat da IA (`meta.aiScreen`). O
 * `id` é o do catálogo do backend (`crates/assistant/src/screens.rs`); as
 * sugestões aparecem primeiro no chat vazio e vão com a tela anexada.
 */
export const coreScreens = {
  dashboard: {
    id: 'dashboard',
    label: 'Dashboard',
    icon: 'mdi-view-dashboard-outline',
    suggestions: [
      {
        title: 'Resumo da rede',
        prompt:
          'Resuma o estado da rede agora: o que está fora, o que oscila e o que merece atenção',
        icon: 'mdi-clipboard-pulse-outline',
        color: 'primary',
      },
    ],
  },
  devices: {
    id: 'devices',
    label: 'Equipamentos',
    icon: 'mdi-devices',
    suggestions: [
      {
        title: 'Equipamentos com problema',
        prompt: 'Quais equipamentos estão offline ou oscilando agora, e desde quando?',
        icon: 'mdi-router-wireless-off',
        color: 'error',
      },
    ],
  },
  'device-detail': {
    id: 'device-detail',
    label: 'Equipamento',
    icon: 'mdi-router-network',
    entity: 'device',
    suggestions: [
      {
        title: 'Diagnóstico deste equipamento',
        prompt:
          'Faça um diagnóstico deste equipamento: disponibilidade, interfaces, alertas e logs recentes',
        icon: 'mdi-stethoscope',
        color: 'primary',
      },
      {
        title: 'Interfaces',
        prompt: 'Alguma interface deste equipamento está caída, com erros ou saturada?',
        icon: 'mdi-ethernet',
        color: 'info',
      },
    ],
  },
  'scheduled-tasks': {
    id: 'scheduled-tasks',
    label: 'Tarefas agendadas',
    icon: 'mdi-calendar-clock',
    suggestions: [
      {
        title: 'Tarefas com falha',
        prompt: 'Alguma tarefa agendada falhou nas últimas execuções? Por quê?',
        icon: 'mdi-calendar-alert',
        color: 'warning',
      },
    ],
  },
  monitors: {
    id: 'monitors',
    label: 'Monitores',
    icon: 'mdi-monitor-eye',
    suggestions: [
      {
        title: 'Monitores falhando',
        prompt: 'Quais monitores estão falhando agora e qual a causa provável?',
        icon: 'mdi-monitor-off',
        color: 'error',
      },
    ],
  },
  'monitor-detail': {
    id: 'monitor-detail',
    label: 'Monitor',
    icon: 'mdi-monitor-eye',
    entity: 'monitor',
    suggestions: [
      {
        title: 'Histórico deste monitor',
        prompt:
          'Como este monitor se comportou nas últimas 24 horas? Houve quedas ou picos de latência?',
        icon: 'mdi-chart-line',
        color: 'info',
      },
    ],
  },
  alerts: {
    id: 'alerts',
    label: 'Alertas',
    icon: 'mdi-bell-alert-outline',
    suggestions: [
      {
        title: 'Causa dos alertas',
        prompt: 'Resuma os alertas abertos e aponte a causa raiz provável',
        icon: 'mdi-bell-alert-outline',
        color: 'warning',
      },
      {
        title: 'Regras ruidosas',
        prompt: 'Quais regras de alerta disparam demais e como ajustá-las?',
        icon: 'mdi-tune-variant',
        color: 'info',
      },
    ],
  },
  'maintenance-windows': {
    id: 'maintenance-windows',
    label: 'Manutenção',
    icon: 'mdi-wrench-clock',
    suggestions: [
      {
        title: 'Janelas ativas',
        prompt: 'Quais janelas de manutenção estão ativas ou vão começar em breve?',
        icon: 'mdi-wrench-clock',
        color: 'info',
      },
    ],
  },
  events: {
    id: 'events',
    label: 'Eventos',
    icon: 'mdi-timeline-clock-outline',
    suggestions: [
      {
        title: 'Linha do tempo',
        prompt:
          'Monte a linha do tempo do último incidente: o que caiu primeiro e o que veio depois',
        icon: 'mdi-timeline-clock-outline',
        color: 'primary',
      },
    ],
  },
  logs: {
    id: 'logs',
    label: 'Logs',
    icon: 'mdi-text-box-search-outline',
    suggestions: [
      {
        title: 'Explicar estes logs',
        prompt: 'O que estes logs mostram? Há erros que exigem ação?',
        icon: 'mdi-text-box-search-outline',
        color: 'primary',
      },
      {
        title: 'Padrões incomuns',
        prompt:
          'Há algum padrão incomum nos logs desta janela, como picos de erro ou falhas de login?',
        icon: 'mdi-chart-bell-curve',
        color: 'warning',
      },
    ],
  },
  'remote-servers': {
    id: 'remote-servers',
    label: 'Servidores remotos',
    icon: 'mdi-server-network',
    suggestions: [
      {
        title: 'Agentes desconectados',
        prompt: 'Algum agente remoto está desconectado ou desatualizado?',
        icon: 'mdi-lan-disconnect',
        color: 'warning',
      },
    ],
  },
  audit: {
    id: 'audit',
    label: 'Auditoria',
    icon: 'mdi-clipboard-text-clock-outline',
    suggestions: [
      {
        title: 'Mudanças recentes',
        prompt: 'Quem mudou o quê nas últimas 24 horas? Alguma mudança antecede um problema?',
        icon: 'mdi-clipboard-text-clock-outline',
        color: 'info',
      },
    ],
  },
} satisfies Record<string, AiScreenMeta>
