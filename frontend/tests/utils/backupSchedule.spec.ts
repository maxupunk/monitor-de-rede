import { describe, expect, it } from 'vitest'
import { backupHealth, intervalItems, intervalLabel, retentionRule } from '@/utils/backupSchedule'

describe('agenda dos backups', () => {
  it('frequência gravada fora dos presets continua selecionável', () => {
    expect(intervalItems(24).map((item) => item.value)).toEqual([6, 12, 24, 168])
    expect(intervalItems(48).at(-1)).toEqual({ value: 48, title: 'A cada 2 dias' })
    expect(intervalLabel(3)).toBe('A cada 3 h')
  })

  it('retenção na mesma faixa do backend', () => {
    expect(retentionRule(1)).toBe(true)
    expect(retentionRule(365)).toBe(true)
    expect(retentionRule(0)).not.toBe(true)
    expect(retentionRule('2.5')).not.toBe(true)
  })
})

describe('selo de proteção', () => {
  const base = {
    storageDestinationId: 1,
    storageDestinationName: 'NAS',
    backupEnabled: true,
    backupIntervalHours: 24,
    backupRetention: 7,
    lastBackupAt: '2026-10-07T03:00:00Z',
    lastBackupStatus: 'success',
    lastBackupError: null,
    nextBackupAt: null,
  }

  it('sem destino não está protegido, mesmo com agenda ligada', () => {
    expect(backupHealth({ ...base, storageDestinationId: null }).label).toBe('Não protegido')
  })

  it('falha pesa mais que o resto', () => {
    expect(backupHealth({ ...base, lastBackupStatus: 'failed' }).color).toBe('error')
    expect(backupHealth({ ...base, backupEnabled: false, lastBackupStatus: 'failed' }).color).toBe(
      'error'
    )
  })

  it('manual, aguardando e protegido', () => {
    expect(backupHealth({ ...base, backupEnabled: false }).label).toBe('Só manual')
    expect(backupHealth({ ...base, lastBackupAt: null, lastBackupStatus: null }).color).toBe('info')
    expect(backupHealth(base)).toMatchObject({ label: 'Protegido', color: 'success', advice: '' })
  })
})
