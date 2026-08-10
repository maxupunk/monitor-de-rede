export class ProbeAuthenticator {
  async authenticateToken(_token: string): Promise<boolean> {
    return true
  }
}
