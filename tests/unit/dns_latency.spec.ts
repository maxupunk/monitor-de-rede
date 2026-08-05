import { test } from '@japa/runner'
import dgram from 'node:dgram'
import net from 'node:net'
import { Buffer } from 'node:buffer'
import {
  DNS_RECORD_TYPE_CODES,
  DnsProtocolError,
  decodeDnsMessage,
  encodeDnsQuery,
} from '#modules/network_tools/dns/dns_wire'
import {
  benchmarkDnsServers,
  measureDnsLookup,
  parseServerAddress,
  sortByLatency,
} from '#modules/network_tools/dns/dns_latency_service'
import { DnsChecker } from '#modules/monitoring/checkers/dns_checker'

/**
 * Monta uma resposta DNS válida para a consulta recebida, com um registro A
 * apontando para `ip`. Permite exercitar o caminho completo (codificação,
 * transporte e decodificação) sem depender de rede externa.
 */
function buildDnsResponse(query: Buffer, ip = '127.0.0.1', truncated = false): Buffer {
  const header = Buffer.alloc(12)
  header.writeUInt16BE(query.readUInt16BE(0), 0)
  header.writeUInt16BE(truncated ? 0x8380 : 0x8180, 2) // QR=1, RD=1, RA=1 (+TC)
  header.writeUInt16BE(1, 4) // QDCOUNT
  header.writeUInt16BE(1, 6) // ANCOUNT

  const answer = Buffer.alloc(16)
  answer.writeUInt16BE(0xc00c, 0) // ponteiro de compressão para o QNAME
  answer.writeUInt16BE(DNS_RECORD_TYPE_CODES.A, 2)
  answer.writeUInt16BE(1, 4) // CLASS IN
  answer.writeUInt32BE(60, 6) // TTL
  answer.writeUInt16BE(4, 10) // RDLENGTH
  ip.split('.').forEach((octet, index) => answer.writeUInt8(Number(octet), 12 + index))

  return Buffer.concat([header, query.subarray(12), answer])
}

function startUdpServer(
  onQuery: (query: Buffer) => Buffer,
  port = 0
): Promise<{ port: number; close: () => Promise<void> }> {
  return new Promise((resolve, reject) => {
    const socket = dgram.createSocket('udp4')
    socket.on('error', reject)
    socket.on('message', (message, remote) => {
      socket.send(onQuery(message), remote.port, remote.address)
    })
    socket.bind(port, '127.0.0.1', () => {
      resolve({
        port: socket.address().port,
        close: () => new Promise<void>((done) => socket.close(() => done())),
      })
    })
  })
}

function startTcpServer(
  onQuery: (query: Buffer) => Buffer,
  port = 0
): Promise<{ port: number; close: () => Promise<void> }> {
  return new Promise((resolve, reject) => {
    const server = net.createServer((socket) => {
      const chunks: Buffer[] = []
      socket.on('data', (chunk: Buffer) => {
        chunks.push(chunk)
        const buffer = Buffer.concat(chunks)
        if (buffer.length < 2) return
        const expected = buffer.readUInt16BE(0)
        if (buffer.length < expected + 2) return

        const response = onQuery(buffer.subarray(2, expected + 2))
        const framed = Buffer.alloc(2 + response.length)
        framed.writeUInt16BE(response.length, 0)
        response.copy(framed, 2)
        socket.end(framed)
      })
      socket.on('error', () => socket.destroy())
    })

    server.on('error', reject)
    server.listen(port, '127.0.0.1', () => {
      const address = server.address()
      resolve({
        port: typeof address === 'object' && address ? address.port : 0,
        close: () => new Promise<void>((done) => server.close(() => done())),
      })
    })
  })
}

test.group('DNS wire format', () => {
  test('encodeDnsQuery deve montar cabeçalho e QNAME conforme a RFC 1035', ({ assert }) => {
    const query = encodeDnsQuery('exemplo.com.br', 'A', 4321)

    assert.equal(query.readUInt16BE(0), 4321)
    assert.equal(query.readUInt16BE(2), 0x0100) // RD=1
    assert.equal(query.readUInt16BE(4), 1) // QDCOUNT
    assert.equal(query[12], 7) // tamanho do rótulo "exemplo"
    assert.equal(query.subarray(13, 20).toString('ascii'), 'exemplo')
    assert.equal(query.readUInt16BE(query.length - 4), DNS_RECORD_TYPE_CODES.A)
    assert.equal(query.readUInt16BE(query.length - 2), 1) // CLASS IN
  })

  test('encodeDnsQuery deve recusar hostnames inválidos', ({ assert }) => {
    assert.throws(() => encodeDnsQuery('', 'A'), 'Hostname vazio')
    assert.throws(() => encodeDnsQuery('site..com', 'A'))
    assert.throws(() => encodeDnsQuery(`${'a'.repeat(64)}.com`, 'A'))
  })

  test('decodeDnsMessage deve ler registros A resolvendo ponteiros de compressão', ({ assert }) => {
    const query = encodeDnsQuery('exemplo.com', 'A', 10)
    const message = decodeDnsMessage(buildDnsResponse(query, '203.0.113.7'))

    assert.equal(message.id, 10)
    assert.equal(message.rcode, 0)
    assert.equal(message.rcodeLabel, 'NOERROR')
    assert.isFalse(message.truncated)
    assert.lengthOf(message.answers, 1)
    assert.equal(message.answers[0]!.name, 'exemplo.com')
    assert.equal(message.answers[0]!.type, 'A')
    assert.equal(message.answers[0]!.value, '203.0.113.7')
  })

  test('decodeDnsMessage deve expor o RCODE de erro devolvido pelo servidor', ({ assert }) => {
    const query = encodeDnsQuery('inexistente.dev', 'A', 11)
    const response = buildDnsResponse(query)
    response.writeUInt16BE(0x8183, 2) // RCODE=3 (NXDOMAIN)

    const message = decodeDnsMessage(response)
    assert.equal(message.rcode, 3)
    assert.equal(message.rcodeLabel, 'NXDOMAIN')
  })

  test('decodeDnsMessage deve rejeitar respostas menores que o cabeçalho', ({ assert }) => {
    assert.throws(() => decodeDnsMessage(Buffer.alloc(4)), DnsProtocolError)
  })
})

test.group('parseServerAddress', () => {
  test('deve separar host e porta preservando IPv6', ({ assert }) => {
    assert.deepEqual(parseServerAddress('1.1.1.1'), { host: '1.1.1.1', port: 53 })
    assert.deepEqual(parseServerAddress('1.1.1.1:5353'), { host: '1.1.1.1', port: 5353 })
    assert.deepEqual(parseServerAddress('[::1]:5353'), { host: '::1', port: 5353 })
    assert.deepEqual(parseServerAddress('2606:4700:4700::1111'), {
      host: '2606:4700:4700::1111',
      port: 53,
    })
  })

  test('deve recusar servidor vazio', ({ assert }) => {
    assert.throws(() => parseServerAddress('  '), 'Servidor DNS não informado')
  })
})

test.group('measureDnsLookup', () => {
  test('deve medir a resolução via UDP contra um servidor local', async ({ assert }) => {
    const server = await startUdpServer((query) => buildDnsResponse(query, '198.51.100.10'))

    try {
      const sample = await measureDnsLookup({
        hostname: 'servidor.local',
        server: `127.0.0.1:${server.port}`,
        protocol: 'udp',
        timeoutMs: 2000,
      })

      assert.isTrue(sample.success)
      assert.deepEqual(sample.addresses, ['198.51.100.10'])
      assert.equal(sample.rcodeLabel, 'NOERROR')
      assert.isAbove(sample.lookupTimeMs, 0)
      assert.isBelow(sample.lookupTimeMs, 2000)
      assert.isNull(sample.error)
      assert.isFalse(sample.usedTcpFallback)
    } finally {
      await server.close()
    }
  }).timeout(5000)

  test('deve medir a resolução via TCP respeitando o prefixo de tamanho', async ({ assert }) => {
    const server = await startTcpServer((query) => buildDnsResponse(query, '198.51.100.20'))

    try {
      const sample = await measureDnsLookup({
        hostname: 'servidor.local',
        server: `127.0.0.1:${server.port}`,
        protocol: 'tcp',
        timeoutMs: 2000,
      })

      assert.isTrue(sample.success)
      assert.deepEqual(sample.addresses, ['198.51.100.20'])
      assert.isAbove(sample.lookupTimeMs, 0)
    } finally {
      await server.close()
    }
  }).timeout(5000)

  test('deve repetir a consulta via TCP quando a resposta UDP vem truncada', async ({ assert }) => {
    const tcpServer = await startTcpServer((query) => buildDnsResponse(query, '198.51.100.30'))
    const udpServer = await startUdpServer(
      (query) => buildDnsResponse(query, '198.51.100.30', true),
      tcpServer.port
    )

    try {
      const sample = await measureDnsLookup({
        hostname: 'servidor.local',
        server: `127.0.0.1:${tcpServer.port}`,
        protocol: 'udp',
        timeoutMs: 2000,
      })

      assert.isTrue(sample.usedTcpFallback)
      assert.isTrue(sample.success)
      assert.deepEqual(sample.addresses, ['198.51.100.30'])
    } finally {
      await udpServer.close()
      await tcpServer.close()
    }
  }).timeout(5000)

  test('deve devolver falha estruturada quando o servidor não responde', async ({ assert }) => {
    const sample = await measureDnsLookup({
      hostname: 'servidor.local',
      server: '127.0.0.1:9',
      protocol: 'tcp',
      timeoutMs: 500,
    })

    assert.isFalse(sample.success)
    assert.isNotNull(sample.error)
    assert.isEmpty(sample.addresses)
    assert.isAbove(sample.lookupTimeMs, 0)
  }).timeout(5000)

  test('deve reportar erro de configuração sem lançar exceção', async ({ assert }) => {
    const semServidor = await measureDnsLookup({ hostname: 'exemplo.com', protocol: 'udp' })
    assert.isFalse(semServidor.success)
    assert.equal(semServidor.error, 'Servidor DNS não informado')

    const semHostname = await measureDnsLookup({ hostname: '', server: '1.1.1.1' })
    assert.isFalse(semHostname.success)
    assert.equal(semHostname.error, 'Hostname não informado')

    const semEndpoint = await measureDnsLookup({ hostname: 'exemplo.com', protocol: 'doh' })
    assert.isFalse(semEndpoint.success)
    assert.equal(semEndpoint.error, 'Endpoint DoH não informado')
  })
})

test.group('Ranking de servidores DNS', () => {
  test('sortByLatency deve ordenar do menor tempo ao maior e jogar falhas para o fim', ({
    assert,
  }) => {
    const ordenado = sortByLatency([
      { server: 'lento', avgLookupTimeMs: 120 },
      { server: 'inacessivel', avgLookupTimeMs: null },
      { server: 'rapido', avgLookupTimeMs: 12.5 },
    ])

    assert.deepEqual(
      ordenado.map((item) => item.server),
      ['rapido', 'lento', 'inacessivel']
    )
  })

  test('benchmarkDnsServers deve agregar as medições por servidor', async ({ assert }) => {
    const rapido = await startUdpServer((query) => buildDnsResponse(query, '198.51.100.40'))
    const indisponivel = { server: '127.0.0.1:9', label: 'Indisponível', protocol: 'udp' as const }

    try {
      const ranking = await benchmarkDnsServers({
        servers: [
          indisponivel,
          { server: `127.0.0.1:${rapido.port}`, label: 'Local', protocol: 'udp' },
        ],
        hostnames: ['servidor.local', 'outro.local'],
        timeoutMs: 500,
      })

      assert.lengthOf(ranking, 2)
      assert.equal(ranking[0]!.label, 'Local')
      assert.equal(ranking[0]!.totalQueries, 2)
      assert.equal(ranking[0]!.failedQueries, 0)
      assert.equal(ranking[0]!.successRate, 1)
      assert.isNotNull(ranking[0]!.avgLookupTimeMs)
      assert.isNotNull(ranking[0]!.medianLookupTimeMs)

      assert.equal(ranking[1]!.label, 'Indisponível')
      assert.isNull(ranking[1]!.avgLookupTimeMs)
      assert.equal(ranking[1]!.successRate, 0)
      assert.isNotNull(ranking[1]!.error)
    } finally {
      await rapido.close()
    }
  }).timeout(10000)
})

test.group('DnsChecker', () => {
  test('deve medir vários hostnames e publicar a métrica de lookup', async ({ assert }) => {
    const server = await startUdpServer((query) => buildDnsResponse(query, '198.51.100.50'))

    try {
      const result = await new DnsChecker().execute({
        domains: ['um.local', 'dois.local'],
        dnsServer: `127.0.0.1:${server.port}`,
        protocol: 'udp',
        timeoutMs: 2000,
      })

      assert.equal(result.status, 'up')
      assert.isTrue(result.success)

      const lookupMetric = result.metrics?.find((metric) => metric.name === 'dns_lookup_time')
      assert.isDefined(lookupMetric)
      assert.isAbove(lookupMetric!.value, 0)

      // Nome histórico preservado para as regras de alerta já cadastradas
      assert.isDefined(result.metrics?.find((metric) => metric.name === 'resolution_time'))
      assert.equal(
        result.metrics?.find((metric) => metric.name === 'dns_success_rate')?.value,
        100
      )

      const data = result.data as Record<string, unknown>
      assert.equal(data.protocol, 'udp')
      assert.lengthOf(data.lookups as unknown[], 2)
    } finally {
      await server.close()
    }
  }).timeout(10000)

  test('deve entrar em warning quando parte dos nomes falha', async ({ assert }) => {
    const server = await startUdpServer((query) => {
      const message = decodeDnsMessage(query)
      const response = buildDnsResponse(query)
      // Só o primeiro nome resolve; o segundo devolve NXDOMAIN
      if (message.answers.length === 0 && query.subarray(12).toString('ascii').includes('falha')) {
        response.writeUInt16BE(0x8183, 2)
      }
      return response
    })

    try {
      const result = await new DnsChecker().execute({
        domains: ['ok.local', 'falha.local'],
        dnsServer: `127.0.0.1:${server.port}`,
        protocol: 'udp',
        timeoutMs: 2000,
      })

      assert.equal(result.status, 'warning')
      assert.include(result.message, 'falha.local')
    } finally {
      await server.close()
    }
  }).timeout(10000)

  test('deve entrar em warning quando a latência passa do limite configurado', async ({
    assert,
  }) => {
    const server = await startUdpServer((query) => buildDnsResponse(query))

    try {
      const result = await new DnsChecker().execute({
        domain: 'lento.local',
        dnsServer: `127.0.0.1:${server.port}`,
        protocol: 'udp',
        timeoutMs: 2000,
        // Qualquer resposta real passa de 0ms, então o limite dispara sempre
        warningThresholdMs: 0.0001,
      })

      assert.equal(result.status, 'warning')
    } finally {
      await server.close()
    }
  }).timeout(10000)

  test('deve reportar down quando nenhum hostname resolve', async ({ assert }) => {
    const result = await new DnsChecker().execute({
      domain: 'inacessivel.local',
      dnsServer: '127.0.0.1:9',
      protocol: 'tcp',
      timeoutMs: 500,
    })

    assert.equal(result.status, 'down')
    assert.isFalse(result.success)
    assert.include(result.message!, 'Falha ao resolver')
  }).timeout(5000)
})
