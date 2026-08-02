export interface ProbeTask {
  id: string
  monitorId: number
  type: 'ping' | 'http' | 'https' | 'tcp' | 'dns' | 'snmp'
  timeoutMs: number
  payload: Record<string, unknown>
}

export class ProbeTaskDispatcher {
  private static taskQueue: Map<string, ProbeTask[]> = new Map()

  dispatchTask(probeId: number | string, task: ProbeTask): void {
    const key = String(probeId)
    const existing = ProbeTaskDispatcher.taskQueue.get(key) || []
    existing.push(task)
    ProbeTaskDispatcher.taskQueue.set(key, existing)
  }

  getPendingTasks(probeId: number | string): ProbeTask[] {
    const key = String(probeId)
    const tasks = ProbeTaskDispatcher.taskQueue.get(key) || []
    ProbeTaskDispatcher.taskQueue.set(key, [])
    return tasks
  }

  clearTasksForProbe(probeId: number | string): void {
    const key = String(probeId)
    ProbeTaskDispatcher.taskQueue.delete(key)
  }
}
