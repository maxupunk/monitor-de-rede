export interface ProbeTask {
  id: string
  type: string
  timeoutMs: number
  payload: Record<string, unknown>
}

export class ProbeTaskDispatcher {
  async dispatchTask(_probeId: string, _task: ProbeTask): Promise<void> {
    // Despachar tarefa para probe
  }
}
