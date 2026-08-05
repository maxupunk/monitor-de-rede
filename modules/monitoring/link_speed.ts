/**
 * Normalização e formatação da velocidade negociada de um link.
 *
 * `ifSpeed` é um contador de 32 bits: agentes que não expõem `ifHighSpeed`
 * devolvem o teto (4.294.967.295) para links acima de ~4,29 Gbps. Tratar esse
 * valor como velocidade real produzia falso downgrade/upgrade sempre que a
 * leitura alternava entre o teto e o valor verdadeiro.
 */
const IF_SPEED_SATURATED = 4_294_967_295

/** Converte a leitura crua em bps utilizável, ou `null` quando não é conclusiva. */
export function normalizeSpeed(bps: number | string | null | undefined): number | null {
  if (bps === null || bps === undefined) return null

  const value = Number(bps)
  if (!Number.isFinite(value) || value <= 0) return null
  if (value >= IF_SPEED_SATURATED) return null

  return value
}

export function formatSpeed(bps: number | null | undefined): string {
  if (bps === null || bps === undefined || Number.isNaN(bps) || bps <= 0) {
    return 'Desconhecido'
  }
  if (bps >= 1_000_000_000) {
    const gbps = bps / 1_000_000_000
    return `${Number.isInteger(gbps) ? gbps : gbps.toFixed(1)} Gbps`
  }
  if (bps >= 1_000_000) {
    const mbps = bps / 1_000_000
    return `${Number.isInteger(mbps) ? mbps : mbps.toFixed(1)} Mbps`
  }
  if (bps >= 1_000) {
    const kbps = bps / 1_000
    return `${Number.isInteger(kbps) ? kbps : kbps.toFixed(1)} Kbps`
  }
  return `${bps} bps`
}
