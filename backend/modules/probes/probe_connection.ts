export class ProbeConnection {
  async connect(_serverUrl: string, _token: string): Promise<boolean> {
    return true
  }
}
