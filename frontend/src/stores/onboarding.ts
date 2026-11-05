import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { OnboardingStatus } from '@/bindings/OnboardingStatus'
import { useAuthStore } from './auth'

export type { OnboardingStatus }

export const useOnboardingStore = defineStore('onboarding', () => {
  const status = ref<OnboardingStatus | null>(null)
  const loading = ref(false)
  const saving = ref(false)
  const showWizard = ref(false)
  const dismissedInSession = ref(false)
  const checked = ref(false)

  async function fetchStatus(): Promise<OnboardingStatus | null> {
    loading.value = true
    try {
      const data = await apiService.get<OnboardingStatus>('/settings/onboarding')
      status.value = data
      return data
    } catch {
      return null
    } finally {
      loading.value = false
    }
  }

  /**
   * Avalia se deve abrir o assistente automaticamente no primeiro acesso.
   * Só abre se o operador tiver permissão de escrita e a instalação estiver
   * pendente: áreas ainda não escolhidas ou, com a área Rede ligada, sem site e
   * rede (o servidor decide em `needsOnboarding`).
   */
  async function checkAndOpenIfNeeded(): Promise<boolean> {
    if (checked.value || dismissedInSession.value) return false
    const auth = useAuthStore()
    if (!auth.isAuthenticated || !auth.canWrite) return false

    checked.value = true
    const st = await fetchStatus()
    if (st && st.needsOnboarding && !st.completed) {
      showWizard.value = true
      return true
    }
    return false
  }

  function openWizard() {
    showWizard.value = true
    void fetchStatus()
  }

  function dismissWizard(persistOnServer = false) {
    showWizard.value = false
    dismissedInSession.value = true
    if (persistOnServer) {
      void completeOnboarding()
    }
  }

  /** A escolha das áreas foi gravada (`POST /settings/onboarding/intent`). */
  function markIntentChosen() {
    if (status.value) status.value.intentChosen = true
  }

  async function completeOnboarding(): Promise<boolean> {
    saving.value = true
    try {
      await apiService.post('/settings/onboarding/complete')
      if (status.value) {
        status.value.completed = true
        status.value.needsOnboarding = false
      }
      return true
    } catch {
      return false
    } finally {
      saving.value = false
    }
  }

  return {
    status,
    loading,
    saving,
    showWizard,
    dismissedInSession,
    checked,
    fetchStatus,
    checkAndOpenIfNeeded,
    openWizard,
    dismissWizard,
    markIntentChosen,
    completeOnboarding,
  }
})
