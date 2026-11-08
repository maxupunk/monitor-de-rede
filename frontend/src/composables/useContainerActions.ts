import { computed } from 'vue'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import { confirm } from '@/composables/useConfirm'
import { useAuthStore } from '@/stores/auth'
import { useDockerStore } from '@/stores/docker'
import {
  CONTAINER_ACTIONS,
  containerActions,
  containerName,
  type ContainerActionKey,
  type ContainerActionMeta,
} from '@/utils/dockerPresentation'

export interface ActionOutcome {
  ok: boolean
  message: string
}

type Target = Pick<DockerContainerSummary, 'id' | 'names' | 'image' | 'state' | 'protected'>

/**
 * As ações de ciclo de vida de um container, no host selecionado: o que a
 * política libera (ADR 011), a confirmação das destrutivas e a chamada. A
 * linha da tabela, o cartão do celular e o diálogo de detalhe usam a mesma.
 */
export function useContainerActions() {
  const docker = useDockerStore()
  const auth = useAuthStore()

  const access = computed(() => ({
    lifecycle: auth.isAdmin && docker.allows('lifecycle'),
    update: auth.isAdmin && docker.allows('update'),
  }))

  function available(container: Target): ContainerActionMeta[] {
    return containerActions(container, access.value)
  }

  async function confirmed(container: Target, key: ContainerActionKey): Promise<boolean> {
    const name = containerName(container)
    const meta = CONTAINER_ACTIONS[key]
    const messages: Partial<Record<ContainerActionKey, string>> = {
      stop: `Parar o container "${name}"? Ele continua existindo e pode ser iniciado de novo.`,
      remove: `Remover o container "${name}"? Os volumes (dados) ficam.`,
      update: `Baixar a versão atual de "${container.image}" e recriar "${name}" com a mesma configuração? Se algo falhar, o container original é restaurado.`,
    }
    const message = messages[key]
    if (!message) return true
    return confirm({
      title: `${meta.label} — ${name}`,
      message,
      confirmText: meta.label,
      confirmColor: meta.color,
      icon: meta.icon,
    })
  }

  async function run(container: Target, key: ContainerActionKey): Promise<ActionOutcome | null> {
    if (!(await confirmed(container, key))) return null
    let message = ''
    const ok = await docker.runAction(async () => {
      if (key === 'update') {
        await docker.api.updateContainer(container.id)
        message = 'Atualização iniciada. O progresso aparece no topo da tela.'
        return
      }
      const response = await {
        start: () => docker.api.startContainer(container.id),
        stop: () => docker.api.stopContainer(container.id),
        restart: () => docker.api.restartContainer(container.id),
        remove: () => docker.api.removeContainer(container.id),
      }[key]()
      message = response.message
    })
    return ok ? { ok, message } : { ok, message: docker.error || 'Operação não concluída' }
  }

  return { access, available, run }
}
