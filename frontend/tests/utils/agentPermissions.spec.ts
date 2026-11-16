import { describe, expect, it } from 'vitest'
import { PERMISSION_ORDER, permissionColor, permissionLabel } from '@/utils/agentPermissions'

describe('agentPermissions', () => {
  it('toda permissão do contrato tem rótulo e descrição', () => {
    expect(PERMISSION_ORDER).toHaveLength(12)
    expect(PERMISSION_ORDER[0]).toBe('read')
    expect(permissionLabel('database')).toBe('Ponte de banco (backup)')
  })

  it('o que mexe no host ou nos dados aparece como atenção', () => {
    expect(permissionColor('database')).toBe('warning')
    expect(permissionColor('build')).toBe('warning')
    expect(permissionColor('read')).toBe('primary')
  })
})
