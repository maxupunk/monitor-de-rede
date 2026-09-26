/** Rótulo, ícone e cor de cada ferramenta da IA exibida no chat. */
export interface AiToolMeta {
  label: string
  icon: string
  color: string
}

const TOOL_META: Record<string, AiToolMeta> = {
  get_system_summary: {
    label: 'Resumo da Infraestrutura',
    icon: 'mdi-chart-box-outline',
    color: 'cyan',
  },
  list_devices: { label: 'Consulta de Dispositivos', icon: 'mdi-devices', color: 'blue' },
  get_device_detail: {
    label: 'Detalhes do Dispositivo',
    icon: 'mdi-information-outline',
    color: 'indigo',
  },
  get_device_interfaces: {
    label: 'Status das Interfaces',
    icon: 'mdi-ethernet',
    color: 'blue',
  },
  list_monitors: { label: 'Consulta de Monitores', icon: 'mdi-monitor-eye', color: 'indigo' },
  get_alerts: { label: 'Alertas', icon: 'mdi-bell-alert-outline', color: 'orange' },
  get_monitor_history: {
    label: 'Histórico do Monitor',
    icon: 'mdi-history',
    color: 'teal',
  },
  get_device_metrics: {
    label: 'Métricas do Dispositivo',
    icon: 'mdi-gauge',
    color: 'teal',
  },
  chart_monitor_latency: {
    label: 'Gráfico de Latência',
    icon: 'mdi-chart-line',
    color: 'primary',
  },
  chart_interface_traffic: {
    label: 'Gráfico de Tráfego',
    icon: 'mdi-chart-areaspline',
    color: 'success',
  },
  chart_device_metric: {
    label: 'Gráfico de Métrica',
    icon: 'mdi-chart-bell-curve-cumulative',
    color: 'deep-purple',
  },
  analyze_root_cause: {
    label: 'Causa Raiz (Topologia)',
    icon: 'mdi-source-branch',
    color: 'deep-orange',
  },
  compare_with_baseline: {
    label: 'Comparação com o Normal',
    icon: 'mdi-chart-bell-curve',
    color: 'indigo',
  },
  get_hourly_pattern: {
    label: 'Padrão por Hora do Dia',
    icon: 'mdi-clock-time-four-outline',
    color: 'primary',
  },
  get_incident_timeline: {
    label: 'Linha do Tempo do Incidente',
    icon: 'mdi-timeline-clock-outline',
    color: 'orange',
  },
  get_logs_overview: {
    label: 'Visão Geral dos Logs',
    icon: 'mdi-text-box-search-outline',
    color: 'blue-grey',
  },
  grep: { label: 'Busca nos Dados (grep)', icon: 'mdi-text-search', color: 'blue-grey' },
  get_docker_containers: { label: 'Containers Docker', icon: 'mdi-docker', color: 'info' },
  get_docker_hosts: { label: 'Servidores Docker', icon: 'mdi-server-network', color: 'info' },
  get_docker_usage: {
    label: 'Consumo dos Containers',
    icon: 'mdi-chart-box-outline',
    color: 'info',
  },
  docker_container_action: {
    label: 'Ação em Container',
    icon: 'mdi-restart',
    color: 'warning',
  },
  get_platform_status: {
    label: 'Estado da Plataforma',
    icon: 'mdi-view-dashboard-outline',
    color: 'blue-grey',
  },
  get_topology: { label: 'Topologia', icon: 'mdi-sitemap-outline', color: 'blue-grey' },
  ask_user: {
    label: 'Pergunta ao Usuário',
    icon: 'mdi-account-question-outline',
    color: 'primary',
  },
  acknowledge_alert: {
    label: 'Reconhecer Alerta',
    icon: 'mdi-check-circle-outline',
    color: 'primary',
  },
  silence_alert: { label: 'Silenciar Alerta', icon: 'mdi-bell-off-outline', color: 'warning' },
  create_maintenance_window: {
    label: 'Janela de Manutenção',
    icon: 'mdi-calendar-clock',
    color: 'purple',
  },
  create_monitor: { label: 'Criar Monitor', icon: 'mdi-monitor-eye', color: 'success' },
  get_alert_rules_guide: {
    label: 'Guia de Regras de Alerta',
    icon: 'mdi-book-alert-outline',
    color: 'green',
  },
  list_alert_rules: { label: 'Regras de Alerta', icon: 'mdi-bell-cog-outline', color: 'orange' },
  explain_alert: {
    label: 'Origem do Alerta',
    icon: 'mdi-bell-ring-outline',
    color: 'deep-orange',
  },
  create_alert_rule: {
    label: 'Criar Regra de Alerta',
    icon: 'mdi-bell-plus-outline',
    color: 'success',
  },
  toggle_alert_rule: {
    label: 'Ativar/Desativar Regra',
    icon: 'mdi-toggle-switch',
    color: 'warning',
  },
  delete_alert_rule: {
    label: 'Excluir Regra de Alerta',
    icon: 'mdi-bell-remove-outline',
    color: 'error',
  },
  search_system_docs: {
    label: 'Base de Conhecimento',
    icon: 'mdi-book-open-page-variant-outline',
    color: 'green',
  },
  ping_host: { label: 'ICMP Ping', icon: 'mdi-pulse', color: 'primary' },
  traceroute: { label: 'Traceroute', icon: 'mdi-routes', color: 'info' },
  scan_ports: { label: 'Scan de Portas TCP', icon: 'mdi-lan-connect', color: 'warning' },
  dns_lookup: { label: 'Resolução DNS', icon: 'mdi-dns', color: 'teal' },
  run_playbook: {
    label: 'Playbook de Diagnóstico',
    icon: 'mdi-clipboard-play-outline',
    color: 'purple',
  },
}

export function aiToolMeta(name: string): AiToolMeta {
  return TOOL_META[name] ?? { label: name, icon: 'mdi-cog-outline', color: 'grey' }
}

/** Resumo de uma linha dos argumentos, na ordem do que mais identifica a chamada. */
export function formatToolArgs(args: Record<string, unknown>): string {
  const labeled: Array<[string, string]> = [
    ['target', 'Alvo'],
    ['hostname', 'Host'],
    ['identifier', 'Dispositivo'],
    ['device', 'Dispositivo'],
    ['interface', 'Interface'],
    ['monitor_id', 'Monitor'],
    ['alert_id', 'Alerta'],
    ['rule_id', 'Regra'],
    ['name', 'Nome'],
    ['field', 'Campo'],
    ['metric', 'Métrica'],
    ['playbook_type', 'Playbook'],
    ['pattern', 'Padrão'],
    ['source', 'Fonte'],
    ['container', 'Container'],
    ['query', 'Busca'],
    ['status', 'Status'],
  ]
  const parts = labeled
    .filter(([key]) => args[key] !== undefined && args[key] !== null && args[key] !== '')
    .map(([key, label]) => `${label}: ${String(args[key])}`)
  if (parts.length > 0) return parts.slice(0, 2).join(' · ')
  const keys = Object.keys(args)
  return keys.length > 0 ? keys.join(', ') : 'Sem parâmetros adicionais'
}
