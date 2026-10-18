import { reactive, readonly } from 'vue'
import type { CredentialView } from '@/bindings/CredentialView'
import type { TransportKind } from '@/bindings/TransportKind'
import { confirm } from '@/composables/useConfirm'
import { useAuthStore } from '@/stores/auth'
import { usePluginsStore } from '@/stores/plugins'
import { accessReady, missingAccessOf, transportLabel } from '@/utils/deviceCredentials'

/**
 * O acesso do equipamento pedido ali mesmo, de qualquer tela (ADR 012): o
 * teste de enlace, o aplicativo de frota, a execução de um plugin, a ativação
 * do log, a IA. Um diálogo só (`DeviceAccessHost`, montado no `App.vue`) e
 * três formas de chamá-lo:
 *
 * * `requestAccess` — abre o formulário (já se sabe que falta);
 * * `ensureAccess` — confere e só abre se faltar;
 * * `withAccess` — roda a ação; se o backend responder "falta o acesso"
 *   (`missing_credential`), pede o acesso e tenta de novo — direto numa
 *   leitura, com mais uma confirmação numa escrita.
 */

export interface AccessRequest {
  deviceId: number
  /** O transporte que falta; `null` deixa escolher (cadastro avulso). */
  transport: TransportKind | null
  /** Por que o acesso é pedido agora — vai no topo do formulário. */
  reason?: string
}

interface AccessState {
  open: boolean
  request: AccessRequest | null
}

const state = reactive<AccessState>({ open: false, request: null })
let pending: ((saved: CredentialView | null) => void) | null = null

/** Abre o formulário. Resolve com a credencial salva, ou `null` em "Agora não". */
export function requestAccess(request: AccessRequest): Promise<CredentialView | null> {
  // Um pedido novo encerra o anterior como "não salvo".
  pending?.(null)
  state.request = request
  state.open = true
  return new Promise((resolve) => {
    pending = resolve
  })
}

/** O diálogo terminou (salvou ou foi fechado). Só o `DeviceAccessHost` chama. */
export function settleAccess(saved: CredentialView | null): void {
  state.open = false
  const resolve = pending
  pending = null
  resolve?.(saved)
}

/**
 * Garante o acesso por `transport`: pronto, segue sem perguntar nada; falta,
 * abre o formulário. `false` quando continua faltando (cancelado, ou o perfil
 * não pode cadastrar).
 */
export async function ensureAccess(
  deviceId: number,
  transport: TransportKind,
  reason?: string
): Promise<boolean> {
  const access = await usePluginsStore().fetchAccess(deviceId)
  if (accessReady(access.credentials, transport)) return true
  if (!useAuthStore().canManagePlugins) return false
  return (await requestAccess({ deviceId, transport, reason })) !== null
}

export interface WithAccessOptions {
  /**
   * `auto`: a ação só lê — roda de novo assim que o acesso é salvo.
   * `confirm`: a ação altera algo — pergunta antes de rodar de novo.
   */
  retry: 'auto' | 'confirm'
  /** O que roda, para a pergunta ("Executar «Reiniciar» de novo?"). */
  what: string
  /** A credencial acabou de ser salva (quem guarda uma cópia local atualiza). */
  onSaved?: (credential: CredentialView) => void
}

/**
 * Roda `action`; se faltar o acesso, pede-o e tenta mais uma vez. Outro erro,
 * ou o formulário fechado sem salvar, sobe como veio. `null` só quando o
 * acesso foi salvo e o operador preferiu não repetir a escrita.
 */
export function withAccess<T>(
  action: () => Promise<T>,
  options: WithAccessOptions & { retry: 'auto' }
): Promise<T>
export function withAccess<T>(
  action: () => Promise<T>,
  options: WithAccessOptions
): Promise<T | null>
export async function withAccess<T>(
  action: () => Promise<T>,
  options: WithAccessOptions
): Promise<T | null> {
  try {
    return await action()
  } catch (err: unknown) {
    const missing = missingAccessOf(err)
    if (!missing || !useAuthStore().canManagePlugins) throw err
    const saved = await requestAccess({
      deviceId: missing.deviceId,
      transport: missing.transport,
      reason: err instanceof Error ? err.message : undefined,
    })
    if (!saved) throw err
    options.onSaved?.(saved)
    if (options.retry === 'confirm') {
      const again = await confirm({
        title: 'Acesso salvo',
        message: `O acesso ${transportLabel(missing.transport)} foi salvo. Executar «${options.what}» agora?`,
        confirmText: 'Executar',
        confirmColor: 'warning',
      })
      if (!again) return null
    }
    return action()
  }
}

export function useDeviceAccess() {
  return {
    state: readonly(state),
    requestAccess,
    ensureAccess,
    withAccess,
    settleAccess,
  }
}
