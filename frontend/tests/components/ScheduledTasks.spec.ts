import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { SchedulableTool } from '@/bindings/SchedulableTool'
import type { ScheduledTaskPreview } from '@/bindings/ScheduledTaskPreview'
import CronScheduleField from '@/components/scheduledTasks/CronScheduleField.vue'
import ScheduledTaskDialog from '@/components/scheduledTasks/ScheduledTaskDialog.vue'
import ScheduledTasksPage from '@/pages/ScheduledTasksPage.vue'
import { apiService } from '@/services/apiService'
import { TASK_TEMPLATES } from '@/utils/scheduledTasks/presentation'
import { bodyText, button, mountWith, setRole, setupDom, teardownDom } from './domHarness'

const CATALOG: SchedulableTool[] = [
  {
    name: 'get_system_summary',
    description: 'Resumo do estado atual da rede.',
    group: 'core',
    groupLabel: 'consultas básicas',
    kind: 'query',
    adminOnly: false,
    parameters: { type: 'object', properties: {} },
  },
  {
    name: 'docker_container_action',
    description: 'Inicia, para ou reinicia um container.',
    group: 'docker',
    groupLabel: 'containers',
    kind: 'container',
    adminOnly: true,
    parameters: {
      type: 'object',
      properties: { action: { type: 'string', enum: ['start', 'stop', 'restart'] } },
      required: ['action'],
    },
  },
]

const PREVIEW: ScheduledTaskPreview = {
  valid: true,
  errors: [],
  scheduleLabel: 'de segunda a sexta às 07:00',
  nextRuns: ['2026-10-26T10:00:00Z'],
  summary: null,
  risk: 'low',
  riskReason: 'Só uma consulta, repetida no horário',
}

function mockGet() {
  return vi.spyOn(apiService, 'get').mockImplementation(async (path: string) => {
    if (path === '/scheduled-tasks/catalog') return CATALOG as never
    if (path === '/devices') return [] as never
    return [] as never
  })
}

async function waitPreview() {
  await new Promise((resolve) => setTimeout(resolve, 400))
  await flushPromises()
}

describe('Tarefas agendadas — telas', () => {
  let pinia: Pinia
  beforeEach(() => {
    pinia = setupDom()
  })
  afterEach(teardownDom)

  it('estado vazio oferece modelos — sem o de container para quem não é administrador', async () => {
    setRole('operator')
    mockGet()
    const wrapper = mountWith(ScheduledTasksPage, {}, pinia)
    await flushPromises()
    expect(bodyText()).toContain('Nenhuma tarefa agendada')
    expect(button('Resumo da rede toda manhã')).toBeDefined()
    expect(button('Reiniciar um container de madrugada')).toBeUndefined()
    expect(button('Nova tarefa')?.disabled).toBe(false)
    wrapper.unmount()
  })

  it('perfil de leitura vê a página mas não agenda', async () => {
    setRole('viewer')
    mockGet()
    const wrapper = mountWith(ScheduledTasksPage, {}, pinia)
    await flushPromises()
    expect(button('Nova tarefa')?.disabled).toBe(true)
    expect(button('Resumo da rede toda manhã')).toBeUndefined()
    wrapper.unmount()
  })

  it('o modelo abre preenchido, a conferência do servidor aparece e "Agendar" salva', async () => {
    setRole('operator')
    mockGet()
    const post = vi.spyOn(apiService, 'post').mockImplementation(async (path: string) => {
      if (path === '/scheduled-tasks/preview') return PREVIEW as never
      return {
        id: 1,
        name: 'Resumo da rede',
        scheduleLabel: PREVIEW.scheduleLabel,
      } as never
    })
    const wrapper = mountWith(
      ScheduledTaskDialog,
      { modelValue: true, task: null, template: TASK_TEMPLATES[0] },
      pinia
    )
    await flushPromises()
    await waitPreview()

    expect(post).toHaveBeenCalledWith('/scheduled-tasks/preview', {
      tool: 'get_system_summary',
      arguments: {},
      cron: '0 7 * * 1-5',
    })
    expect(bodyText()).toContain('de segunda a sexta às 07:00')
    expect(bodyText()).toContain('Risco baixo')
    expect(bodyText()).toContain('Esta ferramenta não recebe parâmetros')

    button('Agendar')?.click()
    await flushPromises()
    expect(post).toHaveBeenLastCalledWith('/scheduled-tasks', {
      name: 'Resumo da rede',
      description: undefined,
      tool: 'get_system_summary',
      arguments: {},
      cron: '0 7 * * 1-5',
      enabled: true,
    })
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false])
    wrapper.unmount()
  })

  it('o campo de agenda troca de modo e devolve a expressão', async () => {
    const wrapper = mountWith(CronScheduleField, { modelValue: '0 3 * * *' }, pinia)
    await flushPromises()
    expect(bodyText()).toContain('0 3 * * *')
    const weekly = [...document.body.querySelectorAll('.v-chip')].find((chip) =>
      chip.textContent?.includes('Dias da semana')
    ) as HTMLElement | undefined
    weekly?.click()
    await flushPromises()
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual(['0 3 * * 1-5'])
    wrapper.unmount()
  })
})
