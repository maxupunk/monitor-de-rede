/**
 * Centralized API Service (SOLID - Single Responsibility Principle)
 * Handles HTTP requests, authentication header injection, and response normalization.
 */

export interface ApiErrorResponse {
  message: string
  errors?: Array<{ field?: string; message: string }>
}

class ApiService {
  private baseUrl = '/api'

  private getHeaders(): HeadersInit {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    }
    const token = localStorage.getItem('auth_token')
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }
    return headers
  }

  private async handleResponse<T>(response: Response): Promise<T> {
    if (!response.ok) {
      let errorMessage = `Erro HTTP ${response.status}: ${response.statusText}`
      try {
        const errorData = (await response.json()) as ApiErrorResponse
        if (errorData.message) {
          errorMessage = errorData.message
        } else if (Array.isArray(errorData.errors) && errorData.errors.length > 0) {
          errorMessage = errorData.errors.map((e) => e.message).join(', ')
        }
      } catch {
        // Fallback for non-JSON error responses
      }
      if (response.status === 401) {
        localStorage.removeItem('auth_token')
      }
      throw new Error(errorMessage)
    }

    if (response.status === 204) {
      return {} as T
    }

    return response.json() as Promise<T>
  }

  async get<T>(path: string): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      method: 'GET',
      headers: this.getHeaders(),
    })
    return this.handleResponse<T>(response)
  }

  async post<T>(path: string, body?: unknown): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      method: 'POST',
      headers: this.getHeaders(),
      body: body ? JSON.stringify(body) : undefined,
    })
    return this.handleResponse<T>(response)
  }

  async put<T>(path: string, body?: unknown): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      method: 'PUT',
      headers: this.getHeaders(),
      body: body ? JSON.stringify(body) : undefined,
    })
    return this.handleResponse<T>(response)
  }

  async patch<T>(path: string, body?: unknown): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      method: 'PATCH',
      headers: this.getHeaders(),
      body: body ? JSON.stringify(body) : undefined,
    })
    return this.handleResponse<T>(response)
  }

  async delete<T>(path: string): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      method: 'DELETE',
      headers: this.getHeaders(),
    })
    return this.handleResponse<T>(response)
  }

  /**
   * POST que retorna a Response crua (sem consumir/parsear o corpo), para endpoints que
   * transmitem a resposta em streaming (ex: NDJSON) em vez de um único JSON final.
   * Aceita um AbortSignal para permitir cancelamento pelo chamador.
   */
  async postStream(path: string, body: unknown, signal?: AbortSignal): Promise<Response> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      method: 'POST',
      headers: this.getHeaders(),
      body: JSON.stringify(body),
      signal,
    })
    if (!response.ok) {
      await this.handleResponse(response)
    }
    return response
  }
}

export const apiService = new ApiService()
