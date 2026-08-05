import type DeviceInterface from '#models/device_interface'
import { formatSpeed, normalizeSpeed } from '#modules/monitoring/link_speed'
import {
  ALERT_FIELDS,
  INTERFACE_SPEED_TRANSITION,
  INTERFACE_STATUS_TRANSITION,
} from '../alert_fields.js'
import type { AlertDataset } from '../contracts/alert_evaluation.js'

/**
 * Traduz o estado de uma interface (e o que mudou desde a última coleta) para o
 * vocabulário avaliado pelas regras.
 *
 * Só publica fatos: a decisão de alertar — e com qual severidade — pertence às
 * regras cadastradas em "Regras Configuradas".
 */
export function buildInterfaceStateDataset(
  iface: DeviceInterface,
  previousOperStatus: string | null,
  previousSpeed: number | null
): AlertDataset {
  const dataset: AlertDataset = {
    [ALERT_FIELDS.interfaceName]: iface.name,
    [ALERT_FIELDS.interfaceOperStatus]: iface.operStatus ?? null,
  }

  if (previousOperStatus && iface.operStatus && previousOperStatus !== iface.operStatus) {
    dataset[ALERT_FIELDS.interfaceStatusTransition] = `${previousOperStatus}_to_${iface.operStatus}`
    dataset.interfacePreviousOperStatus = previousOperStatus
  }

  const current = normalizeSpeed(iface.speed)
  const previous = normalizeSpeed(previousSpeed)

  if (current !== null) dataset[ALERT_FIELDS.interfaceSpeedBps] = current
  if (previous !== null) dataset.interfacePreviousSpeedBps = previous

  // Compara pela velocidade formatada: variações irrelevantes de leitura
  // (1.000.000.000 vs 999.999.999 bps) não são renegociação de link.
  if (
    current !== null &&
    previous !== null &&
    current !== previous &&
    formatSpeed(previous) !== formatSpeed(current)
  ) {
    const isDowngrade = current < previous
    dataset[ALERT_FIELDS.interfaceSpeedTransition] = isDowngrade
      ? INTERFACE_SPEED_TRANSITION.downgrade
      : INTERFACE_SPEED_TRANSITION.upgrade

    if (isDowngrade) {
      dataset[ALERT_FIELDS.interfaceSpeedDropPercent] = Math.round(
        ((previous - current) / previous) * 100
      )
    }
  }

  return dataset
}

/** `true` quando o dataset descreve alguma transição (queda, retorno ou renegociação). */
export function hasInterfaceTransition(dataset: AlertDataset): boolean {
  return (
    dataset[ALERT_FIELDS.interfaceStatusTransition] !== undefined ||
    dataset[ALERT_FIELDS.interfaceSpeedTransition] !== undefined
  )
}

/**
 * `true` quando a interface melhorou no ciclo (voltou a operar ou renegociou
 * para cima). Sinaliza ao motor que os alertas abertos do escopo podem ser
 * normalizados.
 */
export function isInterfaceRecovery(dataset: AlertDataset): boolean {
  return (
    dataset[ALERT_FIELDS.interfaceStatusTransition] === INTERFACE_STATUS_TRANSITION.cameBack ||
    dataset[ALERT_FIELDS.interfaceSpeedTransition] === INTERFACE_SPEED_TRANSITION.upgrade
  )
}

/** Frase legível do que foi observado, usada como mensagem do alerta. */
export function describeInterfaceState(dataset: AlertDataset): string {
  const name = String(dataset[ALERT_FIELDS.interfaceName] ?? 'desconhecida')
  const parts: string[] = []

  const statusTransition = dataset[ALERT_FIELDS.interfaceStatusTransition]
  if (statusTransition) {
    const previous = String(dataset.interfacePreviousOperStatus ?? '').toUpperCase()
    const current = String(dataset[ALERT_FIELDS.interfaceOperStatus] ?? '').toUpperCase()
    parts.push(`Interface ${name} alterou status: ${previous} ➔ ${current}`)
  }

  const speedTransition = dataset[ALERT_FIELDS.interfaceSpeedTransition]
  if (speedTransition) {
    const previous = formatSpeed(dataset.interfacePreviousSpeedBps as number)
    const current = formatSpeed(dataset[ALERT_FIELDS.interfaceSpeedBps] as number)
    parts.push(
      speedTransition === INTERFACE_SPEED_TRANSITION.downgrade
        ? `Interface ${name} sofreu downgrade de velocidade: ${previous} ➔ ${current}`
        : `Interface ${name} renegociou velocidade: ${previous} ➔ ${current}`
    )
  }

  if (parts.length === 0) {
    const status = String(dataset[ALERT_FIELDS.interfaceOperStatus] ?? 'desconhecido').toUpperCase()
    const speed = formatSpeed(dataset[ALERT_FIELDS.interfaceSpeedBps] as number)
    parts.push(`Interface ${name} em ${status} negociada a ${speed}`)
  }

  return parts.join(' | ')
}
