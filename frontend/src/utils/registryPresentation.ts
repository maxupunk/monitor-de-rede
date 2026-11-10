/**
 * Como o registry de imagens aparece na tela (ADR 022): o endereço que a
 * equipe usa pela VPN, os comandos prontos para copiar e o resumo da
 * varredura. Puro — testável sem montar componente.
 */
import type { DeployOriginKind } from '@/bindings/DeployOriginKind'
import type { RegistryScanSummary } from '@/bindings/RegistryScanSummary'
import { countNoun } from '@/utils/formatters'
import { looksPrivate } from '@/utils/githubConnect'

/** O endereço do registry pela rede interna (o host por onde a tela abriu). */
export function registryHost(location: { hostname: string; port: string }): string {
  return location.port ? `${location.hostname}:${location.port}` : location.hostname
}

/** Como a tela chegou à central: o que muda nas instruções do `docker`. */
export interface RegistryAccess {
  host: string
  /** HTTPS: o Docker confia sem `insecure-registries`. */
  secure: boolean
  /** Rede interna ou VPN: só daí um token pessoal vale (ADR 022). */
  internal: boolean
}

export function registryAccess(location: {
  hostname: string
  port: string
  protocol: string
  origin: string
}): RegistryAccess {
  return {
    host: registryHost(location),
    secure: location.protocol === 'https:',
    internal: looksPrivate(location.origin),
  }
}

/** O trecho do `daemon.json`: pela VPN o tráfego já vai cifrado (WireGuard). */
export function daemonJson(host: string): string {
  return JSON.stringify({ 'insecure-registries': [host] }, null, 2)
}

/**
 * O `docker login`. O usuário não importa (quem autentica é o token):
 * `token` deixa isso claro.
 */
export function dockerLogin(host: string): string {
  return `docker login ${host} -u token`
}

/** `docker login`, `tag`, `push` e `pull` com os nomes reais. */
export function dockerCommands(host: string, repository: string, tag = 'latest') {
  return {
    login: dockerLogin(host),
    push: `docker tag minha-imagem ${host}/${repository}:${tag}\ndocker push ${host}/${repository}:${tag}`,
    pull: `docker pull ${host}/${repository}:${tag}`,
  }
}

/** O que a análise diz em uma frase. */
export function scanSentence(scan: RegistryScanSummary | null | undefined): string {
  if (!scan) return 'Ainda não analisada'
  switch (scan.status) {
    case 'pending':
      return 'Análise em andamento'
    case 'no_sbom':
      return 'Sem lista de pacotes para analisar'
    case 'failed':
      return 'A base de vulnerabilidades não respondeu'
    default:
      break
  }
  const serious = scan.critical + scan.high
  if (serious === 0 && scan.medium + scan.low === 0) return 'Nenhuma vulnerabilidade conhecida'
  if (serious === 0) {
    const minor = scan.medium + scan.low
    return `${countNoun(minor, 'vulnerabilidade')} ${minor === 1 ? 'média ou baixa' : 'médias ou baixas'}`
  }
  // Só o que existe: "1 alta", não "0 críticas e 1 alta".
  return [
    scan.critical ? countNoun(scan.critical, 'crítica') : '',
    scan.high ? countNoun(scan.high, 'alta') : '',
  ]
    .filter(Boolean)
    .join(' e ')
}

/** Cor de cada severidade de vulnerabilidade. */
export const SEVERITY_COLOR: Record<string, string> = {
  critical: 'error',
  high: 'deep-orange',
  medium: 'warning',
  low: 'info',
  unknown: 'secondary',
}

export const SEVERITY_LABEL: Record<string, string> = {
  critical: 'Crítica',
  high: 'Alta',
  medium: 'Média',
  low: 'Baixa',
  unknown: 'Sem nota',
}

/** Cor do resumo: a da severidade mais grave encontrada; verde limpa. */
export function scanColor(scan: RegistryScanSummary | null | undefined): string {
  if (!scan || scan.status !== 'done') return 'secondary'
  if (scan.critical > 0) return SEVERITY_COLOR.critical!
  if (scan.high > 0) return SEVERITY_COLOR.high!
  if (scan.medium > 0) return SEVERITY_COLOR.medium!
  return 'success'
}

/** De onde a imagem veio, para a tela. */
export const ORIGIN_LABEL: Record<DeployOriginKind, string> = {
  build: 'compilada pela central',
  workflow: 'enviada pelo GitHub Actions',
  manual_push: 'push manual',
}

/**
 * As plataformas de uma versão, curtas: até duas por extenso, o resto como
 * contagem (a lista inteira fica no `title`). Uma imagem oficial traz oito.
 */
export function platformsLabel(platforms: string[]): string {
  if (platforms.length <= 2) return platforms.join(', ')
  return `${platforms.slice(0, 2).join(', ')} e mais ${platforms.length - 2}`
}

/** Tag de commit (40 hexadecimais): imutável. */
export function isCommitTag(tag: string): boolean {
  return /^[0-9a-f]{40}$/i.test(tag)
}

/** A tag como a tela mostra (o commit abreviado). */
export function shortTag(tag: string): string {
  return isCommitTag(tag) ? tag.slice(0, 7) : tag
}

/** `sha256:` + 12 — o mesmo corte do backend (`short_digest`). */
export function shortDigest(digest: string): string {
  return digest.slice(0, 19)
}

/** Repositório e ação de um escopo de token, para os chips. */
export const ACTION_LABEL: Record<string, string> = {
  pull: 'baixar',
  push: 'enviar',
  delete: 'apagar',
}
