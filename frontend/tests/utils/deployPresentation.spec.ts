import { describe, expect, it } from 'vitest'
import {
  RUN_STATUS,
  commitTitle,
  isFinished,
  isPending,
  stageStates,
  windowLabel,
} from '@/utils/deployPresentation'
import { isValidEnvKey, looksSecret, parseEnv, serializeEnv } from '@/utils/envFile'
import { emptyTarget, toTargetInput } from '@/utils/deployForms'

describe('apresentação do deploy', () => {
  it('nenhum estado usa cor apagada', () => {
    for (const meta of Object.values(RUN_STATUS)) {
      expect(['default', 'grey']).not.toContain(meta.color)
    }
  })

  it('pendente e terminado não se sobrepõem', () => {
    expect(isPending('awaiting_approval')).toBe(true)
    expect(isPending('running')).toBe(false)
    expect(isFinished('running')).toBe(false)
    expect(isFinished('rolled_back')).toBe(true)
  })

  it('etapas acompanham a etapa em curso e a falha', () => {
    const running = stageStates('pull', 'running')
    expect(running.sync).toBe('done')
    expect(running.pull).toBe('current')
    expect(running.start).toBe('todo')

    const failed = stageStates('start', 'failed')
    expect(failed.start).toBe('failed')

    const rolledBack = stageStates(null, 'rolled_back')
    expect(rolledBack.start).toBe('failed')
    expect(rolledBack.rollback).toBe('done')

    const ok = stageStates(null, 'succeeded')
    expect(ok.prune).toBe('done')
    expect(ok.rollback).toBe('todo')
  })

  it('textos curtos', () => {
    expect(commitTitle('feat: x\n\ncorpo')).toBe('feat: x')
    expect(commitTitle(null)).toBe('')
    expect(windowLabel('23:00', '05:00')).toBe('23:00–05:00')
    expect(windowLabel(null, '05:00')).toBe('')
  })
})

describe('.env no modo lista', () => {
  const ENV = '# banco\nDB_HOST=postgres\nexport DB_PASS="x=y"\n\nlinha estranha\n'

  it('volta ao texto igual ao que era', () => {
    expect(serializeEnv(parseEnv(ENV))).toBe(ENV)
  })

  it('separa variáveis do resto e preserva o = do valor', () => {
    const lines = parseEnv(ENV)
    const variables = lines.filter((line) => line.kind === 'variable')
    expect(variables.map((line) => line.key)).toEqual(['DB_HOST', 'DB_PASS'])
    expect(variables[1]).toMatchObject({ value: '"x=y"', exported: true })
    expect(lines.filter((line) => line.kind === 'other')).toHaveLength(3)
  })

  it('mascara o que parece segredo e valida nomes', () => {
    expect(looksSecret('DB_PASSWORD')).toBe(true)
    expect(looksSecret('APP_KEY')).toBe(true)
    expect(looksSecret('DB_HOST')).toBe(false)
    expect(isValidEnvKey('A_1')).toBe(true)
    expect(isValidEnvKey('1A')).toBe(false)
  })
})

describe('formulário do alvo', () => {
  it('produção nasce com aprovação e rollback, e sem agente não vai à API', () => {
    const form = emptyTarget({ project: 'sgc' })
    expect(form.requiresApproval).toBe(true)
    expect(form.autoRollback).toBe(true)
    expect(toTargetInput(form)).toBeNull()
    const input = toTargetInput({ ...form, probeId: 3, windowStart: '', windowEnd: '' })
    expect(input).toMatchObject({ probeId: 3, project: 'sgc' })
    expect(input?.windowStart).toBeUndefined()
  })
})
