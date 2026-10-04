/**
 * Conversor leve de Markdown para HTML seguro (DOMPurify). Um só lugar para o
 * chat da IA e a documentação de uso dos plugins.
 *
 * Lê por blocos — código cercado, títulos, tabelas, listas, citações e
 * parágrafos — e só depois formata o texto de cada bloco. Assim o conteúdo
 * de um bloco de código nunca é reinterpretado (um `*` dentro dele não vira
 * itálico, e as quebras de linha não dobram).
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
  /**
   * `#`…`######` viram título. Ligado por padrão: modelo de IA escreve
   * títulos o tempo todo, e o `##` cru na tela parecia defeito.
   */
  headings?: boolean
}

const FENCE = /^```\s*([\w-]*)\s*$/
const HEADING = /^(#{1,6})\s+(.+?)\s*#*\s*$/
const UNORDERED = /^\s*[-*+]\s+(.*)$/
const ORDERED = /^\s*\d+[.)]\s+(.*)$/
const QUOTE = /^>\s?(.*)$/
const RULE = /^\s*([-*_])(\s*\1){2,}\s*$/
const TABLE_SEPARATOR = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/

/** Negrito, itálico, código e link num trecho de texto já sem blocos. */
function inline(text: string): string {
  const codes: string[] = []
  let html = escapeHtml(text).replace(/`([^`]+)`/g, (_match, code: string) => {
    codes.push(`<code class="inline-code px-1 rounded font-mono text-body-small">${code}</code>`)
    return `\u0000${codes.length - 1}\u0000`
  })
  html = html
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/__([^_]+)__/g, '<strong>$1</strong>')
    .replace(/(^|[^*\w])\*([^*\s][^*]*)\*(?!\w)/g, '$1<em>$2</em>')
    .replace(
      /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
      '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>'
    )
  return html.replace(/\u0000(\d+)\u0000/g, (_match, index: string) => codes[Number(index)])
}

function cells(row: string): string[] {
  return row
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split('|')
    .map((cell) => cell.trim())
}

function table(header: string, rows: string[]): string {
  const head = cells(header)
    .map((cell) => `<th>${inline(cell)}</th>`)
    .join('')
  const body = rows
    .map(
      (row) =>
        `<tr>${cells(row)
          .map((cell) => `<td>${inline(cell)}</td>`)
          .join('')}</tr>`
    )
    .join('')
  return `<table class="md-table"><thead><tr>${head}</tr></thead><tbody>${body}</tbody></table>`
}

export function renderMarkdown(
  content: string | null | undefined,
  options: MarkdownOptions = {}
): string {
  if (!content) return ''
  const headings = options.headings ?? true
  const lines = content.replace(/\r\n?/g, '\n').split('\n')
  const blocks: string[] = []
  let paragraph: string[] = []

  const flushParagraph = () => {
    if (paragraph.length > 0) {
      blocks.push(`<p>${paragraph.map(inline).join('<br/>')}</p>`)
      paragraph = []
    }
  }

  let index = 0
  while (index < lines.length) {
    const line = lines[index]

    const fence = FENCE.exec(line)
    if (fence) {
      flushParagraph()
      const code: string[] = []
      index += 1
      while (index < lines.length && !FENCE.test(lines[index])) {
        code.push(lines[index])
        index += 1
      }
      index += 1
      blocks.push(
        `<pre class="code-block pa-2 rounded my-2 font-mono text-body-small overflow-x-auto"><code>${escapeHtml(code.join('\n').replace(/^\n+|\n+$/g, ''))}</code></pre>`
      )
      continue
    }

    const heading = headings ? HEADING.exec(line) : null
    if (heading) {
      flushParagraph()
      const level = Math.min(6, heading[1].length + 2)
      blocks.push(`<h${level} class="md-heading">${inline(heading[2])}</h${level}>`)
      index += 1
      continue
    }

    if (line.includes('|') && TABLE_SEPARATOR.test(lines[index + 1] ?? '')) {
      flushParagraph()
      const header = line
      const rows: string[] = []
      index += 2
      while (index < lines.length && lines[index].includes('|') && lines[index].trim()) {
        rows.push(lines[index])
        index += 1
      }
      blocks.push(table(header, rows))
      continue
    }

    const listKind = UNORDERED.test(line) ? 'ul' : ORDERED.test(line) ? 'ol' : null
    if (listKind) {
      flushParagraph()
      const pattern = listKind === 'ul' ? UNORDERED : ORDERED
      const items: string[] = []
      while (index < lines.length && pattern.test(lines[index])) {
        items.push(`<li>${inline(pattern.exec(lines[index])?.[1] ?? '')}</li>`)
        index += 1
      }
      blocks.push(`<${listKind} class="md-list">${items.join('')}</${listKind}>`)
      continue
    }

    if (QUOTE.test(line)) {
      flushParagraph()
      const quoted: string[] = []
      while (index < lines.length && QUOTE.test(lines[index])) {
        quoted.push(QUOTE.exec(lines[index])?.[1] ?? '')
        index += 1
      }
      blocks.push(`<blockquote class="md-quote">${quoted.map(inline).join('<br/>')}</blockquote>`)
      continue
    }

    if (RULE.test(line)) {
      flushParagraph()
      blocks.push('<hr class="md-rule"/>')
      index += 1
      continue
    }

    if (line.trim() === '') {
      flushParagraph()
    } else {
      paragraph.push(line)
    }
    index += 1
  }
  flushParagraph()

  return DOMPurify.sanitize(blocks.join(''), {
    ALLOWED_TAGS: [
      'a',
      'blockquote',
      'br',
      'code',
      'em',
      'h3',
      'h4',
      'h5',
      'h6',
      'hr',
      'li',
      'ol',
      'p',
      'pre',
      'span',
      'strong',
      'table',
      'tbody',
      'td',
      'th',
      'thead',
      'tr',
      'ul',
    ],
    ALLOWED_ATTR: ['class', 'href', 'target', 'rel'],
  })
}
