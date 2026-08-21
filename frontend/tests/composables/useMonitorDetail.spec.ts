import { describe, it, expect } from 'vitest'
import { useMonitorDetail } from '../../src/composables/useMonitorDetail.ts'

describe('useMonitorDetail', () => {
  it('inicia fechado e sem monitor', () => {
    const { detalheAberto, monitorEmDetalhe } = useMonitorDetail()

    expect(detalheAberto.value).toBe(false)
    expect(monitorEmDetalhe.value).toBeNull()
  })

  it('abre o detalhe a partir de um id numérico', () => {
    const { detalheAberto, monitorEmDetalhe, abrirDetalhe } = useMonitorDetail()

    abrirDetalhe(42)

    expect(detalheAberto.value).toBe(true)
    expect(monitorEmDetalhe.value).toBe(42)
  })

  it('abre o detalhe a partir de um objeto com id', () => {
    const { detalheAberto, monitorEmDetalhe, abrirDetalhe } = useMonitorDetail()

    abrirDetalhe({ id: 7 })

    expect(detalheAberto.value).toBe(true)
    expect(monitorEmDetalhe.value).toBe(7)
  })

  it('mantém estado isolado entre instâncias', () => {
    const a = useMonitorDetail()
    const b = useMonitorDetail()

    a.abrirDetalhe(1)

    expect(a.monitorEmDetalhe.value).toBe(1)
    expect(b.monitorEmDetalhe.value).toBeNull()
  })
})
