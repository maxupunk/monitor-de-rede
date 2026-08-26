/**
 * Por onde o servidor alcança um equipamento.
 *
 * O vocabulário é o mesmo do backend (`services::devices::access`). Vive aqui
 * porque três telas o mostram — o cadastro do dispositivo, a ativação de log e a
 * lista de endereços do servidor — e uma quarta lista de rótulos divergiria das
 * outras na primeira correção.
 *
 * O sentinela `auto` é um valor de verdade e não a ausência do campo: o
 * formulário manda tudo a cada gravação, e sem a palavra não haveria como
 * dizer ao servidor "volte a decidir por mim".
 */
export const AUTO_ACCESS_MODE = 'auto'

export type AccessMode = 'local' | 'vpn' | 'remote'
export type AccessModeChoice = AccessMode | typeof AUTO_ACCESS_MODE

interface AccessModeMeta {
  label: string
  icon: string
  color: string
  /** Quando usar esta forma de acesso — é a frase que carrega o conceito. */
  description: string
}

const META: Record<AccessMode, AccessModeMeta> = {
  local: {
    label: 'Rede local',
    icon: 'mdi-lan',
    color: 'primary',
    description: 'O equipamento está na mesma rede que este servidor',
  },
  vpn: {
    label: 'Túnel VPN',
    icon: 'mdi-shield-lock-outline',
    color: 'deep-purple',
    description: 'O equipamento chega por um túnel WireGuard',
  },
  remote: {
    label: 'Internet (remoto)',
    icon: 'mdi-web',
    color: 'teal',
    description: 'O equipamento está em outro local e chega pela internet',
  },
}

const AUTO_META: AccessModeMeta = {
  label: 'Automático',
  icon: 'mdi-auto-fix',
  color: 'grey',
  description: 'O sistema decide pela rota, pela VPN e pela faixa do endereço',
}

export function accessModeMeta(mode: string | null | undefined): AccessModeMeta {
  if (!mode || mode === AUTO_ACCESS_MODE) return AUTO_META
  return META[mode as AccessMode] ?? AUTO_META
}

export function accessModeLabel(mode: string | null | undefined): string {
  return accessModeMeta(mode).label
}

/**
 * As opções do seletor do cadastro.
 *
 * A primeira é sempre o automático, e o subtítulo dela mostra **o que o sistema
 * concluiu** para este equipamento. Escondê-lo faria o operador declarar no
 * escuro: ele só sabe se precisa corrigir depois de ver a conclusão.
 */
export function accessModeOptions(deduced?: {
  mode?: string | null
  reason?: string | null
}): { value: AccessModeChoice; title: string; subtitle: string; icon: string }[] {
  const conclusao =
    deduced?.mode && deduced.reason
      ? `Detectado automaticamente — ${deduced.reason}`
      : AUTO_META.description

  return [
    {
      value: AUTO_ACCESS_MODE,
      title: deduced?.mode ? accessModeLabel(deduced.mode) : AUTO_META.label,
      subtitle: conclusao,
      icon: deduced?.mode ? accessModeMeta(deduced.mode).icon : AUTO_META.icon,
    },
    ...(['local', 'vpn', 'remote'] as AccessMode[]).map((modo) => ({
      value: modo,
      title: META[modo].label,
      subtitle: META[modo].description,
      icon: META[modo].icon,
    })),
  ]
}
