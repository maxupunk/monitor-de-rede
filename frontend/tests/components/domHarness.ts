/**
 * Montagem de componentes com o Vuetify de verdade num DOM de teste: o que os
 * diálogos (teleportados para o `body`) precisam do navegador, e atalhos para
 * achar botão e campo pelo texto que o usuário vê.
 */
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia, type Pinia } from 'pinia'
import { vi } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'
import { useAuthStore } from '@/stores/auth'

class FakeResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

/** Pinia nova e os globais do navegador que o Vuetify usa. */
export function setupDom(): Pinia {
  const pinia = createPinia()
  setActivePinia(pinia)
  vi.stubGlobal('ResizeObserver', FakeResizeObserver)
  vi.stubGlobal('visualViewport', {
    width: 1280,
    height: 800,
    offsetLeft: 0,
    offsetTop: 0,
    addEventListener() {},
    removeEventListener() {},
  })
  vi.stubGlobal('localStorage', {
    getItem: vi.fn(() => null),
    setItem: vi.fn(),
    removeItem: vi.fn(),
    clear: vi.fn(),
  })
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  )
  return pinia
}

export function teardownDom() {
  document.body.innerHTML = ''
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
}

export function setRole(role: 'admin' | 'operator' | 'viewer') {
  const auth = useAuthStore()
  auth.user = { id: 1, name: 'Ana', email: 'a@b.c', role } as never
}

export function mountWith<T>(component: T, props: Record<string, unknown>, pinia: Pinia) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/:path(.*)*', component: { template: '<div />' } }],
  })
  return mount(component as never, {
    props,
    attachTo: document.body,
    global: { plugins: [pinia, createVuetify({ components, directives }), router] },
  })
}

/** O botão visível com este texto, em qualquer lugar da página (diálogos inclusive). */
export function button(label: string): HTMLButtonElement | undefined {
  return [...document.body.querySelectorAll('button')].find((b) => b.textContent?.includes(label))
}

export function bodyText(): string {
  return document.body.textContent ?? ''
}

/** Digita num campo, como o usuário. */
export function type(input: HTMLInputElement, value: string) {
  input.value = value
  input.dispatchEvent(new Event('input'))
}
