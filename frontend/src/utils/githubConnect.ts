/**
 * "Conectar GitHub" (ADR 021): a ida ao GitHub e a volta.
 *
 * A ida é um formulário enviado ao GitHub com o manifest do app — navegação
 * de página inteira, porque é o GitHub quem mostra a tela de confirmação. A
 * volta chega em `/deploys/github` com `code` (app criado) ou
 * `installation_id` (app instalado). O que a pessoa fazia antes de sair
 * (o assistente aberto) fica guardado na aba, para a tela retomar dali.
 */
import type { DeployGithubManifestView } from '@/bindings/DeployGithubManifestView'

const RETURN_KEY = 'deploys:github-return'
const CENTRAL_URL_KEY = 'deploys:central-url'

/** Onde retomar depois do GitHub. */
export type GithubReturn = 'setup' | 'connections'

/** O que o GitHub mandou de volta na URL. */
export type GithubCallback =
  { kind: 'created'; code: string; state: string } | { kind: 'installed'; installationId: number }

type QueryValue = string | null | (string | null)[] | undefined

function single(value: QueryValue): string | null {
  const first = Array.isArray(value) ? value[0] : value
  return typeof first === 'string' && first.trim() !== '' ? first.trim() : null
}

/** Lê o retorno do GitHub da query da rota; `null` quando não é um. */
export function parseGithubCallback(query: Record<string, QueryValue>): GithubCallback | null {
  const code = single(query.code)
  const state = single(query.state)
  if (code && state) return { kind: 'created', code, state }
  const installation = Number(single(query.installation_id))
  if (Number.isInteger(installation) && installation > 0) {
    return { kind: 'installed', installationId: installation }
  }
  return null
}

/** As chaves da query que o retorno do GitHub usa (para limpar a URL). */
export const GITHUB_CALLBACK_KEYS = ['code', 'state', 'installation_id', 'setup_action'] as const

export function rememberGithubReturn(target: GithubReturn): void {
  sessionStorage.setItem(RETURN_KEY, target)
}

/** O lugar a retomar — uma vez só. */
export function takeGithubReturn(): GithubReturn | null {
  const value = sessionStorage.getItem(RETURN_KEY)
  sessionStorage.removeItem(RETURN_KEY)
  return value === 'setup' || value === 'connections' ? value : null
}

/**
 * O endereço público da central, que o workflow chama. Começa pelo endereço
 * da aba; quem acessa por IP interno troca uma vez e fica lembrado.
 */
export function storedCentralUrl(): string {
  return localStorage.getItem(CENTRAL_URL_KEY) || window.location.origin
}

export function storeCentralUrl(url: string): void {
  const clean = url.trim().replace(/\/+$/, '')
  if (clean === '' || clean === window.location.origin) localStorage.removeItem(CENTRAL_URL_KEY)
  else localStorage.setItem(CENTRAL_URL_KEY, clean)
}

/** É um endereço que o GitHub Actions consegue chamar (pelo formato)? */
export function centralUrlRule(value: string): true | string {
  try {
    const url = new URL(value.trim())
    if (url.protocol !== 'https:' && url.protocol !== 'http:') return 'Use https://…'
    if (url.pathname !== '/' || url.search || url.hash) return 'Só o endereço, sem caminho'
    return true
  } catch {
    return 'Use https://…'
  }
}

/** Um endereço que só a rede local alcança — o GitHub não chega nele. */
export function looksPrivate(value: string): boolean {
  try {
    const host = new URL(value).hostname
    return (
      host === 'localhost' ||
      /^127\./.test(host) ||
      /^10\./.test(host) ||
      /^192\.168\./.test(host) ||
      /^172\.(1[6-9]|2\d|3[01])\./.test(host) ||
      host.endsWith('.local')
    )
  } catch {
    return false
  }
}

/** Envia o manifest ao GitHub (a página sai daqui). */
export function submitManifest(form: DeployGithubManifestView, target: Document = document): void {
  const element = target.createElement('form')
  element.method = 'post'
  element.action = form.actionUrl
  element.style.display = 'none'
  const field = target.createElement('input')
  field.type = 'hidden'
  field.name = 'manifest'
  field.value = form.manifest
  element.appendChild(field)
  target.body.appendChild(element)
  element.submit()
}
