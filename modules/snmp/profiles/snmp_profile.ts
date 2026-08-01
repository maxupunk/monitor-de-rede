export interface SnmpProfile {
  id: string
  name: string
  version: 'v1' | 'v2c' | 'v3'
  community?: string
}
