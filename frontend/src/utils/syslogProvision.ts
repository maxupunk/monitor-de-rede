interface UsableServerAddress {
  id: string
  label: string
  description: string
  value: string | null
}

export interface LogSetupTarget {
  sessionId: number
  id: number
  name: string
  host: string | null
  operatingSystem: string | null
}

interface SavedDeviceIdentity {
  id: number
  name: string
  ipAddress?: string | null
  effectiveOperatingSystem?: string | null
}

/** Cria uma fotografia independente da store para uma sessão de ativação. */
export function createLogSetupTarget(
  sessionId: number,
  device: SavedDeviceIdentity,
  selectedOperatingSystem: string,
  detectedOperatingSystem?: string | null
): Readonly<LogSetupTarget> {
  const operatingSystem =
    selectedOperatingSystem === 'auto'
      ? (detectedOperatingSystem ?? device.effectiveOperatingSystem ?? null)
      : selectedOperatingSystem
  return Object.freeze({
    sessionId,
    id: device.id,
    name: device.name,
    host: device.ipAddress?.trim() || null,
    operatingSystem,
  })
}

/** Guarda comum contra respostas assíncronas de outra abertura/dispositivo. */
export function isProvisionSessionCurrent(
  open: boolean,
  expectedSequence: number,
  currentSequence: number,
  expectedDeviceId: number,
  currentDeviceId: number
): boolean {
  return open && expectedSequence === currentSequence && expectedDeviceId === currentDeviceId
}

export interface ProvisionAddressOption {
  value: string
  title: string
  subtitle: string
  suggested: boolean
}

interface ProvisionAddressHint {
  value?: string | null
  label?: string | null
  description?: string | null
}

/** Host externo visto pelo navegador; sobrevive ao proxy que troca o `Host`. */
export function observedApplicationAddress(
  location: Pick<Location, 'hostname'> | null = typeof window === 'undefined'
    ? null
    : window.location
): string | null {
  const hostname = location?.hostname.trim() ?? ''
  return hostname || null
}

/** Mantém o snapshot enquanto a nova sonda ainda não trouxe um sistema válido. */
export function resolveProvisionOperatingSystem(
  hinted: string | null | undefined,
  snapshot: string | null | undefined
): string {
  return hinted?.trim() || snapshot?.trim() || ''
}

/** Converte a emissão livre ou estruturada do `v-combobox` em um único texto. */
export function normalizeComboboxAddress(value: unknown): string {
  if (typeof value === 'string') return value.trim()
  if (value && typeof value === 'object' && 'value' in value) {
    const nested = (value as { value?: unknown }).value
    return typeof nested === 'string' ? nested.trim() : ''
  }
  return ''
}

/** Comparação usada apenas na apresentação; a normalização canônica é do backend. */
export function sameProvisionAddress(
  left: string | null | undefined,
  right: string | null | undefined
): boolean {
  const normalizedLeft = left?.trim().toLocaleLowerCase() ?? ''
  const normalizedRight = right?.trim().toLocaleLowerCase() ?? ''
  return Boolean(normalizedLeft) && normalizedLeft === normalizedRight
}

/**
 * Opções conhecidas do campo único, sem duplicatas e com a recomendação atual
 * primeiro. O `value` é o endereço real para a seleção e o texto livre terem o
 * mesmo formato de saída.
 */
export function buildProvisionAddressOptions(
  usable: UsableServerAddress[],
  suggestedAddressId: string | null | undefined,
  hinted?: ProvisionAddressHint | null
): ProvisionAddressOption[] {
  const seen = new Set<string>()
  const options = usable
    .map((entry, index) => ({ entry, index }))
    .sort((left, right) => {
      const leftSuggested = left.entry.id === suggestedAddressId ? 0 : 1
      const rightSuggested = right.entry.id === suggestedAddressId ? 0 : 1
      return leftSuggested - rightSuggested || left.index - right.index
    })
    .flatMap(({ entry }) => {
      const value = entry.value?.trim() ?? ''
      const normalized = value.toLocaleLowerCase()
      if (!normalized || seen.has(normalized)) return []
      seen.add(normalized)
      const suggested = entry.id === suggestedAddressId
      return [
        {
          value,
          title: `${entry.label} — ${value}`,
          subtitle: suggested
            ? `Sugerido automaticamente — ${entry.description}`
            : entry.description,
          suggested,
        },
      ]
    })

  const hintedValue = hinted?.value?.trim() ?? ''
  const hintedKey = hintedValue.toLocaleLowerCase()
  if (hintedValue && !seen.has(hintedKey)) {
    options.unshift({
      value: hintedValue,
      title: `${hinted?.label?.trim() || 'Detectado automaticamente'} — ${hintedValue}`,
      subtitle: hinted?.description?.trim() || 'Sugerido automaticamente pelo backend',
      suggested: true,
    })
  }
  return options
}
