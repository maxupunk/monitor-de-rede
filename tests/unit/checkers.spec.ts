import { test } from '@japa/runner'
import { PingChecker } from '#modules/monitoring/checkers/ping_checker'
import { HttpChecker } from '#modules/monitoring/checkers/http_checker'
import { TcpChecker } from '#modules/monitoring/checkers/tcp_checker'
import { DnsChecker } from '#modules/monitoring/checkers/dns_checker'
import { MonitorRunner } from '#modules/monitoring/monitor_runner'

test.group('Checkers de Monitoramento - Unit Tests', () => {
  test('PingChecker deve retornar um resultado de check estruturado', async ({ assert }) => {
    const checker = new PingChecker()
    const result = await checker.execute({ host: '127.0.0.1', packetCount: 1, timeoutMs: 1000 })

    assert.exists(result.startedAt)
    assert.exists(result.finishedAt)
    assert.isNumber(result.durationMs)
    assert.isArray(result.metrics)
  })

  test('HttpChecker deve executar requisição HTTP e medir o tempo de resposta', async ({ assert }) => {
    const checker = new HttpChecker()
    const result = await checker.execute({
      url: 'http://127.0.0.1:3333/',
      method: 'GET',
      timeoutMs: 1000,
    })

    assert.exists(result.startedAt)
    assert.isNumber(result.durationMs)
    assert.isNotNull(result.metrics)
  })

  test('TcpChecker deve medir tempo de tentativa de conexão socket', async ({ assert }) => {
    const checker = new TcpChecker()
    const result = await checker.execute({
      host: '127.0.0.1',
      port: 3333,
      timeoutMs: 1000,
    })

    assert.exists(result.startedAt)
    assert.isNumber(result.durationMs)
  })

  test('DnsChecker deve resolver domínio ou tratar erro de timeout', async ({ assert }) => {
    const checker = new DnsChecker()
    const result = await checker.execute({
      domain: 'localhost',
      recordType: 'A',
      timeoutMs: 1000,
    })

    assert.exists(result.startedAt)
    assert.isNumber(result.durationMs)
  })

  test('MonitorRunner deve despachar para o checker correto baseado no tipo', async ({ assert }) => {
    const runner = new MonitorRunner()

    const pingResult = await runner.runMonitor('ping', { host: '127.0.0.1', packetCount: 1, timeoutMs: 500 })
    assert.exists(pingResult.startedAt)

    const tcpResult = await runner.runMonitor('tcp', { host: '127.0.0.1', port: 3333, timeoutMs: 500 })
    assert.exists(tcpResult.startedAt)

    await assert.rejects(async () => {
      await runner.runMonitor('invalido', {})
    }, 'Tipo de monitor desconhecido ou não suportado: invalido')
  }).timeout(10000)
})
