import { ref } from 'vue'
import type { DeployReferencesView } from '@/bindings/DeployReferencesView'
import { useDeploysStore, type GithubSource } from '@/stores/deploys'

/**
 * As branches e tags de um repositório, para o alvo escolher o que
 * acompanha. É ajuda, não condição: sem acesso ao GitHub a lista fica vazia
 * e o campo continua aceitando texto.
 */
export function useRepositoryReferences() {
  const deploys = useDeploysStore()
  const references = ref<DeployReferencesView | null>(null)
  const loading = ref(false)

  async function load(repository: string, source: GithubSource): Promise<void> {
    references.value = null
    if (!repository) return
    loading.value = true
    try {
      references.value = await deploys.githubReferences(source, repository)
    } catch {
      references.value = null
    } finally {
      loading.value = false
    }
  }

  return { references, loading, load }
}
