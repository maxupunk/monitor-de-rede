import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import { drainNdjson } from '@/services/ndjson'
import { getStoredToken } from '@/utils/authStorage'

export interface TracerouteHop {
  hop: number
  ip?: string | null
  hostname?: string | null
  rttMs: (number | null)[]
  avgRttMs?: number | null
  status: 'reached' | 'intermediate' | 'timeout' | 'unreachable' | string
}

export interface SpeedTestProgress {
  phase: 'ping' | 'download' | 'upload' | 'complete' | 'done' | string
  progressPct: number
  currentMbps?: number | null
  pingMs?: number | null
  jitterMs?: number | null
  downloadMbps?: number | null
  uploadMbps?: number | null
  serverName?: string | null
  serverLocation?: string | null
}

export interface SpeedTestResult {
  pingMs: number
  jitterMs: number
  downloadMbps: number
  uploadMbps: number
  serverName?: string | null
  serverLocation?: string | null
  timestamp: string
}

export interface PlaybookStepResult {
  stepIndex: number
  stepName: string
  description: string
  status: 'running' | 'success' | 'warning' | 'failed' | string
  message?: string | null
  data?: Record<string, unknown> | null
}

export interface PlaybookSummary {
  playbookType: string
  target?: string | null
  status: 'success' | 'warning' | 'failed' | string
  diagnosis: string
  recommendations: string[]
  steps: PlaybookStepResult[]
}

export const useDiagnosticsStore = defineStore('diagnostics', () => {
  // Traceroute State
  const tracerouteRunning = ref(false)
  const tracerouteError = ref<string | null>(null)
  let activeTracerouteController: AbortController | null = null

  // Speed Test State
  const speedTestRunning = ref(false)
  const speedTestError = ref<string | null>(null)
  let activeSpeedTestController: AbortController | null = null

  // Playbook State
  const playbookRunning = ref(false)
  const playbookError = ref<string | null>(null)
  let activePlaybookController: AbortController | null = null

  /**
   * Executa o traceroute ICMP nativo transmitindo cada salto resolvido em tempo real.
   */
  async function runTraceroute(
    payload: {
      host: string
      maxHops?: number
      timeoutMs?: number
      probesPerHop?: number
    },
    onHop: (hop: TracerouteHop) => void
  ): Promise<boolean> {
    tracerouteRunning.value = true
    tracerouteError.value = null
    const controller = new AbortController()
    activeTracerouteController = controller

    try {
      const response = await apiService.postStream(
        '/diagnostics/traceroute',
        payload,
        controller.signal
      )
      const reader = response.body?.getReader()
      if (!reader) return false

      const decoder = new TextDecoder()
      let buffer = ''
      let completed = false

      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        buffer += decoder.decode(value, { stream: true })

        const parsedChunk = drainNdjson<{ type?: string; message?: string } & TracerouteHop>(buffer)
        buffer = parsedChunk.remainder
        for (const parsed of parsedChunk.events) {
          if (parsed.type === 'hop') {
            delete parsed.type
            onHop(parsed as TracerouteHop)
          } else if (parsed.type === 'error') {
            tracerouteError.value = parsed.message || 'Erro durante o traceroute'
          } else if (parsed.type === 'done') {
            completed = true
          }
        }
      }

      const finalChunk = drainNdjson<{ type?: string; message?: string } & TracerouteHop>(buffer, {
        final: true,
      })
      for (const parsed of finalChunk.events) {
        if (parsed.type === 'hop') {
          delete parsed.type
          onHop(parsed as TracerouteHop)
        } else if (parsed.type === 'error') {
          tracerouteError.value = parsed.message || 'Erro durante o traceroute'
        } else if (parsed.type === 'done') {
          completed = true
        }
      }

      return completed && !tracerouteError.value
    } catch (err: unknown) {
      if (err instanceof DOMException && err.name === 'AbortError') {
        return false
      }
      tracerouteError.value = err instanceof Error ? err.message : 'Erro ao executar traceroute'
      return false
    } finally {
      tracerouteRunning.value = false
      activeTracerouteController = null
    }
  }

  function cancelTraceroute() {
    activeTracerouteController?.abort()
  }

  /**
   * Executa o teste de velocidade WAN (Cloudflare CDN) transmitindo o progresso em tempo real.
   */
  async function runSpeedTest(
    onProgress: (progress: SpeedTestProgress) => void,
    onComplete: (result: SpeedTestResult) => void
  ): Promise<boolean> {
    speedTestRunning.value = true
    speedTestError.value = null
    const controller = new AbortController()
    activeSpeedTestController = controller

    try {
      const response = await apiService.postStream('/diagnostics/speedtest', {}, controller.signal)
      const reader = response.body?.getReader()
      if (!reader) return false

      const decoder = new TextDecoder()
      let buffer = ''
      let completed = false

      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        buffer += decoder.decode(value, { stream: true })

        const parsedChunk = drainNdjson<
          {
            type?: string
            message?: string
            result?: SpeedTestResult
          } & SpeedTestProgress
        >(buffer)
        buffer = parsedChunk.remainder
        for (const parsed of parsedChunk.events) {
          if (parsed.type === 'progress') {
            delete parsed.type
            onProgress(parsed as SpeedTestProgress)
          } else if (parsed.type === 'complete' && parsed.result) {
            onComplete(parsed.result)
            completed = true
          } else if (parsed.type === 'error') {
            speedTestError.value = parsed.message || 'Erro durante o teste de velocidade'
          }
        }
      }

      const finalChunk = drainNdjson<
        {
          type?: string
          message?: string
          result?: SpeedTestResult
        } & SpeedTestProgress
      >(buffer, { final: true })
      for (const parsed of finalChunk.events) {
        if (parsed.type === 'progress') {
          delete parsed.type
          onProgress(parsed as SpeedTestProgress)
        } else if (parsed.type === 'complete' && parsed.result) {
          onComplete(parsed.result)
          completed = true
        } else if (parsed.type === 'error') {
          speedTestError.value = parsed.message || 'Erro durante o teste de velocidade'
        }
      }

      return completed && !speedTestError.value
    } catch (err: unknown) {
      if (err instanceof DOMException && err.name === 'AbortError') {
        return false
      }
      speedTestError.value =
        err instanceof Error ? err.message : 'Erro ao executar teste de velocidade'
      return false
    } finally {
      speedTestRunning.value = false
      activeSpeedTestController = null
    }
  }

  function cancelSpeedTest() {
    activeSpeedTestController?.abort()
  }

  /**
   * Executa teste de velocidade LAN (Navegador <-> NetMonitor Backend).
   */
  async function runLanTest(
    onProgress: (progress: SpeedTestProgress) => void
  ): Promise<SpeedTestResult | null> {
    speedTestRunning.value = true
    speedTestError.value = null
    const controller = new AbortController()
    activeSpeedTestController = controller

    try {
      const token = getStoredToken()
      const authHeaders: Record<string, string> = token ? { Authorization: `Bearer ${token}` } : {}

      // 1. Ping LAN
      onProgress({
        phase: 'ping',
        progressPct: 10.0,
        serverName: 'Servidor Local (LAN)',
        serverLocation: 'Rede Local',
      })

      const pingSamples: number[] = []
      for (let i = 0; i < 5; i++) {
        if (controller.signal.aborted) return null
        const t0 = performance.now()
        const pingResp = await fetch('/api/diagnostics/speedtest/lan/download?bytes=0', {
          headers: authHeaders,
          signal: controller.signal,
        })
        if (!pingResp.ok) {
          throw new Error(`Falha no teste de latência LAN (${pingResp.status})`)
        }
        const rtt = performance.now() - t0
        pingSamples.push(rtt)
        onProgress({
          phase: 'ping',
          progressPct: 10.0 + (i + 1) * 3.0,
          pingMs:
            Math.round((pingSamples.reduce((a, b) => a + b, 0) / pingSamples.length) * 100) / 100,
          serverName: 'Servidor Local (LAN)',
          serverLocation: 'Rede Local',
        })
      }

      const avgPing =
        Math.round((pingSamples.reduce((a, b) => a + b, 0) / pingSamples.length) * 100) / 100
      let jitter = 0
      if (pingSamples.length > 1) {
        let diffSum = 0
        for (let i = 1; i < pingSamples.length; i++) {
          diffSum += Math.abs(pingSamples[i] - pingSamples[i - 1])
        }
        jitter = Math.round((diffSum / (pingSamples.length - 1)) * 100) / 100
      }

      // 2. Download LAN (25 MB)
      onProgress({
        phase: 'download',
        progressPct: 30.0,
        pingMs: avgPing,
        jitterMs: jitter,
        serverName: 'Servidor Local (LAN)',
        serverLocation: 'Rede Local',
      })

      const downloadBytes = 25_000_000
      const downStart = performance.now()
      const downResp = await fetch(
        `/api/diagnostics/speedtest/lan/download?bytes=${downloadBytes}`,
        {
          headers: authHeaders,
          signal: controller.signal,
        }
      )

      if (!downResp.ok || !downResp.body) {
        throw new Error(`Falha no stream de download LAN (${downResp.status})`)
      }
      const reader = downResp.body.getReader()
      let received = 0

      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        received += value?.length || 0
        const elapsed = (performance.now() - downStart) / 1000
        const currentMbps = Math.round(((received * 8) / (elapsed * 1_000_000)) * 100) / 100
        const pct = 30.0 + (received / downloadBytes) * 35.0

        onProgress({
          phase: 'download',
          progressPct: Math.min(65.0, Math.round(pct)),
          currentMbps,
          downloadMbps: currentMbps,
          pingMs: avgPing,
          jitterMs: jitter,
          serverName: 'Servidor Local (LAN)',
          serverLocation: 'Rede Local',
        })
      }

      const downTotalSec = (performance.now() - downStart) / 1000
      const finalDownloadMbps =
        Math.round(((received * 8) / (downTotalSec * 1_000_000)) * 100) / 100

      // 3. Upload LAN (10 MB)
      onProgress({
        phase: 'upload',
        progressPct: 70.0,
        downloadMbps: finalDownloadMbps,
        pingMs: avgPing,
        jitterMs: jitter,
        serverName: 'Servidor Local (LAN)',
        serverLocation: 'Rede Local',
      })

      const uploadBytes = 10_000_000
      const dummyData = new Uint8Array(uploadBytes)
      const upStart = performance.now()

      const upResp = await fetch('/api/diagnostics/speedtest/lan/upload', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/octet-stream',
          ...authHeaders,
        },
        body: dummyData,
        signal: controller.signal,
      })

      if (!upResp.ok) throw new Error(`Falha no upload LAN (${upResp.status})`)
      const upTotalSec = (performance.now() - upStart) / 1000
      const finalUploadMbps = Math.round(((uploadBytes * 8) / (upTotalSec * 1_000_000)) * 100) / 100

      const result: SpeedTestResult = {
        pingMs: avgPing,
        jitterMs: jitter,
        downloadMbps: finalDownloadMbps,
        uploadMbps: finalUploadMbps,
        serverName: 'Servidor Local (LAN)',
        serverLocation: 'Rede Local (Cliente ↔ Servidor)',
        timestamp: new Date().toISOString(),
      }

      onProgress({
        phase: 'complete',
        progressPct: 100.0,
        pingMs: result.pingMs,
        jitterMs: result.jitterMs,
        downloadMbps: result.downloadMbps,
        uploadMbps: result.uploadMbps,
        serverName: result.serverName,
        serverLocation: result.serverLocation,
      })

      return result
    } catch (err: unknown) {
      if (err instanceof DOMException && err.name === 'AbortError') {
        return null
      }
      speedTestError.value =
        err instanceof Error ? err.message : 'Erro ao executar teste de velocidade LAN'
      return null
    } finally {
      speedTestRunning.value = false
      activeSpeedTestController = null
    }
  }

  /**
   * Executa um Playbook de Diagnóstico automatizado transmitindo cada etapa resolvida.
   */
  async function runPlaybook(
    payload: {
      playbookType: 'internet_health' | 'device_reachability' | string
      target?: string
      deviceId?: number
    },
    onStep: (step: PlaybookStepResult) => void,
    onSummary: (summary: PlaybookSummary) => void
  ): Promise<boolean> {
    playbookRunning.value = true
    playbookError.value = null
    const controller = new AbortController()
    activePlaybookController = controller

    try {
      const response = await apiService.postStream(
        '/diagnostics/playbook/run',
        payload,
        controller.signal
      )
      const reader = response.body?.getReader()
      if (!reader) return false

      const decoder = new TextDecoder()
      let buffer = ''
      let completed = false

      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        buffer += decoder.decode(value, { stream: true })

        const parsedChunk = drainNdjson<{
          type?: string
          message?: string
          summary?: PlaybookSummary
          step?: PlaybookStepResult
        }>(buffer)
        buffer = parsedChunk.remainder
        for (const parsed of parsedChunk.events) {
          if (parsed.type === 'step' && parsed.step) {
            onStep(parsed.step)
          } else if (parsed.type === 'summary' && parsed.summary) {
            onSummary(parsed.summary)
            completed = true
          } else if (parsed.type === 'done') {
            completed = true
          } else if (parsed.type === 'error') {
            playbookError.value = parsed.message || 'Erro durante execução do playbook'
          }
        }
      }

      const finalChunk = drainNdjson<{
        type?: string
        message?: string
        summary?: PlaybookSummary
        step?: PlaybookStepResult
      }>(buffer, { final: true })
      for (const parsed of finalChunk.events) {
        if (parsed.type === 'step' && parsed.step) {
          onStep(parsed.step)
        } else if (parsed.type === 'summary' && parsed.summary) {
          onSummary(parsed.summary)
          completed = true
        } else if (parsed.type === 'done') {
          completed = true
        } else if (parsed.type === 'error') {
          playbookError.value = parsed.message || 'Erro durante execução do playbook'
        }
      }

      return completed && !playbookError.value
    } catch (err: unknown) {
      if (err instanceof DOMException && err.name === 'AbortError') {
        return false
      }
      playbookError.value =
        err instanceof Error ? err.message : 'Erro ao executar playbook de diagnóstico'
      return false
    } finally {
      playbookRunning.value = false
      activePlaybookController = null
    }
  }

  function cancelPlaybook() {
    activePlaybookController?.abort()
  }

  return {
    tracerouteRunning,
    tracerouteError,
    runTraceroute,
    cancelTraceroute,
    speedTestRunning,
    speedTestError,
    runSpeedTest,
    runLanTest,
    cancelSpeedTest,
    playbookRunning,
    playbookError,
    runPlaybook,
    cancelPlaybook,
  }
})
