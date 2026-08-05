import { test } from '@japa/runner'
import type { ApiClient } from '@japa/api-client'
import testUtils from '@adonisjs/core/services/test_utils'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'
import VpnServer from '#models/vpn_server'
import VpnPeer from '#models/vpn_peer'
import { derivePublicKey } from '#modules/vpn/key_generator'

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
