import { describe, expect, it } from 'vitest'
import {
  DEFAULT_DATABASE_TARGETS,
  PERMISSION_ORDER,
  cloneInstallPolicy,
  defaultInstallPolicy,
  permissionColor,
  permissionLabel,
} from '@/utils/agentPermissions'

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

  it('a política inicial sugere todas as permissões e clona sem compartilhar referência', () => {
    const initial = defaultInstallPolicy()
    expect(initial.allow).toEqual(PERMISSION_ORDER)
    expect(initial.databaseTargets).toBe(DEFAULT_DATABASE_TARGETS)

    const copy = cloneInstallPolicy(initial)
    copy.allow.pop()
    expect(initial.allow).toHaveLength(PERMISSION_ORDER.length)
  })
})
