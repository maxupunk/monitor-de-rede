import { describe, expect, it } from 'vitest'
import { renderMarkdown } from '@/utils/markdown'

describe('renderMarkdown', () => {
  it('renderiza títulos, listas agrupadas e parágrafos', () => {
    const html = renderMarkdown('## Diagnóstico\n\nTexto **forte**\n\n- um\n- dois\n\n1. a\n2. b')
    expect(html).toContain('<h4 class="md-heading">Diagnóstico</h4>')
    expect(html).toContain('<p>Texto <strong>forte</strong></p>')
    expect(html).toContain('<ul class="md-list"><li>um</li><li>dois</li></ul>')
    expect(html).toContain('<ol class="md-list"><li>a</li><li>b</li></ol>')
  })

  it('não reinterpreta o conteúdo de um bloco de código', () => {
    const html = renderMarkdown('```bash\nping *host*\n- não é lista\n```')
    expect(html).toContain('ping *host*\n- não é lista')
    expect(html).not.toContain('<em>')
    expect(html).not.toContain('<li>')
    expect(html).not.toContain('<br')
  })

  it('renderiza tabela com cabeçalho', () => {
    const html = renderMarkdown('| Host | Média |\n|---|:---:|\n| 1.1.1.1 | 12 ms |')
    expect(html).toContain('<th>Host</th><th>Média</th>')
    expect(html).toContain('<td>1.1.1.1</td><td>12 ms</td>')
  })

  it('escapa HTML do texto e só aceita link http(s)', () => {
    const html = renderMarkdown(
      '<script>alert(1)</script> [doc](https://exemplo.local/a) [x](javascript:alert(1))'
    )
    expect(html).not.toContain('<script>')
    expect(html).toContain('&lt;script&gt;')
    expect(html).toContain('<a href="https://exemplo.local/a"')
    expect(html).not.toContain('href="javascript')
  })

  it('código inline protege o que está dentro', () => {
    expect(renderMarkdown('use `**não**` aqui')).toContain('>**não**</code>')
  })

  it('títulos podem ser desligados', () => {
    expect(renderMarkdown('# Título', { headings: false })).toBe('<p># Título</p>')
  })
})
