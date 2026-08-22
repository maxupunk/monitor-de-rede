import { apiService } from './apiService'

export interface VapidPublicKeyResponse {
  publicKey: string
}

export interface PushStatusResponse {
  configured: boolean
  publicKey: string
  totalSubscriptions: number
  userSubscriptions: number
}

export interface SaveSubscriptionPayload {
  endpoint: string
  keys: {
    p256dh: string
    auth: string
  }
  userAgent?: string
}

export interface TestPushResponse {
  success: boolean
  sent: number
  expiredPruned: number
  message: string
}

export class PushService {
  async getVapidPublicKey(): Promise<string> {
    const res = await apiService.get<VapidPublicKeyResponse>('/push/vapid-public-key')
    return res.publicKey
  }

  async getStatus(): Promise<PushStatusResponse> {
    return apiService.get<PushStatusResponse>('/push/status')
  }

  async saveSubscription(
    payload: SaveSubscriptionPayload
  ): Promise<{ success: boolean; id: number }> {
    return apiService.post<{ success: boolean; id: number }>('/push/subscriptions', payload)
  }

  async deleteSubscription(endpoint: string): Promise<{ success: boolean }> {
    return apiService.delete<{ success: boolean }>('/push/subscriptions', { endpoint })
  }

  async sendTestPush(): Promise<TestPushResponse> {
    return apiService.post<TestPushResponse>('/push/test')
  }
}

export const pushService = new PushService()
