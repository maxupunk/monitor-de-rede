import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useBackupStore } from '@/stores/backup'

const downloadWithProgress = vi.fn()
const upload = vi.fn()
vi.mock('@/services/apiService', () => ({
  apiService: {
    downloadWithProgress: (...args: unknown[]) => downloadWithProgress(...args),
    upload: (...args: unknown[]) => upload(...args),
  },
}))

const triggerDownload = vi.fn()
vi.mock('@/utils/download', () => ({
  triggerDownload: (...args: unknown[]) => triggerDownload(...args),
}))

const counts = {
  scope: 'full',
  tables: [{ table: 'users', label: 'Usuários', rows: 1 }],
  totalRows: 1,
}

describe('backup store — backup completo', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    downloadWithProgress.mockReset()
    upload.mockReset()
    triggerDownload.mockReset()
  })

  it('baixa em fluxo, mostra o andamento e entrega o arquivo com o nome do servidor', async () => {
    const store = useBackupStore()
    const seen: number[] = []
    downloadWithProgress.mockImplementationOnce(
      async (_path: string, onProgress: (bytes: number) => void) => {
        onProgress(1024)
        seen.push(store.fullDownload?.receivedBytes ?? -1)
        onProgress(4096)
        seen.push(store.fullDownload?.receivedBytes ?? -1)
        return {
          blob: new Blob(['x'.repeat(4096)]),
          filename: 'netmonitor-backup-completo-1.json.gz',
        }
      }
    )

    const size = await store.downloadFull()

    expect(downloadWithProgress.mock.calls[0]?.[0]).toBe('/backup/full')
    expect(seen).toEqual([1024, 4096])
    expect(size).toBe(4096)
    expect(triggerDownload).toHaveBeenCalledWith(
      expect.any(Blob),
      'netmonitor-backup-completo-1.json.gz'
    )
    expect(store.fullDownload).toBeNull()
    expect(store.error).toBeNull()
  })

  it('falha no download vira mensagem clara e nada é entregue', async () => {
    downloadWithProgress.mockRejectedValueOnce(
      new Error('O download foi interrompido antes do fim.')
    )
    const store = useBackupStore()

    expect(await store.downloadFull()).toBeNull()
    expect(store.error).toBe('O download foi interrompido antes do fim.')
    expect(triggerDownload).not.toHaveBeenCalled()
    expect(store.fullDownload).toBeNull()
  })

  it('cancelar não é erro', async () => {
    const store = useBackupStore()
    downloadWithProgress.mockImplementationOnce(
      (_path: string, _onProgress: unknown, signal: AbortSignal) =>
        new Promise((_resolve, reject) => {
          signal.addEventListener('abort', () => reject(new DOMException('abort', 'AbortError')))
        })
    )
    const pending = store.downloadFull()
    store.cancelFullDownload()

    expect(await pending).toBeNull()
    expect(store.error).toBeNull()
  })
})

describe('backup store — restaurar de um arquivo', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    upload.mockReset()
  })

  it('envia o arquivo cru para a prévia e depois para a restauração, com o andamento', async () => {
    const store = useBackupStore()
    const file = new File(['{}'], 'netmonitor-backup-completo.json.gz')
    const phases: string[] = []
    upload
      .mockImplementationOnce(
        async (_path: string, _file: File, onProgress: (s: number, t: number) => void) => {
          onProgress(1, 2)
          phases.push(store.upload?.phase ?? '')
          return counts
        }
      )
      .mockImplementationOnce(
        async (_path: string, _file: File, onProgress: (s: number, t: number) => void) => {
          onProgress(2, 2)
          phases.push(store.upload?.phase ?? '')
          return { counts, signInAgain: true }
        }
      )

    expect(await store.loadFile(file)).toBe(true)
    expect(upload.mock.calls[0]?.slice(0, 2)).toEqual(['/backup/preview', file])
    expect(store.pendingCounts).toEqual(counts)
    expect(store.pendingFile).toBe(file)

    const result = await store.restoreFile()
    expect(upload.mock.calls[1]?.slice(0, 2)).toEqual(['/backup/restore', file])
    expect(result).toEqual({ counts, signInAgain: true })
    expect(phases).toEqual(['preview', 'restore'])
    expect(store.pendingFile).toBeNull()
    expect(store.upload).toBeNull()
    expect(store.restoring).toBe(false)
  })

  it('arquivo recusado na prévia: guarda a mensagem e não deixa restaurar', async () => {
    upload.mockRejectedValueOnce(
      new Error('Arquivo de backup na versão 2 — este sistema lê a versão 1')
    )
    const store = useBackupStore()

    expect(await store.loadFile(new File(['{}'], 'x.json'))).toBe(false)
    expect(store.error).toContain('versão 2')
    expect(await store.restoreFile()).toBeNull()
    expect(upload).toHaveBeenCalledTimes(1)
  })
})
