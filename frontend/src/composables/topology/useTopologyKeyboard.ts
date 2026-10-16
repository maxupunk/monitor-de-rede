import { onBeforeUnmount, onMounted } from 'vue'
import { resolveShortcut, type TopologyCommand } from '@/utils/topology/shortcuts'

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return (
    target.isContentEditable ||
    target.tagName === 'INPUT' ||
    target.tagName === 'TEXTAREA' ||
    target.tagName === 'SELECT'
  )
}

/**
 * Liga os atalhos do mapa enquanto a página está montada. `enabled` permite
 * desligá-los com um diálogo aberto (o diálogo cuida das próprias teclas).
 */
export function useTopologyKeyboard(
  handler: (command: TopologyCommand, event: KeyboardEvent) => void,
  enabled: () => boolean
) {
  function onKeyDown(event: KeyboardEvent) {
    const command = resolveShortcut({
      key: event.key,
      ctrlKey: event.ctrlKey,
      metaKey: event.metaKey,
      altKey: event.altKey,
      shiftKey: event.shiftKey,
      inEditable: isEditable(event.target),
    })
    if (!command) return
    if (command !== 'search' && !enabled()) return
    event.preventDefault()
    handler(command, event)
  }

  onMounted(() => window.addEventListener('keydown', onKeyDown))
  onBeforeUnmount(() => window.removeEventListener('keydown', onKeyDown))
}
