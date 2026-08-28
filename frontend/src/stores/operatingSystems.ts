import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { OperatingSystemOption } from '@/bindings/OperatingSystemOption'
import type { IdentifyResult } from '@/bindings/IdentifyResult'

export type { OperatingSystemOption, IdentifyResult }

/** Corpo de `POST /api/devices/identify` — vem do formulário, não de um id. */
export interface IdentifyPayload {
  name?: string | null
  ipAddress: string | null
  snmpVersion: string | null
  snmpCommunity: string | null
  vendor: string | null
  model: string | null
}

/** Valor que a API aceita para "não declarei — deduza". */
export const AUTO_OPERATING_SYSTEM = 'auto'

/**
 * O catálogo de sistemas dos equipamentos.
 *
 * Vem do servidor, e não de uma constante local, porque é a **mesma** lista que
 * decide três coisas do lado de lá: quais comandos de syslog existem, se o
 * MAC-Telnet é possível e qual perfil da VPN corresponde. Uma cópia aqui
 * divergiria na primeira entrada nova — e a divergência só apareceria quando um
 * operador escolhesse a opção que o backend não conhece.
 *
 * Carrega uma vez por sessão: são sete linhas estáticas.
 */
export const useOperatingSystemsStore = defineStore('operatingSystems', () => {
  const systems = ref<OperatingSystemOption[]>([])
  const loading = ref(false)
  const identifying = ref(false)
  const loaded = ref(false)
  const error = ref('')

  /** Os que a ativação automática de log consegue configurar. */
  const withSyslog = computed(() => systems.value.filter((sistema) => sistema.supportsSyslog))

  async function fetchAll(force = false): Promise<void> {
    if (loading.value || (loaded.value && !force)) return
    loading.value = true
    error.value = ''
    try {
      systems.value = await apiService.get<OperatingSystemOption[]>('/devices/systems')
      loaded.value = true
    } catch (erro) {
      error.value = erro instanceof Error ? erro.message : 'Não foi possível carregar os sistemas.'
    } finally {
      loading.value = false
    }
  }

  /**
   * Descobre o sistema **agora**, consultando o equipamento.
   *
   * Existe porque a dedução silenciosa não dava como conferir: um OpenWrt cujo
   * agente SNMP responde só o `uname` ficava como Linux, e o campo apenas
   * afirmava isso. Aqui a conclusão vem com a evidência crua junto.
   *
   * **Não grava nada** — quem decide o que fica é o formulário.
   */
  async function identify(payload: IdentifyPayload): Promise<IdentifyResult> {
    identifying.value = true
    try {
      return await apiService.post<IdentifyResult>('/devices/identify', payload)
    } finally {
      identifying.value = false
    }
  }

  function byId(id: string | null | undefined): OperatingSystemOption | null {
    if (!id) return null
    return systems.value.find((sistema) => sistema.id === id) ?? null
  }

  function label(id: string | null | undefined): string {
    return byId(id)?.label ?? 'Sistema não identificado'
  }

  function icon(id: string | null | undefined): string {
    return byId(id)?.icon ?? 'mdi-help-circle-outline'
  }

  return {
    systems,
    loading,
    identifying,
    loaded,
    error,
    withSyslog,
    fetchAll,
    identify,
    byId,
    label,
    icon,
  }
})

/**
 * De onde veio a conclusão sobre o sistema, em português.
 *
 * As chaves são as de `services::devices::systems::source`. A frase muda a
 * confiança que o operador deposita na escolha, então cada origem tem a sua —
 * "identificado pelo SNMP" e "não foi possível identificar" não podem ler igual.
 */
export function operatingSystemSourceLabel(origem: string | null | undefined): string {
  switch (origem) {
    case 'declarado':
      return 'Definido no cadastro do dispositivo'
    case 'snmp':
      return 'Identificado pelo SNMP do equipamento'
    case 'sonda':
      return 'Identificado pela sonda do servidor SSH'
    case 'cadastro':
      return 'Deduzido do fabricante informado no cadastro'
    case 'padrão':
      return 'Não foi possível identificar — confirme antes de aplicar'
    default:
      return 'Define quais comandos serão enviados'
  }
}
