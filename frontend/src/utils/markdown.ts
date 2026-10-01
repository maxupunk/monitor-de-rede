/**
 * Conversor leve de Markdown para HTML seguro (DOMPurify): blocos e trechos de
 * código, negrito, itálico, listas e quebras de linha. Um só lugar para o chat
 * da IA e a documentação de uso dos plugins.
 */
import DOMPurify from 'dompurify'

export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;')
}

export interface MarkdownOptions {
  /** `#`, `##`, `###` viram título em negrito (documentação de uso). */
  headings?: boolean
}

export function renderMarkdown(
  content: string | null | undefined,
  options: MarkdownOptions = {}
): string {
  if (!content) return ''

  let text = content

  // Bloco de código: ```linguagem\n...\n```
  text = text.replace(/```([a-zA-Z0-9_-]*)\n([\s\S]*?)```/g, (_match, _lang, code) => {
    return `<pre class="code-block pa-2 rounded my-2 font-mono text-body-small overflow-x-auto"><code>${escapeHtml(code.trim())}</code></pre>`
  })

  // Código inline: `...`
  text = text.replace(/`([^`]+)`/g, (_match, code) => {
    return `<code class="inline-code px-1 rounded font-mono text-body-small">${escapeHtml(code)}</code>`
  })

  if (options.headings) {
    text = text.replace(/^#{1,3}\s+(.+)$/gm, '<strong class="md-heading">$1</strong>')
  }

  // Negrito: **...**
  text = text.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')

  // Itálico: *...*
  text = text.replace(/\*([^*]+)\*/g, '<em>$1</em>')

  // Listas não ordenadas: - item ou * item
  text = text.replace(/^[*-]\s+(.+)$/gm, '<li class="ml-4">$1</li>')

  // Listas ordenadas: 1. item
  text = text.replace(/^\d+\.\s+(.+)$/gm, '<li class="ml-4 list-decimal">$1</li>')

  // Quebras de linha normais
  text = text.replace(/\n\n/g, '<br/><br/>').replace(/\n/g, '<br/>')

  return DOMPurify.sanitize(text, {
    ALLOWED_TAGS: ['strong', 'em', 'code', 'pre', 'li', 'ul', 'ol', 'br', 'p', 'span'],
    ALLOWED_ATTR: ['class'],
  })
}
