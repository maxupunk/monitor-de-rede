export class ProbeBuffer {
  async saveResultOffline(taskId: string, result: unknown): Promise<void> {
    // Buffer local SQLite para funcionamento offline do probe
  }

  async getPendingResults(): Promise<Array<{ taskId: string; result: unknown }>> {
    return []
  }
}
