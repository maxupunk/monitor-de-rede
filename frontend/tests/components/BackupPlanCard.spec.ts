import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { Pinia } from 'pinia'
import BackupPlanCard from '@/components/backup/BackupPlanCard.vue'
import type { BackupPlanView } from '@/utils/backupSchedule'
import { button, mountWith, setupDom, teardownDom } from './domHarness'

const semDestino: BackupPlanView = {
  storageDestinationId: null,
  storageDestinationName: null,
  backupEnabled: false,
  backupIntervalHours: 24,
  backupRetention: 7,
  lastBackupAt: null,
  lastBackupStatus: null,
  lastBackupError: null,
  nextBackupAt: null,
}

describe('BackupPlanCard.vue', () => {
  let pinia: Pinia
  beforeEach(() => {
    pinia = setupDom()
  })
  afterEach(teardownDom)

  const card = (extra: Record<string, unknown> = {}) =>
    mountWith(
      BackupPlanCard,
      {
        title: 'NetMonitor',
        description: 'Tudo o que você configurou aqui.',
        icon: 'mdi-monitor-dashboard',
        color: 'primary',
        plan: semDestino,
        ...extra,
      },
      pinia
    )

  it('o NetMonitor restaura de um arquivo mesmo sem destino', async () => {
    const wrapper = card({ restoreWithoutDestination: true })
    expect(button('Escolher onde guardar')).toBeDefined()
    button('Restaurar…')?.click()
    expect(wrapper.emitted('restore')).toHaveLength(1)
  })

  it('um banco sem destino não oferece restaurar', () => {
    card()
    expect(button('Escolher onde guardar')).toBeDefined()
    expect(button('Restaurar…')).toBeUndefined()
  })
})
