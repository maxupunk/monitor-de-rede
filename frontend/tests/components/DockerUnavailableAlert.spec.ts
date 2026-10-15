import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import DockerUnavailableAlert from '@/components/docker/DockerUnavailableAlert.vue'

const stubs = {
  'v-alert': { template: '<section><slot /></section>' },
  'v-expansion-panels': { template: '<div><slot /></div>' },
  'v-expansion-panel': { template: '<div><slot /></div>' },
  'v-expansion-panel-title': { template: '<div><slot /></div>' },
  'v-expansion-panel-text': { template: '<div><slot /></div>' },
}

describe('DockerUnavailableAlert.vue', () => {
  it('manda corrigir o agente local, que é quem monta o socket', () => {
    const wrapper = mount(DockerUnavailableAlert, {
      props: {
        reason: 'Integração Docker desativada: DOCKER_ENABLED=false no ambiente do agente',
        local: true,
      },
      global: { stubs },
    })

    const text = wrapper.text()
    expect(text).toContain('DOCKER_ENABLED=false')
    expect(text).toContain('.env')
    expect(text).toContain('/var/run/docker.sock:/var/run/docker.sock')
    expect(text).toContain('docker compose up -d --force-recreate netmonitor-agent')
    expect(text).toContain('ao lado desta central')
    expect(text).toContain('LOCAL_AGENT_ALLOW')
  })

  it('aponta para o servidor remoto quando o host é de outro agente', () => {
    const wrapper = mount(DockerUnavailableAlert, {
      props: { reason: 'Docker Engine inacessível', local: false },
      global: { stubs },
    })

    const text = wrapper.text()
    expect(text).toContain('instalado no servidor remoto')
    expect(text).not.toContain('LOCAL_AGENT_ALLOW')
  })
})
