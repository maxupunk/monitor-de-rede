/**
 * Centralized API Service (SOLID - Single Responsibility Principle)
 * Handles HTTP requests, authentication header injection, response normalization,
 * request timeouts and distinct error types for network vs API failures.
 */

export interface ApiErrorResponse {
  message: string
  errors?: Array<{ field?: string; message: string }>
}

/**
 * Endpoints em que um 401 é resposta de negócio, não sessão expirada.
 *
 * Sem esta lista, errar a senha no login ou o token de instalação no cadastro
 * inicial dispararia o redirecionamento de "sua sessão acabou" — e a mensagem
 * do backend sumiria da tela junto com o formulário preenchido.
 */
const CREDENTIAL_PATHS = ['/auth/login', '/auth/setup']

/** Timeout padrão para requisições à API (ms). */
const DEFAULT_TIMEOUT_MS = 15000

/** Erro de resposta da API (status >= 400 com corpo parseável). */
export class ApiError extends Error {
  readonly status: number
  readonly response?: ApiErrorResponse

  constructor(message: string, status: number, response?: ApiErrorResponse) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.response = response
  }
}

/** Erro de rede ou timeout — a requisição não chegou a produzir uma resposta. */
export class NetworkError extends Error {
  constructor(message = 'Falha de conexão com o servidor.') {
    super(message)
    this.name = 'NetworkError'
  }
}

function isNetworkError(err: unknown): boolean {
  return err instanceof TypeError || (err instanceof Error && err.name === 'AbortError')
}

class ApiService {
  private baseUrl = '/api'

  private getHeaders(): HeadersInit {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    }
    // SEC-07: o token continua no localStorage. A mitigação principal contra
    // XSS é o CSP restritivo (`script-src 'self'`) aplicado pelo servidor
    // estático em `backend/src/spa.rs`, que impede execução de scripts
    // injetados. A migração para cookie HttpOnly foi avaliada, mas exigiria
    // reescrita do fluxo de autenticação e proteção CSRF para mutações.
    const token = localStorage.getItem('auth_token')
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }
    return headers
  }

  private buildUrl(path: string): string {
    return `${this.baseUrl}${path}`
  }

  private redirectToLogin() {
    const current = encodeURIComponent(window.location.pathname + window.location.search)
    if (window.location.pathname !== '/login') {
      window.location.assign(`/login?redirect=${current}`)
    }
  }

  private async doFetch(path: string, init: RequestInit): Promise<Response> {
    const controller = new AbortController()
    const timeoutId = window.setTimeout(() => controller.abort(), DEFAULT_TIMEOUT_MS)

    try {
      return await fetch(this.buildUrl(path), {
        ...init,
        signal: controller.signal,
      })
    } catch (err: unknown) {
      if (isNetworkError(err)) {
        throw new NetworkError()
      }
      throw err
    } finally {
      window.clearTimeout(timeoutId)
    }
  }

  private async handleResponse<T>(response: Response, path = ''): Promise<T> {
    if (!response.ok) {
      let errorMessage = `Erro HTTP ${response.status}: ${response.statusText}`
      let errorData: ApiErrorResponse | undefined
      try {
        errorData = (await response.json()) as ApiErrorResponse
        if (errorData.message) {
          errorMessage = errorData.message
        } else if (Array.isArray(errorData.errors) && errorData.errors.length > 0) {
          errorMessage = errorData.errors.map((e) => e.message).join(', ')
        }
      } catch {
        // Fallback for non-JSON error responses
      }
      if (response.status === 401 && !CREDENTIAL_PATHS.includes(path)) {
        localStorage.removeItem('auth_token')
        localStorage.removeItem('auth_user')
        this.redirectToLogin()
      }
      throw new ApiError(errorMessage, response.status, errorData)
    }

    if (response.status === 204) {
      return {} as T
    }

    return response.json() as Promise<T>
  }

  async get<T>(path: string): Promise<T> {
    const response = await this.doFetch(path, {
      method: 'GET',
      headers: this.getHeaders(),
    })
    return this.handleResponse<T>(response, path)
  }

  async post<T>(path: string, body?: unknown): Promise<T> {
    const response = await this.doFetch(path, {
      method: 'POST',
      headers: this.getHeaders(),
      body: body ? JSON.stringify(body) : undefined,
    })
    return this.handleResponse<T>(response, path)
  }

  async put<T>(path: string, body?: unknown): Promise<T> {
    const response = await this.doFetch(path, {
      method: 'PUT',
      headers: this.getHeaders(),
      body: body ? JSON.stringify(body) : undefined,
    })
    return this.handleResponse<T>(response, path)
  }

  async patch<T>(path: string, body?: unknown): Promise<T> {
    const response = await this.doFetch(path, {
      method: 'PATCH',
      headers: this.getHeaders(),
      body: body ? JSON.stringify(body) : undefined,
    })
    return this.handleResponse<T>(response, path)
  }

  async delete<T>(path: string, body?: unknown): Promise<T> {
    const response = await this.doFetch(path, {
      method: 'DELETE',
      headers: this.getHeaders(),
      body: body ? JSON.stringify(body) : undefined,
    })
    return this.handleResponse<T>(response, path)
  }

  /**
   * POST que retorna a Response crua (sem consumir/parsear o corpo), para endpoints que
   * transmitem a resposta em streaming (ex: NDJSON) em vez de um único JSON final.
   * Aceita um AbortSignal externo para permitir cancelamento pelo chamador; se nenhum
   * for passado, o timeout padrão ainda é aplicado.
   */
  async postStream(path: string, body: unknown, signal?: AbortSignal): Promise<Response> {
    const controller = new AbortController()
    const timeoutId = window.setTimeout(() => controller.abort(), DEFAULT_TIMEOUT_MS)

    if (signal) {
      signal.addEventListener('abort', () => controller.abort(), { once: true })
    }

    try {
      const response = await fetch(this.buildUrl(path), {
        method: 'POST',
        headers: this.getHeaders(),
        body: JSON.stringify(body),
        signal: controller.signal,
      })
      if (!response.ok) {
        await this.handleResponse(response, path)
      }
      return response
    } catch (err: unknown) {
      if (isNetworkError(err)) {
        throw new NetworkError()
      }
      throw err
    } finally {
      window.clearTimeout(timeoutId)
    }
  }
}

export const apiService = new ApiService()
