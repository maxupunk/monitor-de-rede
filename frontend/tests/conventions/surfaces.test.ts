// @vitest-environment node
import { readdirSync, readFileSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

/**
 * O padrão de superfícies (`plugins/vuetify.ts`): card de página com
 * `elevation="2"`, painel aninhado com `border flat`, card de escolha com
 * `<ChoiceCard>` e canto `lg`. Estas regras barram as duas fugas que já
 * aconteceram — o contorno quase branco e o canto exagerado.
 */

const SRC = resolve(__dirname, '../../src')

function vueFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) return vueFiles(path)
    return entry.name.endsWith('.vue') ? [path] : []
  })
}

interface Tag {
  where: string
  name: string
  attrs: string
}

/** A tag inteira — `>` dentro de aspas (`v-if="n > 0"`) não a fecha. */
const SURFACE = /<(v-card|v-sheet|v-expansion-panel)(?=[\s>/])((?:[^>"]|"[^"]*")*)>/g

const CHIP = /<(v-chip)(?=[\s>/])((?:[^>"]|"[^"]*")*)>/g

function surfaces(pattern: RegExp = SURFACE): Tag[] {
  return vueFiles(SRC).flatMap((file) => {
    const source = readFileSync(file, 'utf8')
    return [...source.matchAll(pattern)].map((match) => {
      const line = source.slice(0, match.index).split('\n').length
      return {
        where: `${relative(SRC, file)}:${line}`,
        name: match[1] ?? '',
        attrs: match[2] ?? '',
      }
    })
  })
}

describe('superfícies: o padrão visual dos cards', () => {
  const tags = surfaces()

  it('encontra as superfícies do app', () => {
    expect(tags.length).toBeGreaterThan(100)
  })

  it('canto é no máximo lg — xl destoa do resto das telas', () => {
    const offenders = tags
      .filter((tag) => /rounded="(xl|xxl)"|\brounded-(xl|xxl)\b/.test(tag.attrs))
      .map((tag) => tag.where)
    expect(offenders).toEqual([])
  })

  it('card contornado tem cor: sem ela a borda sai na cor do texto (quase branca)', () => {
    const offenders = tags
      .filter((tag) => tag.name === 'v-card')
      .filter((tag) => /(^|\s)variant="outlined"|:variant="[^"]*'outlined'/.test(tag.attrs))
      .filter((tag) => !/(^|\s):?color=/.test(tag.attrs))
      .map((tag) => tag.where)
    expect(offenders).toEqual([])
  })

  it('chip contornado tem cor, e o de filtro também quando não está escolhido', () => {
    const offenders = surfaces(CHIP)
      .filter((tag) => /(^|\s)variant="outlined"/.test(tag.attrs))
      .filter((tag) => {
        const isFilter = /(^|\s)filter(\s|$)/.test(tag.attrs)
        // Num grupo, o chip não escolhido usa só o `base-color`.
        return isFilter
          ? !/(^|\s):?base-color=/.test(tag.attrs)
          : !/(^|\s):?(base-)?color=/.test(tag.attrs)
      })
      .map((tag) => tag.where)
    expect(offenders).toEqual([])
  })
})
