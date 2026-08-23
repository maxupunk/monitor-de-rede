import { reactive } from 'vue'

export type ConfirmColor = 'primary' | 'secondary' | 'error' | 'warning' | 'info' | 'success'

export interface ConfirmOptions {
  title?: string
  message: string
  confirmText?: string
  cancelText?: string
  confirmColor?: ConfirmColor | string
  icon?: string
  iconColor?: string
  width?: number | string
  persistent?: boolean
}

export interface PromptOptions {
  title?: string
  message?: string
  placeholder?: string
  defaultValue?: string
  inputLabel?: string
  inputType?: string
  confirmText?: string
  cancelText?: string
  confirmColor?: ConfirmColor | string
  icon?: string
  iconColor?: string
  width?: number | string
  persistent?: boolean
  rules?: Array<(val: string) => boolean | string>
}

interface DialogState {
  isOpen: boolean
  isPrompt: boolean
  promptValue: string
  options: ConfirmOptions & {
    placeholder?: string
    defaultValue?: string
    inputLabel?: string
    inputType?: string
    rules?: Array<(val: string) => boolean | string>
  }
}

const defaultOptions: ConfirmOptions = {
  title: 'Confirmação',
  message: '',
  confirmText: 'Confirmar',
  cancelText: 'Cancelar',
  confirmColor: 'primary',
  icon: 'mdi-help-circle-outline',
  width: 480,
  persistent: false,
}

const state = reactive<DialogState>({
  isOpen: false,
  isPrompt: false,
  promptValue: '',
  options: { ...defaultOptions },
})

let resolveCallback: ((value: any) => void) | null = null

/**
 * Abre um diálogo assíncrono de confirmação na interface.
 * Retorna uma Promise<boolean> que resolve em true se o usuário confirmou ou false se cancelou.
 */
export function confirm(opts: string | ConfirmOptions): Promise<boolean> {
  const normalized: ConfirmOptions =
    typeof opts === 'string'
      ? {
          ...defaultOptions,
          message: opts,
          icon: 'mdi-alert-circle-outline',
          confirmColor: 'primary',
        }
      : {
          ...defaultOptions,
          ...opts,
          icon:
            opts.icon ||
            (opts.confirmColor === 'error'
              ? 'mdi-delete-alert-outline'
              : opts.confirmColor === 'warning'
                ? 'mdi-alert-outline'
                : 'mdi-help-circle-outline'),
        }

  state.isPrompt = false
  state.promptValue = ''
  state.options = normalized
  state.isOpen = true

  return new Promise<boolean>((resolve) => {
    resolveCallback = resolve
  })
}

/**
 * Abre um diálogo assíncrono de prompt com campo de texto.
 * Retorna uma Promise<string | null> com o valor digitado se confirmado, ou null se cancelado.
 */
export function prompt(opts: PromptOptions): Promise<string | null> {
  const normalized = {
    ...defaultOptions,
    title: opts.title || 'Informação necessária',
    message: opts.message || '',
    confirmText: opts.confirmText || 'Confirmar',
    cancelText: opts.cancelText || 'Cancelar',
    confirmColor: opts.confirmColor || 'primary',
    icon: opts.icon || 'mdi-form-textbox',
    iconColor: opts.iconColor,
    width: opts.width || 480,
    persistent: opts.persistent || false,
    placeholder: opts.placeholder,
    defaultValue: opts.defaultValue,
    inputLabel: opts.inputLabel,
    inputType: opts.inputType || 'text',
    rules: opts.rules,
  }

  state.isPrompt = true
  state.promptValue = opts.defaultValue || ''
  state.options = normalized
  state.isOpen = true

  return new Promise<string | null>((resolve) => {
    resolveCallback = resolve
  })
}

function handleConfirm(customValue?: string) {
  state.isOpen = false
  if (resolveCallback) {
    if (state.isPrompt) {
      resolveCallback(customValue !== undefined ? customValue : state.promptValue)
    } else {
      resolveCallback(true)
    }
    resolveCallback = null
  }
}

function handleCancel() {
  state.isOpen = false
  if (resolveCallback) {
    if (state.isPrompt) {
      resolveCallback(null)
    } else {
      resolveCallback(false)
    }
    resolveCallback = null
  }
}

export function useConfirm() {
  return {
    state,
    confirm,
    prompt,
    handleConfirm,
    handleCancel,
  }
}
