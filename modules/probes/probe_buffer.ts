export class ProbeBuffer {
  async saveResultOffline(_taskId: string, _result: unknown): Promise<void> {
    // Buffer local SQLite para funcionamento offline do probe
  }

  async getPendingResults(): Promise<Array<{ taskId: string; result: unknown }>> {
    return []
  }
}
