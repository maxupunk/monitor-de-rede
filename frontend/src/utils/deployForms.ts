/**
 * Os formulários do deploy contínuo: valores iniciais, leitura do que está
 * salvo e o corpo que a API recebe. O assistente e os diálogos de edição
 * usam as mesmas peças — uma regra só para "o que é um alvo novo".
 */
import type { DeployApplicationInput } from '@/bindings/DeployApplicationInput'
import type { DeployComposeDiscovery } from '@/bindings/DeployComposeDiscovery'
import type { DeployReferencesView } from '@/bindings/DeployReferencesView'
import type { DeployApplicationView } from '@/bindings/DeployApplicationView'
import type { DeployTargetView } from '@/bindings/DeployTargetView'
import type { DeployTargetInput } from '@/bindings/DeployTargetInput'

/** O que o assistente já sabe ao abrir (o botão da aba Compose). */
export interface SetupPreset {
  probeId?: number
  project?: string
}

/** O alvo no formulário: o agente pode ainda não ter sido escolhido. */
export type TargetForm = Omit<DeployTargetInput, 'probeId'> & { probeId: number | null }

export function emptyApplication(): DeployApplicationInput {
  return {
    name: '',
    slug: '',
    repository: '',
    credentialId: null,
    githubConnectionId: null,
    composeFiles: ['docker-compose.yml'],
    imageTagVariable: 'IMAGE_TAG',
    images: [],
    rollout: 'sequential',
  }
}

export function applicationFromView(view: DeployApplicationView): DeployApplicationInput {
  return {
    name: view.name,
    slug: view.slug,
    repository: view.repository,
    credentialId: view.credentialId,
    githubConnectionId: view.githubConnectionId,
    composeFiles: [...view.composeFiles],
    imageTagVariable: view.imageTagVariable,
    images: view.images.map((image) => ({ ...image })),
    rollout: view.rollout,
  }
}

/** Um alvo novo: produção com aprovação, rollback e 3 versões guardadas. */
export function emptyTarget(partial: Partial<TargetForm> = {}): TargetForm {
  return {
    name: 'Produção',
    probeId: null,
    project: '',
    reference: 'main',
    requiresApproval: true,
    windowStart: undefined,
    windowEnd: undefined,
    backupConnectionId: null,
    waitTimeoutSecs: 180,
    autoRollback: true,
    keepImages: 3,
    enabled: true,
    position: 0,
    ...partial,
  }
}

export function targetFromView(view: DeployTargetView): TargetForm {
  return {
    name: view.name,
    probeId: view.agent?.id ?? null,
    project: view.project,
    reference: view.reference,
    requiresApproval: view.requiresApproval,
    windowStart: view.windowStart ?? undefined,
    windowEnd: view.windowEnd ?? undefined,
    backupConnectionId: view.backupConnectionId,
    waitTimeoutSecs: view.waitTimeoutSecs,
    autoRollback: view.autoRollback,
    keepImages: view.keepImages,
    enabled: view.enabled,
    position: view.position,
  }
}

/** O corpo da API. `null` quando falta o agente (o formulário não deixa chegar aqui). */
export function toTargetInput(form: TargetForm): DeployTargetInput | null {
  if (form.probeId === null) return null
  return {
    ...form,
    probeId: form.probeId,
    windowStart: form.windowStart || undefined,
    windowEnd: form.windowEnd || undefined,
  }
}

/** Aplicação sem as imagens deixadas em branco. */
export function cleanApplication(input: DeployApplicationInput): DeployApplicationInput {
  return {
    ...input,
    images: input.images.filter((image) => image.name.trim() !== ''),
  }
}

/** O identificador de projeto a partir de um nome: `SGCv4` → `sgcv4`. */
export function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, '-')
    .replace(/^[-_]+|[-_]+$/g, '')
    .slice(0, 63)
}

/**
 * O que o compose do repositório preenche: arquivos, imagens e a variável
 * da tag. Só troca o que o compose de fato disse.
 */
export function applyDiscovery(
  input: DeployApplicationInput,
  discovery: DeployComposeDiscovery
): DeployApplicationInput {
  return {
    ...input,
    composeFiles: discovery.suggestedFiles.length
      ? [...discovery.suggestedFiles]
      : input.composeFiles,
    images: discovery.images.length
      ? discovery.images.map((image) => ({ ...image }))
      : input.images,
    imageTagVariable: discovery.imageTagVariable ?? input.imageTagVariable,
  }
}

/**
 * O que um alvo pode acompanhar: as branches do repositório e padrões de
 * tag (o `v*` quando há tags no formato `v1.2.3`).
 */
export function referenceChoices(references: DeployReferencesView | null): string[] {
  if (!references) return []
  const patterns = references.tags.some((tag) => /^v\d/.test(tag)) ? ['v*'] : []
  return [...references.branches, ...patterns]
}
