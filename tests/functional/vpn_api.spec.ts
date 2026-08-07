import { test } from '@japa/runner'
import { DateTime } from 'luxon'
import type { ApiClient } from '@japa/api-client'
import testUtils from '@adonisjs/core/services/test_utils'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'
import Probe from '#models/probe'
import VpnServer from '#models/vpn_server'
import VpnPeer from '#models/vpn_peer'
import { derivePublicKey } from '#modules/vpn/key_generator'
import { PeerStatusService } from '#modules/vpn/peer_status'

test.group('VPN API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  async function configureServer(httpClient: ApiClient) {
    const site = await Site.create({ name: 'Matriz', active: true })

    const response = await httpClient.visit('vpn_servers.update').json({
      siteId: site.id,
      cidr: '10.8.0.0/24',
      listenPort: 51820,
      publicEndpoint: 'vpn.exemplo.com.br',
      mtu: 1420,
      allowPeerToPeer: false,
    })

    return { site, response }
  }

  test('PUT /api/vpn/server deve criar o servidor com par de chaves e rede da VPN', async ({
    client: httpClient,
    assert,
  }) => {
    const { response } = await configureServer(httpClient)

    response.assertStatus(200)
    assert.equal(response.body().cidr, '10.8.0.0/24')
    assert.equal(response.body().serverAddress, '10.8.0.1')

    const server = await VpnServer.query().firstOrFail()
    assert.isNotEmpty(server.publicKey)
    // A privada é cifrada em repouso, mas volta decifrada pelo model.
    assert.equal(derivePublicKey(server.privateKey), server.publicKey)
    // E nunca é serializada para a API.
    assert.notProperty(response.body().server, 'privateKey')
  })

  test('POST /api/vpn/peers deve provisionar device, peer e monitores no IP alocado', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const response = await httpClient.visit('vpn_peers.store').json({
      name: 'MikroTik Filial',
      profile: 'mikrotik',
      snmpEnabled: true,
      snmpCommunity: 'netmonitor',
    })

    response.assertStatus(201)
    assert.equal(response.body().device.ipAddress, '10.8.0.2')
    assert.equal(response.body().peer.persistentKeepalive, 25)

    const artifact = response.body().artifact
    assert.include(artifact.content, 'persistent-keepalive=25s')
    assert.include(artifact.content, 'allowed-address=10.8.0.0/24')
    assert.notInclude(artifact.content, '0.0.0.0/0')

    const device = await Device.query().where('name', 'MikroTik Filial').firstOrFail()
    assert.equal(device.type, 'router')
    // SQLite devolve 1/0 para colunas booleanas — isOk cobre os dois drivers.
    assert.isOk(device.isMonitored)

    const monitors = await Monitor.query().where('deviceId', device.id)
    assert.lengthOf(monitors, 2)
    assert.includeMembers(
      monitors.map((monitor) => monitor.type),
      ['ping', 'snmp']
    )
  })

  test('IPs devem ser alocados sequencialmente sem colisão', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    for (const name of ['Peer A', 'Peer B', 'Peer C']) {
      const response = await httpClient.visit('vpn_peers.store').json({ name, profile: 'openwrt' })
      response.assertStatus(201)
    }

    const devices = await Device.query().orderBy('id', 'asc')
    assert.deepEqual(
      devices.map((device) => device.ipAddress),
      ['10.8.0.2', '10.8.0.3', '10.8.0.4']
    )
  })

  test('POST /api/vpn/peers deve entregar o QR Code do celular junto do artefato', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const created = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Celular Suporte', profile: 'mobile' })

    created.assertStatus(201)
    const artifact = created.body().artifact

    // Sem isso o QR só sairia numa segunda requisição — que já não teria a chave privada.
    assert.isTrue(artifact.supportsQrCode)
    assert.include(artifact.qrSvg, '<svg')
    assert.notInclude(artifact.content, 'CHAVE-PRIVADA-INDISPONIVEL')

    // A primeira leitura ainda traz a chave; depois dela o QR deixa de ser
    // gerado, porque apontaria para um túnel que nunca conecta.
    const peerId = created.body().peer.id
    await httpClient.visit('vpn_peers.config', { id: peerId })

    const exhausted = await httpClient.visit('vpn_peers.config', { id: peerId })
    assert.include(exhausted.body()!.content, 'CHAVE-PRIVADA-INDISPONIVEL')
    assert.isNull(exhausted.body()!.qrSvg)
  })

  test('POST /api/vpn/peers deve gerar os scripts de terminal por gerenciador de pacotes', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const windows = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Notebook Diretoria', profile: 'windows' })

    const wingetIds = windows.body().artifact.variants.map((variant: { id: string }) => variant.id)
    assert.deepEqual(wingetIds, ['winget'])
    assert.isNull(windows.body().artifact.qrSvg)

    const openwrt = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Roteador Loja', profile: 'openwrt' })

    const openwrtIds = openwrt.body().artifact.variants.map((variant: { id: string }) => variant.id)
    assert.deepEqual(openwrtIds, ['opkg', 'apk'])
  })

  test('PATCH /api/vpn/peers/:id deve renomear o dispositivo e acompanhar os monitores', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const created = await httpClient.visit('vpn_peers.store').json({
      name: 'Filial Centro',
      profile: 'mikrotik',
      snmpEnabled: true,
      snmpCommunity: 'netmonitor',
    })
    const peerId = created.body().peer.id
    const deviceId = created.body().device.id

    const renamed = await httpClient.visit('vpn_peers.update', { id: peerId }).json({
      name: '  Filial Centro-Sul  ',
    })

    renamed.assertStatus(200)
    assert.equal(renamed.body()!.device.name, 'Filial Centro-Sul')

    const device = await Device.findOrFail(deviceId)
    assert.equal(device.name, 'Filial Centro-Sul')
    // IP e chaves não podem se mover: o túnel está no ar.
    assert.equal(device.ipAddress, '10.8.0.2')

    const monitors = await Monitor.query().where('deviceId', deviceId).orderBy('type', 'asc')
    assert.deepEqual(
      monitors.map((monitor) => monitor.name),
      ['Ping Filial Centro-Sul', 'SNMP Filial Centro-Sul']
    )

    // O monitor SNMP precisa manter community e versão — é onde o update
    // genérico de /api/devices/:id estragaria o peer.
    const snmp = monitors.find((monitor) => monitor.type === 'snmp')!
    assert.equal(snmp.configuration.community, 'netmonitor')
    assert.equal(snmp.configuration.version, 'v2c')
  })

  test('PATCH /api/vpn/peers/:id deve recusar nome vazio', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const created = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Peer Fixo', profile: 'linux' })

    const response = await httpClient
      .visit('vpn_peers.update', { id: created.body().peer.id })
      .json({ name: '   ' })

    response.assertStatus(400)

    const device = await Device.findOrFail(created.body().device.id)
    assert.equal(device.name, 'Peer Fixo')
  })

  test('GET /api/vpn/peers não deve culpar o firewall quando o ping roda fora do túnel', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const created = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Roteador Silencioso', profile: 'mikrotik' })

    // connectionStatus é derivado do handshake recente.
    const peer = await VpnPeer.findOrFail(created.body().peer.id)
    peer.lastHandshakeAt = DateTime.now()
    await peer.save()
    assert.equal(peer.connectionStatus, 'connected')

    const ping = await Monitor.query()
      .where('deviceId', peer.deviceId)
      .where('type', 'ping')
      .firstOrFail()
    ping.status = 'down'
    await ping.save()

    // Sem vpn-probe registrado, o monitor nasce com probeId nulo: o ICMP parte da
    // API, que não tem rota para a VPN, e nunca chega ao equipamento.
    assert.isNull(ping.probeId)

    const listed = await httpClient.visit('vpn_peers.index')
    const item = listed.body()[0]

    assert.isTrue(item.pingOutsideTunnel)
    assert.isFalse(item.needsFirewallHint)

    // Com o monitor no vpn-probe, aí sim o silêncio aponta para o firewall.
    const probe = await Probe.create({ name: 'vpn-probe', tokenHash: 'hash', status: 'online' })
    ping.probeId = probe.id
    await ping.save()

    const reListed = await httpClient.visit('vpn_peers.index')
    assert.isTrue(reListed.body()[0].needsFirewallHint)
    assert.isFalse(reListed.body()[0].pingOutsideTunnel)
  })

  test('syncPeers deve tratar o keepalive como sinal de vida, não o handshake', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const created = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Notebook Ocioso', profile: 'windows' })

    const peerId = created.body().peer.id
    const publicKey = created.body().peer.publicKey

    // Handshake de 7 minutos atrás: num túnel ocioso o WireGuard não renegocia
    // chaves, então esse é o quadro normal de um cliente perfeitamente conectado.
    const staleHandshake = Math.floor(DateTime.now().minus({ minutes: 7 }).toSeconds())
    const dumpWith = (bytesRx: number) =>
      [
        'CHAVE-PRIV\tCHAVE-PUB-SERVIDOR\t51820\toff',
        `${publicKey}\tPSK\t189.10.0.5:4820\t10.8.0.2/32\t${staleHandshake}\t${bytesRx}\t2048\t25`,
      ].join('\n')

    let dump = dumpWith(1024)
    const sink = {
      read: async () => dump,
      write: async () => {},
    }
    const statusService = new PeerStatusService(sink)
    const server = await VpnServer.query().firstOrFail()

    await statusService.syncPeers(server.interfaceName, server.id)

    let peer = await VpnPeer.findOrFail(peerId)
    assert.isNotNull(peer.lastSeenAt)
    assert.equal(peer.connectionStatus, 'connected')

    // Contador parado: nenhum keepalive chegou desde a última leitura.
    peer.lastSeenAt = DateTime.now().minus({ minutes: 5 })
    await peer.save()
    await statusService.syncPeers(server.interfaceName, server.id)

    peer = await VpnPeer.findOrFail(peerId)
    assert.equal(peer.connectionStatus, 'unstable')

    // Keepalive contabilizado: volta a conectado sem depender de handshake novo.
    dump = dumpWith(1056)
    await statusService.syncPeers(server.interfaceName, server.id)

    peer = await VpnPeer.findOrFail(peerId)
    assert.equal(peer.connectionStatus, 'connected')
  })

  test('GET /api/vpn/peers/:id/config deve entregar a chave privada apenas uma vez', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const created = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Peer Linux', profile: 'linux' })
    const peerId = created.body().peer.id

    const first = await httpClient.visit('vpn_peers.config', { id: peerId })
    first.assertStatus(200)
    assert.notInclude(first.body()!.content, 'CHAVE-PRIVADA-INDISPONIVEL')

    const second = await httpClient.visit('vpn_peers.config', { id: peerId })
    second.assertStatus(200)
    assert.include(second.body()!.content, 'CHAVE-PRIVADA-INDISPONIVEL')
  })

  test('POST /api/vpn/peers/:id/rotate deve trocar a chave pública do peer', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const created = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Peer Rotativo', profile: 'linux' })
    const peerId = created.body().peer.id
    const originalPublicKey = created.body().peer.publicKey

    const rotated = await httpClient.visit('vpn_peers.rotate', { id: peerId })
    rotated.assertStatus(200)
    assert.notEqual(rotated.body()!.peer.publicKey, originalPublicKey)

    const peer = await VpnPeer.findOrFail(peerId)
    assert.equal(peer.publicKey, rotated.body()!.peer.publicKey)
  })

  test('POST /api/vpn/peers/:id/firewall-hints deve devolver as regras do perfil', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const created = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Peer MikroTik', profile: 'mikrotik' })

    const response = await httpClient.visit('vpn_peers.firewall_hints', {
      id: created.body().peer.id,
    })

    response.assertStatus(200)
    assert.include(response.body().content, 'protocol=icmp')
    assert.include(response.body().content, 'dst-port=161')
  })

  test('DELETE /api/vpn/peers/:id deve revogar o peer e liberar o IP', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)

    const created = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Peer Revogado', profile: 'linux' })
    const peerId = created.body().peer.id

    const response = await httpClient.visit('vpn_peers.destroy', { id: peerId })
    response.assertStatus(200)

    assert.isNull(await VpnPeer.find(peerId))
    assert.lengthOf(await Device.query().where('name', 'Peer Revogado'), 0)

    // IP liberado: o próximo peer reaproveita o endereço.
    const recreated = await httpClient
      .visit('vpn_peers.store')
      .json({ name: 'Peer Novo', profile: 'linux' })
    assert.equal(recreated.body().device.ipAddress, '10.8.0.2')
  })

  test('GET /api/vpn/server deve refletir o estado agregado dos peers', async ({
    client: httpClient,
    assert,
  }) => {
    await configureServer(httpClient)
    await httpClient.visit('vpn_peers.store').json({ name: 'Peer Estado', profile: 'linux' })

    const response = await httpClient.visit('vpn_servers.show')

    response.assertStatus(200)
    assert.isTrue(response.body().configured)
    assert.equal(response.body().peersTotal, 1)
    assert.equal(response.body().peersConnected, 0)
    assert.equal(response.body().persistentKeepalive, 25)
    assert.lengthOf(response.body().profiles, 5)
  })
})
