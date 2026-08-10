import { test } from '@japa/runner'
import { ProbeBuffer } from '#modules/probes/probe_buffer'
import fs from 'node:fs'
import path from 'node:path'

test.group('ProbeBuffer - Unit Tests', (group) => {
  const testBufferPath = path.join(process.cwd(), 'tmp', 'test_probe_buffer.json')

  group.each.teardown(async () => {
    if (fs.existsSync(testBufferPath)) {
      fs.unlinkSync(testBufferPath)
    }
  })

  test('deve salvar e ler resultados offline no buffer', async ({ assert }) => {
    const buffer = new ProbeBuffer(testBufferPath)

    await buffer.saveResultOffline('task-1', { status: 'up', rtt: 15 })
    const pending = await buffer.getPendingResults()

    assert.lengthOf(pending, 1)
    assert.equal(pending[0].taskId, 'task-1')
    assert.deepEqual(pending[0].result, { status: 'up', rtt: 15 })
  })

  test('deve criar o diretório recursivamente caso não exista', async ({ assert }) => {
    const nestedBufferPath = path.join(process.cwd(), 'tmp', 'nested', 'dir', 'buffer.json')
    const buffer = new ProbeBuffer(nestedBufferPath)

    await buffer.saveResultOffline('task-nested', { status: 'up' })
    assert.isTrue(fs.existsSync(nestedBufferPath))

    await buffer.clearPendingResults()
    if (fs.existsSync(path.dirname(nestedBufferPath))) {
      fs.rmSync(path.join(process.cwd(), 'tmp', 'nested'), { recursive: true, force: true })
    }
  })

  test('deve limpar resultados do buffer com sucesso', async ({ assert }) => {
    const buffer = new ProbeBuffer(testBufferPath)

    await buffer.saveResultOffline('task-2', { status: 'down' })
    assert.lengthOf(await buffer.getPendingResults(), 1)

    await buffer.clearPendingResults()
    assert.lengthOf(await buffer.getPendingResults(), 0)
  })
})
