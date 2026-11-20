import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import BackupCountsSummary from '@/components/backup/BackupCountsSummary.vue'
import type { BackupCountsResponse } from '@/bindings/BackupCountsResponse'

const stubs = { 'v-chip': { template: '<span><slot /></span>' } }

function summary(counts: BackupCountsResponse) {
  return mount(BackupCountsSummary, {
    props: { name: 'backup.json.gz', counts },
    global: { stubs },
  })
}

describe('BackupCountsSummary.vue', () => {
  it('mostra só as tabelas com registros e resume as vazias', () => {
    const wrapper = summary({
      scope: 'full',
      totalRows: 3,
      tables: [
        { table: 'users', label: 'Usuários', rows: 1 },
        { table: 'probes', label: 'Agentes e sondas', rows: 0 },
        { table: 'sites', label: 'Sites', rows: 2 },
        { table: 'networks', label: 'Redes IP', rows: 0 },
      ],
    })
    const chips = wrapper.findAll('[data-table]').map((chip) => chip.text())
    expect(chips).toEqual(['Usuários: 1', 'Sites: 2'])
    expect(wrapper.find('[data-empty]').text()).toBe('e 2 tabelas vazias')
  })

  it('arquivo sem nenhum registro diz isso sem chip nenhum', () => {
    const wrapper = summary({
      scope: 'config',
      totalRows: 0,
      tables: [{ table: 'sites', label: 'Sites', rows: 0 }],
    })
    expect(wrapper.findAll('[data-table]')).toHaveLength(0)
    expect(wrapper.find('[data-empty]').text()).toBe('1 tabela vazia')
  })
})
