export interface ProbeTask {
  id: string
  type: string
  timeoutMs: number
  payload: Record<string, unknown>
}

export class ProbeTaskDispatcher {
  async dispatchTask(probeId: string, task: ProbeTask): Promise<void> {
    // Despachar tarefa para probe
  }
}
