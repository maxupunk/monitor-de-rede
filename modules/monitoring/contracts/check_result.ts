export type MonitorStatus = 'up' | 'down' | 'warning' | 'unknown'

export interface CheckMetric {
  name: string
  value: number
  unit: string
}

export interface CheckResult {
  success: boolean
  status: MonitorStatus
  startedAt: Date
  finishedAt: Date
  durationMs: number
  message?: string
  metrics?: CheckMetric[]
  data?: Record<string, unknown>
}

export interface MonitorChecker<TConfig, TResult extends CheckResult = CheckResult> {
  execute(config: TConfig): Promise<TResult>
}
