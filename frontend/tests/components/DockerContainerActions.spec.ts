import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import DockerContainerActions from '@/components/docker/DockerContainerActions.vue'
import { useDockerStore } from '@/stores/docker'
import { bodyText, button, mountWith, setRole, setupDom, teardownDom } from './domHarness'

const stopped: DockerContainerSummary = {
  id: 'abc',
  names: ['/erp_db'],
  image: 'postgres:16',
  imageId: '',
  state: 'exited',
  status: 'Exited (0)',
  labels: {},
  ports: [],
  created: 0,
  projectName: 'erp',
  protected: false,
}

let pinia: Pinia

beforeEach(() => {
  pinia = setupDom()
  setRole('admin')
})

afterEach(() => {
  vi.restoreAllMocks()
  teardownDom()
})

describe('ações do container', () => {
  it('a principal fica à vista e chama a Engine sem confirmação', async () => {
    const docker = useDockerStore()
    vi.spyOn(docker, 'allows').mockReturnValue(true)
    const run = vi.spyOn(docker, 'runAction').mockResolvedValue(true)
    const wrapper = mountWith(DockerContainerActions, { container: stopped, labeled: true }, pinia)

    expect(button('Iniciar')).toBeDefined()
    expect(button('Mais ações')).toBeDefined()
    button('Iniciar')?.click()
    await flushPromises()
    expect(run).toHaveBeenCalledOnce()
    expect(wrapper.emitted('done')).toHaveLength(1)
  })

  it('container protegido mostra o selo e nada que o derrube', () => {
    const docker = useDockerStore()
    vi.spyOn(docker, 'allows').mockReturnValue(true)
    mountWith(
      DockerContainerActions,
      { container: { ...stopped, state: 'running', protected: true }, labeled: true },
      pinia
    )
    expect(bodyText()).toContain('Protegido')
    expect(button('Parar')).toBeUndefined()
    expect(button('Mais ações')).toBeUndefined()
  })
})
