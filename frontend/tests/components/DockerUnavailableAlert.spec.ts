import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import DockerUnavailableAlert from '@/components/docker/DockerUnavailableAlert.vue'

describe('DockerUnavailableAlert.vue', () => {
  it('mostra onde configurar a integração e como recriar o container', () => {
    const wrapper = mount(DockerUnavailableAlert, {
      props: {
        reason: 'Integração Docker desativada: DOCKER_ENABLED=false no ambiente do backend',
      },
      global: {
        stubs: {
          'v-alert': { template: '<section><slot /></section>' },
          'v-expansion-panels': { template: '<div><slot /></div>' },
          'v-expansion-panel': { template: '<div><slot /></div>' },
          'v-expansion-panel-title': { template: '<div><slot /></div>' },
          'v-expansion-panel-text': { template: '<div><slot /></div>' },
        },
      },
    })

    const text = wrapper.text()
    expect(text).toContain('DOCKER_ENABLED=false')
    expect(text).toContain('.env')
    expect(text).toContain('/var/run/docker.sock:/var/run/docker.sock')
    expect(text).toContain('docker compose up -d --force-recreate netmonitor')
    expect(text).toContain('Docker Desktop ou Docker Engine')
  })
})
