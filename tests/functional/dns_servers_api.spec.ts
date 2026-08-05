import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import DnsServer from '#models/dns_server'
import Monitor from '#models/monitor'

test.group('DNS Servers API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('GET /api/dns/servers deve semear os resolvedores públicos no primeiro acesso', async ({
    client,
    assert,
  }) => {
    const response = await client.get('/api/dns/servers')

    response.assertStatus(200)
    const body = response.body() as Array<{ address: string; isDefault: boolean }>

    assert.isAbove(body.length, 0)
    assert.isTrue(body.some((server) => server.address === '1.1.1.1'))
    assert.isTrue(body.every((server) => server.isDefault))
  })

  test('GET /api/dns/servers não deve semear novamente quando já existe cadastro', async ({
    client,
    assert,
  }) => {
    await DnsServer.create({
      name: 'DNS Interno',
      address: '192.168.1.1',
      protocol: 'udp',
      isDefault: true,
    })

    const response = await client.get('/api/dns/servers')

    response.assertStatus(200)
    const servers = response.body() as Array<{ address: string }>
    assert.lengthOf(servers, 1)
    assert.equal(servers[0]!.address, '192.168.1.1')
  })

  test('POST /api/dns/servers deve cadastrar um servidor novo', async ({ client, assert }) => {
    const response = await client.post('/api/dns/servers').json({
      name: 'DNS da Matriz',
      address: '10.0.0.53',
      protocol: 'udp',
      description: 'Servidor interno',
    })

    response.assertStatus(201)
    const created = response.body() as { name: string; address: string; isDefault: boolean }
    assert.equal(created.name, 'DNS da Matriz')
    assert.equal(created.address, '10.0.0.53')
    assert.isTrue(created.isDefault)
  })

  test('POST /api/dns/servers deve recusar endereço inválido para o transporte', async ({
    client,
    assert,
  }) => {
    const invalido = await client.post('/api/dns/servers').json({
      name: 'Errado',
      address: 'nao é um endereço',
      protocol: 'udp',
    })
    invalido.assertStatus(422)

    const dohSemHttps = await client.post('/api/dns/servers').json({
      name: 'DoH errado',
      address: '1.1.1.1',
      protocol: 'doh',
    })
    dohSemHttps.assertStatus(422)
    assert.include((dohSemHttps.body() as { message: string }).message, 'https://')
  })

  test('POST /api/dns/servers deve recusar duplicado de endereço e protocolo', async ({
    client,
    assert,
  }) => {
    await DnsServer.create({ name: 'Cloudflare', address: '1.1.1.1', protocol: 'udp' })

    const response = await client.post('/api/dns/servers').json({
      name: 'Cloudflare de novo',
      address: '1.1.1.1',
      protocol: 'udp',
    })

    response.assertStatus(409)
    assert.include((response.body() as { message: string }).message, 'já está cadastrado')
  })

  test('POST /api/dns/servers deve aceitar o mesmo endereço em transportes diferentes', async ({
    client,
  }) => {
    await DnsServer.create({ name: 'Cloudflare UDP', address: '1.1.1.1', protocol: 'udp' })

    const response = await client.post('/api/dns/servers').json({
      name: 'Cloudflare TCP',
      address: '1.1.1.1',
      protocol: 'tcp',
    })

    response.assertStatus(201)
  })

  test('PUT /api/dns/servers/:id deve atualizar nome e participação na comparação', async ({
    client,
    assert,
  }) => {
    const server = await DnsServer.create({
      name: 'Antigo',
      address: '10.0.0.53',
      protocol: 'udp',
      isDefault: true,
    })

    const response = await client.put(`/api/dns/servers/${server.id}`).json({
      name: 'DNS Matriz',
      isDefault: false,
    })

    response.assertStatus(200)
    const updated = response.body() as { name: string; address: string; isDefault: boolean }
    assert.equal(updated.name, 'DNS Matriz')
    assert.isFalse(updated.isDefault)
    assert.equal(updated.address, '10.0.0.53')
  })

  test('DELETE /api/dns/servers/:id deve remover o cadastro', async ({ client, assert }) => {
    const server = await DnsServer.create({
      name: 'Temporário',
      address: '10.0.0.54',
      protocol: 'udp',
    })

    const response = await client.delete(`/api/dns/servers/${server.id}`)

    response.assertStatus(204)
    assert.isNull(await DnsServer.find(server.id))
  })

  test('POST /api/dns/benchmark deve comparar todos quando nenhum está marcado', async ({
    client,
    assert,
  }) => {
    await DnsServer.create({
      name: 'Sem marcação A',
      address: '127.0.0.1:9',
      protocol: 'udp',
      isDefault: false,
    })
    await DnsServer.create({
      name: 'Sem marcação B',
      address: '127.0.0.1:10',
      protocol: 'udp',
      isDefault: false,
    })

    const response = await client.post('/api/dns/benchmark').json({
      hostnames: ['servidor.local'],
      timeoutMs: 400,
    })

    response.assertStatus(200)
    const ranking = (response.body() as { ranking: Array<{ label: string }> }).ranking
    assert.lengthOf(ranking, 2)
  }).timeout(10000)

  test('POST /api/dns/benchmark sem lista deve usar os servidores cadastrados', async ({
    client,
    assert,
  }) => {
    await DnsServer.create({
      name: 'Local Indisponível',
      address: '127.0.0.1:9',
      protocol: 'udp',
      isDefault: true,
    })
    await DnsServer.create({
      name: 'Fora da comparação',
      address: '127.0.0.1:10',
      protocol: 'udp',
      isDefault: false,
    })

    const response = await client.post('/api/dns/benchmark').json({
      hostnames: ['servidor.local'],
      timeoutMs: 400,
    })

    response.assertStatus(200)
    const ranking = (response.body() as { ranking: Array<{ label: string }> }).ranking

    // Só o servidor marcado como padrão entra na comparação
    assert.lengthOf(ranking, 1)
    assert.equal(ranking[0].label, 'Local Indisponível')
  }).timeout(10000)

  test('POST /api/monitors deve aceitar monitor sem dispositivo vinculado', async ({
    client,
    assert,
  }) => {
    const response = await client.post('/api/monitors').json({
      name: 'Latência Cloudflare',
      type: 'dns',
      configuration: {
        domain: 'google.com',
        dnsServer: '1.1.1.1',
        protocol: 'udp',
        recordType: 'A',
      },
      intervalSeconds: 300,
      timeoutSeconds: 5,
    })

    response.assertStatus(201)
    const created = response.body() as { id: number; deviceId: number | null }
    assert.isNotOk(created.deviceId)

    const monitor = await Monitor.findOrFail(created.id)
    assert.isNull(monitor.deviceId)
  })
})
