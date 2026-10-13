import type { CredentialView } from '@/bindings/CredentialView'
import type { TransportKind } from '@/bindings/TransportKind'

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
