import fs from 'node:fs'
import path from 'node:path'

export interface BufferedResult {
  taskId: string
  result: unknown
  timestamp: string
}

export class ProbeBuffer {
  private filePath: string

  constructor(customPath?: string) {
    this.filePath = customPath || path.join(process.cwd(), 'tmp', 'probe_buffer.json')
  }

  private ensureDirectoryExists() {
    try {
      const dir = path.dirname(this.filePath)
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true })
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      console.error(`[ProbeBuffer] Erro ao criar diretório do buffer: ${msg}`)
    }
  }

  async saveResultOffline(taskId: string, result: unknown): Promise<void> {
    try {
      this.ensureDirectoryExists()
      const current = await this.getPendingResults()
      const newItem: BufferedResult = {
        taskId,
        result,
        timestamp: new Date().toISOString(),
      }
      current.push(newItem)
      fs.writeFileSync(this.filePath, JSON.stringify(current, null, 2), 'utf-8')
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      console.error(`[ProbeBuffer] Erro ao salvar resultado offline: ${msg}`)
    }
  }

  async getPendingResults(): Promise<BufferedResult[]> {
    try {
      if (!fs.existsSync(this.filePath)) {
        return []
      }
      const raw = fs.readFileSync(this.filePath, 'utf-8')
      if (!raw.trim()) {
        return []
      }
      return JSON.parse(raw) as BufferedResult[]
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      console.error(`[ProbeBuffer] Erro ao ler resultados offline: ${msg}`)
      return []
    }
  }

  async clearPendingResults(): Promise<void> {
    try {
      if (fs.existsSync(this.filePath)) {
        fs.unlinkSync(this.filePath)
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      console.error(`[ProbeBuffer] Erro ao limpar resultados offline: ${msg}`)
    }
  }
}
