import { describe, expect, it } from 'vitest'
import { intervalLabel } from '@/utils/backupSchedule'
import { providerInfo, STORAGE_PROVIDERS } from '@/utils/storagePresentation'

describe('storagePresentation', () => {
  it('a família S3 compartilha o mesmo formato de config', () => {
    for (const provider of ['aws_s3', 'minio', 'cloudflare_r2', 's3_compatible'] as const) {
      expect(providerInfo(provider).configType).toBe('s3')
    }
    expect(providerInfo('sftp').configType).toBe('sftp')
  })

  it('cada provider tem rótulo, ícone e cor', () => {
    for (const item of STORAGE_PROVIDERS) {
      expect(item.label).not.toBe('')
      expect(item.icon.startsWith('mdi-')).toBe(true)
      expect(item.color).not.toBe('default')
    }
  })

  it('descreve a frequência num chip', () => {
    expect(intervalLabel(24)).toBe('Diário')
    expect(intervalLabel(168)).toBe('Semanal')
    expect(intervalLabel(6)).toBe('A cada 6 h')
    expect(intervalLabel(72)).toBe('A cada 3 dias')
  })
})
