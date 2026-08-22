import { ref, onMounted } from 'vue'

interface BeforeInstallPromptEvent extends Event {
  prompt(): Promise<void>
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed'; platform: string }>
}

const deferredPrompt = ref<BeforeInstallPromptEvent | null>(null)
const canInstall = ref<boolean>(false)
const isInstalled = ref<boolean>(false)
const isIos = ref<boolean>(false)
const showIosDialog = ref<boolean>(false)

let isInitialized = false

export function usePwaInstall() {
  function checkInstalledState() {
    if (typeof window === 'undefined') return

    // Verifica se o app já está rodando em modo PWA standalone
    const isStandalone =
      window.matchMedia('(display-mode: standalone)').matches ||
      ('standalone' in window.navigator &&
        Boolean((window.navigator as { standalone?: boolean }).standalone))

    isInstalled.value = isStandalone

    // Detecta se é dispositivo iOS / Safari
    const ua = window.navigator.userAgent.toLowerCase()
    const isIosDevice = /iphone|ipad|ipod/.test(ua)
    const isSafari = /safari/.test(ua) && !/chrome|crios|fxios|edgios/.test(ua)
    isIos.value = isIosDevice && isSafari && !isStandalone

    // No iOS, se não estiver instalado em modo standalone, o usuário pode instalar via menu de compartilhamento
    if (isIos.value && !isStandalone) {
      canInstall.value = true
    }
  }

  function initPwaListeners() {
    if (typeof window === 'undefined' || isInitialized) return
    isInitialized = true

    checkInstalledState()

    // Captura o evento nativo de instalação do navegador (Chrome, Edge, Android, Opera)
    window.addEventListener('beforeinstallprompt', (e: Event) => {
      e.preventDefault()
      deferredPrompt.value = e as BeforeInstallPromptEvent
      canInstall.value = true
    })

    // Ouve quando o app foi instalado com sucesso
    window.addEventListener('appinstalled', () => {
      isInstalled.value = true
      canInstall.value = false
      deferredPrompt.value = null
      console.log('NetMonitor PWA instalado com sucesso!')
    })
  }

  async function promptInstall(): Promise<boolean> {
    if (isIos.value) {
      showIosDialog.value = true
      return true
    }

    if (!deferredPrompt.value) {
      return false
    }

    try {
      await deferredPrompt.value.prompt()
      const choice = await deferredPrompt.value.userChoice
      if (choice.outcome === 'accepted') {
        canInstall.value = false
        deferredPrompt.value = null
        return true
      }
      return false
    } catch (err) {
      console.error('Erro ao acionar prompt de instalação PWA:', err)
      return false
    }
  }

  onMounted(() => {
    initPwaListeners()
  })

  return {
    canInstall,
    isInstalled,
    isIos,
    showIosDialog,
    promptInstall,
    initPwaListeners,
  }
}
