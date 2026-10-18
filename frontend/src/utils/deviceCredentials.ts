import type { CredentialView } from '@/bindings/CredentialView'
import type { TransportKind } from '@/bindings/TransportKind'
import { ApiError, MISSING_CREDENTIAL_CODE } from '@/services/apiService'

/**
 * As credenciais do equipamento moram num lugar só — a aba "Plugins ›
 * Credenciais". A ativação do syslog lê e grava ali; estas regras são as
 * mesmas nas duas telas para uma não dizer "pronta" onde a outra diz "falta a
 * senha".
 */

/** Tipos que abrem um shell no equipamento — os únicos que ativam o log. */
const SHELL_KINDS: readonly TransportKind[] = ['ssh', 'telnet']

/**
 * Onde o login de um meio de acesso fica salvo. Igual ao
 * `Protocol::credential_kind` do backend: o MAC-Telnet entra com o usuário do
 * Telnet, só muda como o equipamento é endereçado.
 */
export function credentialKindFor(protocol: string): TransportKind {
  return protocol === 'ssh' ? 'ssh' : 'telnet'
}

/** Há senha pronta para uso agora, sem perguntar nada ao operador. */
export function credentialReady(credential: CredentialView): boolean {
  return credential.storage === 'vault' ? credential.hasStoredSecret : credential.sessionActive
}

export function credentialSecretLabel(credential: CredentialView): string {
  if (credential.storage === 'vault') {
    return credential.hasStoredSecret ? 'senha guardada cifrada' : 'sem senha guardada'
  }
  return credential.sessionActive ? 'senha da sessão informada' : 'senha pedida a cada sessão'
}

export function credentialSubtitle(credential: CredentialView): string {
  const where = credential.viaProbeId
    ? `via agente #${credential.viaProbeId}`
    : 'a partir da central'
  const https = credential.https ? ' (HTTPS)' : ''
  return `porta ${credential.port}${https} · ${credentialSecretLabel(credential)} · ${where}`
}

/**
 * A credencial que serve para entrar no equipamento por `protocol`.
 *
 * A do próprio tipo vem antes, e uma pronta vence uma que ainda pede senha: o
 * usuário de administração costuma ser o mesmo no SSH e no Telnet, então a do
 * outro tipo ainda poupa a digitação. `null` quando não há nenhuma de shell.
 */
export function pickShellCredential(
  credentials: readonly CredentialView[],
  protocol: string
): CredentialView | null {
  const preferred = credentialKindFor(protocol)
  const rank = (credential: CredentialView): number =>
    (credentialReady(credential) ? 0 : 2) + (credential.kind === preferred ? 0 : 1)
  return (
    credentials
      .filter((credential) => SHELL_KINDS.includes(credential.kind))
      .sort((a, b) => rank(a) - rank(b))[0] ?? null
  )
}

const TRANSPORT_LABELS: Record<TransportKind, string> = {
  ssh: 'SSH',
  telnet: 'Telnet',
  http: 'HTTP (interface web)',
}

/** "SSH", "Telnet", "HTTP (interface web)". */
export function transportLabel(kind: TransportKind): string {
  return TRANSPORT_LABELS[kind]
}

/** Título do formulário pelo que falta: "Acesso SSH". */
export function accessTitle(kind: TransportKind): string {
  return `Acesso ${transportLabel(kind)}`
}

/** A credencial do tipo, quando há. */
export function credentialOf(
  credentials: readonly CredentialView[],
  kind: TransportKind
): CredentialView | null {
  return credentials.find((credential) => credential.kind === kind) ?? null
}

/**
 * O acesso por `kind` está pronto — o mesmo critério do backend
 * (`credentials::gap_in`): HTTP sem login vale; SSH/Telnet precisam de senha.
 */
export function accessReady(credentials: readonly CredentialView[], kind: TransportKind): boolean {
  const credential = credentialOf(credentials, kind)
  if (!credential) return kind === 'http'
  return credentialReady(credential)
}

/** O equipamento e o transporte cujo acesso falta. */
export interface MissingAccess {
  deviceId: number
  transport: TransportKind
}

const TRANSPORTS: readonly TransportKind[] = ['ssh', 'telnet', 'http']

function isTransport(value: unknown): value is TransportKind {
  return typeof value === 'string' && (TRANSPORTS as readonly string[]).includes(value)
}

/**
 * O erro é "falta o acesso" (`code: "missing_credential"` no corpo da API, o
 * `AppError::MissingCredential` do backend)? Devolve o que pedir; `null` para
 * qualquer outro erro. É o único lugar que reconhece esse erro.
 */
export function missingAccessOf(err: unknown): MissingAccess | null {
  if (!(err instanceof ApiError)) return null
  const body = err.response
  if (body?.code !== MISSING_CREDENTIAL_CODE) return null
  return typeof body.deviceId === 'number' && isTransport(body.transport)
    ? { deviceId: body.deviceId, transport: body.transport }
    : null
}

/** O mesmo `missingAccess` dentro de um resultado guardado (ferramenta da IA). */
export function missingAccessIn(value: unknown): MissingAccess | null {
  if (typeof value !== 'object' || value === null) return null
  const access = (value as Record<string, unknown>).missingAccess
  if (typeof access !== 'object' || access === null) return null
  const { deviceId, transport } = access as Record<string, unknown>
  return typeof deviceId === 'number' && isTransport(transport) ? { deviceId, transport } : null
}

/** "Para gerenciar AP-terraço o NetMonitor precisa do acesso SSH dele." */
export function accessNeededReason(deviceName: string, kind: TransportKind): string {
  return `Para gerenciar ${deviceName} o NetMonitor precisa do acesso ${transportLabel(kind)} dele.`
}

/**
 * O que o chat diz ao assistente depois que o acesso que faltava foi salvo
 * no cartão da ferramenta: ele tenta de novo sozinho.
 */
export function accessSavedFollowUp(kind: TransportKind, deviceName: string | null): string {
  const device = deviceName ? `de ${deviceName}` : 'do equipamento'
  return `Cadastrei o acesso ${transportLabel(kind)} ${device}; pode tentar de novo.`
}
