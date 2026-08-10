export interface Hop {
  step: number
  ipAddress: string
  hostname?: string
  latencyMs: number
}

export class RouteResolver {
  async resolveRoute(_targetIp: string): Promise<Hop[]> {
    return []
  }
}
