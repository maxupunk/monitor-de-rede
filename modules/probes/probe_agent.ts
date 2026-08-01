export interface ProbePayload {
  probeId: string
  version: string
  status: 'online' | 'offline' | 'busy'
  runningTasks: number
  timestamp: string
}

export class ProbeAgent {
  async start() {
    // Inicializar o agente Probe
  }
}
