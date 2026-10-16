/**
 * Atalhos de teclado do mapa. A tradução tecla → comando é pura (testável);
 * a página só executa o comando. A mesma tabela alimenta a folha "?".
 */

export type TopologyCommand =
  | 'search'
  | 'fit'
  | 'zoom-in'
  | 'zoom-out'
  | 'zoom-reset'
  | 'escape'
  | 'delete'
  | 'toggle-edit'
  | 'toggle-legend'
  | 'help'
  | 'nudge-left'
  | 'nudge-right'
  | 'nudge-up'
  | 'nudge-down'

export interface KeyLike {
  key: string
  ctrlKey?: boolean
  metaKey?: boolean
  altKey?: boolean
  shiftKey?: boolean
  /** O foco está num campo de texto — só Esc e Ctrl/⌘+K valem. */
  inEditable?: boolean
}

export function resolveShortcut(event: KeyLike): TopologyCommand | null {
  const key = event.key
  const mod = Boolean(event.ctrlKey || event.metaKey)
  if (mod && key.toLowerCase() === 'k') return 'search'
  if (key === 'Escape') return 'escape'
  if (event.inEditable || mod || event.altKey) return null
  switch (key) {
    case 'f':
    case 'F':
      return 'fit'
    case '+':
    case '=':
      return 'zoom-in'
    case '-':
    case '_':
      return 'zoom-out'
    case '0':
      return 'zoom-reset'
    case 'Delete':
    case 'Backspace':
      return 'delete'
    case 'e':
    case 'E':
      return 'toggle-edit'
    case 'l':
    case 'L':
      return 'toggle-legend'
    case '?':
      return 'help'
    case 'ArrowLeft':
      return 'nudge-left'
    case 'ArrowRight':
      return 'nudge-right'
    case 'ArrowUp':
      return 'nudge-up'
    case 'ArrowDown':
      return 'nudge-down'
    default:
      return null
  }
}

/** Passo do ajuste fino com as setas (Shift = passo maior). */
export function nudgeDelta(command: TopologyCommand, big: boolean): [number, number] | null {
  const step = big ? 40 : 8
  switch (command) {
    case 'nudge-left':
      return [-step, 0]
    case 'nudge-right':
      return [step, 0]
    case 'nudge-up':
      return [0, -step]
    case 'nudge-down':
      return [0, step]
    default:
      return null
  }
}

export const SHORTCUT_SHEET: ReadonlyArray<{ keys: string[]; description: string }> = [
  { keys: ['Ctrl/⌘', 'K'], description: 'Buscar dispositivo' },
  { keys: ['F'], description: 'Enquadrar o mapa' },
  { keys: ['+', '−'], description: 'Aproximar / afastar' },
  { keys: ['0'], description: 'Zoom 100%' },
  { keys: ['Esc'], description: 'Limpar seleção / cancelar cabo' },
  { keys: ['Del'], description: 'Excluir o selecionado (modo de edição)' },
  { keys: ['E'], description: 'Ligar / desligar "Editar mapa"' },
  { keys: ['L'], description: 'Mostrar / esconder a legenda' },
  { keys: ['←', '↑', '→', '↓'], description: 'Mover o nó selecionado (Shift = passo maior)' },
  { keys: ['?'], description: 'Esta lista de atalhos' },
]
