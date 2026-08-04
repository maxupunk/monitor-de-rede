import { test } from '@japa/runner'
import {
  generateKeyPair,
  derivePublicKey,
  generatePresharedKey,
  isValidKey,
} from '#modules/vpn/key_generator'
import {
  parseCidr,
  firstUsableAddress,
  isIpInCidr,
  iterateUsableAddresses,
} from '#modules/vpn/cidr'
import { WireGuardConfigBuilder } from '#modules/vpn/config_builder'
import { MikrotikProfileGenerator } from '#modules/vpn/profiles/mikrotik'
import { OpenWrtProfileGenerator } from '#modules/vpn/profiles/openwrt'
import { createLinuxGenerator, createMobileGenerator } from '#modules/vpn/profiles/wg_conf'
import { ProfileRegistry } from '#modules/vpn/profiles/profile_registry'
import {
  PERSISTENT_KEEPALIVE_SECONDS,
  type PeerConfigContext,
} from '#modules/vpn/profiles/profile_contract'
import { parseWgDump } from '#modules/vpn/peer_status'
import { IpAllocator } from '#modules/vpn/ip_allocator'
import { EphemeralSecretStore } from '#modules/vpn/secret_store'
import { SlidingWindowRateLimiter } from '#modules/vpn/access_control'
import { isCgnatAddress, isPrivateAddress } from '#modules/vpn/preflight'

const context: PeerConfigContext = {
  peerName: 'Roteador Filial',
  peerIpAddress: '10.8.0.11',
  vpnCidr: '10.8.0.0/24',
  serverVpnAddress: '10.8.0.1',
  clientPrivateKey: 'CHAVE-PRIVADA-CLIENTE',
  serverPublicKey: 'CHAVE-PUBLICA-SERVIDOR',
  presharedKey: 'CHAVE-PSK',
  endpointHost: 'vpn.exemplo.com.br',
  endpointPort: 51820,
  mtu: 1420,
  dnsServers: null,
  snmpEnabled: true,
  snmpCommunity: 'netmonitor',
}

test.group('VPN - Geração de chaves X25519', () => {
  test('generateKeyPair deve produzir chaves válidas de 32 bytes', ({ assert }) => {
    const { privateKey, publicKey } = generateKeyPair()

    assert.isTrue(isValidKey(privateKey))
    assert.isTrue(isValidKey(publicKey))
    assert.notEqual(privateKey, publicKey)
  })

  test('derivePublicKey deve reproduzir a pública do par (equivalente a wg pubkey)', ({
    assert,
  }) => {
    const { privateKey, publicKey } = generateKeyPair()

    assert.equal(derivePublicKey(privateKey), publicKey)
  })

  test('generatePresharedKey deve gerar chaves simétricas distintas', ({ assert }) => {
    const first = generatePresharedKey()
    const second = generatePresharedKey()

    assert.isTrue(isValidKey(first))
    assert.notEqual(first, second)
  })

  test('isValidKey deve rejeitar valores fora do formato WireGuard', ({ assert }) => {
    assert.isFalse(isValidKey('chave-invalida'))
    assert.isFalse(isValidKey(''))
  })
})

test.group('VPN - Cálculo de CIDR (IPAM)', () => {
  test('parseCidr deve calcular rede, broadcast e máscara', ({ assert }) => {
    const range = parseCidr('10.8.0.0/24')

    assert.equal(range.networkAddress, '10.8.0.0')
    assert.equal(range.broadcastAddress, '10.8.0.255')
    assert.equal(range.netmask, '255.255.255.0')
    assert.equal(range.usableHosts, 254)
  })

  test('firstUsableAddress deve devolver o endereço do servidor VPN', ({ assert }) => {
    assert.equal(firstUsableAddress('10.8.0.0/24'), '10.8.0.1')
    assert.equal(firstUsableAddress('192.168.100.0/28'), '192.168.100.1')
  })

  test('isIpInCidr deve validar pertencimento à faixa', ({ assert }) => {
    assert.isTrue(isIpInCidr('10.8.0.55', '10.8.0.0/24'))
    assert.isFalse(isIpInCidr('10.9.0.55', '10.8.0.0/24'))
  })

  test('iterateUsableAddresses deve excluir rede e broadcast', ({ assert }) => {
    const addresses = [...iterateUsableAddresses('10.8.0.0/29')]

    assert.deepEqual(addresses, [
      '10.8.0.1',
      '10.8.0.2',
      '10.8.0.3',
      '10.8.0.4',
      '10.8.0.5',
      '10.8.0.6',
    ])
  })

  test('parseCidr deve rejeitar entradas inválidas', ({ assert }) => {
    assert.throws(() => parseCidr('10.8.0.0/33'))
    assert.throws(() => parseCidr('999.1.1.0/24'))
  })
})

test.group('VPN - Alocador de IP', () => {
  test('deve reconhecer violações de unicidade de PostgreSQL e SQLite', ({ assert }) => {
    assert.isTrue(IpAllocator.isUniqueViolation({ code: '23505' }))
    assert.isTrue(IpAllocator.isUniqueViolation({ code: 'SQLITE_CONSTRAINT_UNIQUE' }))
    assert.isTrue(
      IpAllocator.isUniqueViolation(new Error('UNIQUE constraint failed: devices.ip_address'))
    )
    assert.isFalse(IpAllocator.isUniqueViolation(new Error('conexão recusada')))
  })

  test('deve tentar o próximo IP quando outra transação vence a corrida', async ({ assert }) => {
    class FakeAllocator extends IpAllocator {
      async findNextFree(_networkId: number, _cidr: string, reserved: string[] = []) {
        const pool = ['10.8.0.2', '10.8.0.3', '10.8.0.4']
        const free = pool.find((ip) => !reserved.includes(ip))
        if (!free) throw new Error('sem endereços')
        return free
      }
    }

    const allocator = new FakeAllocator()
    const attempted: string[] = []

    const result = await allocator.allocate(1, '10.8.0.0/24', async (ipAddress) => {
      attempted.push(ipAddress)
      if (attempted.length < 3) {
        throw Object.assign(new Error('duplicate key'), { code: '23505' })
      }
      return ipAddress
    })

    assert.deepEqual(attempted, ['10.8.0.2', '10.8.0.3', '10.8.0.4'])
    assert.equal(result, '10.8.0.4')
  })
})

test.group('VPN - Geração do wg0.conf do servidor', () => {
  const builder = new WireGuardConfigBuilder()
  const server = {
    interfaceName: 'wg0',
    address: '10.8.0.1',
    cidr: '10.8.0.0/24',
    listenPort: 51820,
    privateKey: 'CHAVE-PRIVADA-SERVIDOR',
    mtu: 1420,
    allowPeerToPeer: false,
  }

  test('deve montar a seção [Interface] com endereço, porta e MTU', ({ assert }) => {
    const config = builder.buildInterfaceSection(server)

    assert.include(config, 'Address = 10.8.0.1/24')
    assert.include(config, 'ListenPort = 51820')
    assert.include(config, 'MTU = 1420')
  })

  test('modo isolado deve dropar tráfego entre peers', ({ assert }) => {
    const config = builder.buildInterfaceSection(server)

    assert.include(config, 'iptables -A FORWARD -i wg0 -d 10.8.0.1 -j ACCEPT')
    assert.include(config, 'iptables -A FORWARD -i wg0 -o wg0 -j DROP')
  })

  test('modo visível deve permitir tráfego entre peers', ({ assert }) => {
    const config = builder.buildInterfaceSection({ ...server, allowPeerToPeer: true })

    assert.include(config, 'iptables -A FORWARD -i wg0 -o wg0 -j ACCEPT')
    assert.notInclude(config, '-j DROP')
  })

  test('cada peer deve receber AllowedIPs /32 e peers desabilitados são omitidos', ({ assert }) => {
    const config = builder.build(server, [
      { name: 'Filial A', publicKey: 'PUB-A', presharedKey: 'PSK-A', ipAddress: '10.8.0.11' },
      { name: 'Filial B', publicKey: 'PUB-B', ipAddress: '10.8.0.12', enabled: false },
    ])

    assert.include(config, 'AllowedIPs = 10.8.0.11/32')
    assert.include(config, 'PresharedKey = PSK-A')
    assert.notInclude(config, 'PUB-B')
  })
})

test.group('VPN - Geradores por perfil', () => {
  test('MikroTik: keepalive obrigatório de 25s e AllowedIPs restrito ao CIDR', ({ assert }) => {
    const artifact = new MikrotikProfileGenerator().generate(context)

    assert.include(artifact.content, `persistent-keepalive=${PERSISTENT_KEEPALIVE_SECONDS}s`)
    assert.include(artifact.content, 'allowed-address=10.8.0.0/24')
    assert.notInclude(artifact.content, '0.0.0.0/0')
    assert.equal(artifact.delivery, 'copy')
    assert.isFalse(artifact.supportsQrCode)
  })

  test('MikroTik: deve liberar ICMP e SNMP na chain input da interface WireGuard', ({ assert }) => {
    const artifact = new MikrotikProfileGenerator().generate(context)

    assert.include(artifact.content, 'chain=input in-interface=wg-netmonitor protocol=icmp')
    assert.include(artifact.content, 'dst-port=161 action=accept')
    assert.include(artifact.content, 'name="netmonitor"')
  })

  test('OpenWrt: keepalive de 25s, AllowedIPs restrito e zona de firewall', ({ assert }) => {
    const artifact = new OpenWrtProfileGenerator().generate(context)

    assert.include(artifact.content, `persistent_keepalive='${PERSISTENT_KEEPALIVE_SECONDS}'`)
    assert.include(artifact.content, "allowed_ips='10.8.0.0/24'")
    assert.notInclude(artifact.content, '0.0.0.0/0')
    assert.include(artifact.content, "name='vpn_netmonitor'")
  })

  test('wg.conf: AllowedIPs jamais pode redirecionar toda a internet', ({ assert }) => {
    const artifact = createLinuxGenerator().generate(context)

    assert.include(artifact.content, 'AllowedIPs = 10.8.0.0/24')
    assert.include(artifact.content, `PersistentKeepalive = ${PERSISTENT_KEEPALIVE_SECONDS}`)
    assert.include(artifact.content, 'Endpoint = vpn.exemplo.com.br:51820')
    assert.notInclude(artifact.content, '0.0.0.0/0')
  })

  test('perfil móvel deve ser o único a suportar QR Code', ({ assert }) => {
    assert.isTrue(createMobileGenerator().generate(context).supportsQrCode)
    assert.isFalse(createLinuxGenerator().generate(context).supportsQrCode)
  })

  test('SNMP desabilitado não deve gerar comandos de community', ({ assert }) => {
    const artifact = new MikrotikProfileGenerator().generate({ ...context, snmpEnabled: false })

    assert.notInclude(artifact.content, '/snmp/community/set')
  })

  test('registry deve resolver todos os perfis suportados e rejeitar desconhecidos', ({
    assert,
  }) => {
    const registry = new ProfileRegistry()

    assert.lengthOf(registry.list(), 5)
    assert.equal(registry.resolve('mikrotik').profile, 'mikrotik')
    assert.isTrue(registry.has('openwrt'))
    assert.isFalse(registry.has('vyos'))
    assert.throws(() => registry.resolve('vyos' as never))
  })
})

test.group('VPN - Telemetria dos túneis', () => {
  test('parseWgDump deve interpretar handshake e contadores', ({ assert }) => {
    const dump = [
      'CHAVE-PRIV\tCHAVE-PUB-SERVIDOR\t51820\toff',
      'PUB-A\tPSK-A\t189.10.0.5:4820\t10.8.0.11/32\t1754236800\t1024\t2048\t25',
      'PUB-B\t(none)\t(none)\t10.8.0.12/32\t0\t0\t0\toff',
    ].join('\n')

    const peers = parseWgDump(dump)

    assert.lengthOf(peers, 2)
    assert.equal(peers[0].publicKey, 'PUB-A')
    assert.equal(peers[0].bytesRx, 1024)
    assert.equal(peers[0].bytesTx, 2048)
    assert.equal(peers[0].persistentKeepalive, 25)
    assert.instanceOf(peers[0].latestHandshakeAt, Date)

    assert.isNull(peers[1].latestHandshakeAt)
    assert.isNull(peers[1].presharedKey)
    assert.equal(peers[1].persistentKeepalive, 0)
  })

  test('parseWgDump deve tolerar saída vazia', ({ assert }) => {
    assert.lengthOf(parseWgDump(''), 0)
  })
})

test.group('VPN - Segurança dos endpoints sensíveis', () => {
  test('chave privada do cliente deve ser entregue apenas uma vez', ({ assert }) => {
    const store = new EphemeralSecretStore()
    store.put('vpn-peer:1', 'PRIVADA')

    assert.equal(store.consume('vpn-peer:1'), 'PRIVADA')
    assert.isNull(store.consume('vpn-peer:1'))
  })

  test('segredo expirado não deve ser devolvido', ({ assert }) => {
    const store = new EphemeralSecretStore(-1)
    store.put('vpn-peer:2', 'PRIVADA')

    assert.isNull(store.consume('vpn-peer:2'))
  })

  test('rate limit deve bloquear após o limite da janela', ({ assert }) => {
    const limiter = new SlidingWindowRateLimiter(2, 60_000)

    assert.isTrue(limiter.consume('user:1').allowed)
    assert.isTrue(limiter.consume('user:1').allowed)

    const blocked = limiter.consume('user:1')
    assert.isFalse(blocked.allowed)
    assert.isAbove(blocked.retryAfterSeconds, 0)

    assert.isTrue(limiter.consume('user:2').allowed)
  })
})

test.group('VPN - Pré-voo de conectividade', () => {
  test('deve identificar a faixa de CGNAT (RFC 6598)', ({ assert }) => {
    assert.isTrue(isCgnatAddress('100.64.0.1'))
    assert.isTrue(isCgnatAddress('100.127.255.254'))
    assert.isFalse(isCgnatAddress('100.128.0.1'))
    assert.isFalse(isCgnatAddress('200.150.10.1'))
  })

  test('deve identificar endereços privados', ({ assert }) => {
    assert.isTrue(isPrivateAddress('192.168.0.10'))
    assert.isTrue(isPrivateAddress('10.0.0.1'))
    assert.isTrue(isPrivateAddress('172.20.5.1'))
    assert.isFalse(isPrivateAddress('172.32.5.1'))
    assert.isFalse(isPrivateAddress('200.150.10.1'))
  })
})
