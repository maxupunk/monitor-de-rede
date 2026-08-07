import { test } from '@japa/runner'
import { DateTime } from 'luxon'
import VpnPeer, { type VpnPeerConnectionStatus } from '#models/vpn_peer'
import {
  buildVpnPeerDataset,
  describeVpnPeerState,
  hasVpnTransition,
  isVpnRecovery,
} from '#modules/alerts/datasets/vpn_peer_dataset'
import { VPN_STATUS_TRANSITION } from '#modules/alerts/alert_fields'
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
import {
  createLinuxGenerator,
  createMobileGenerator,
  createWindowsGenerator,
} from '#modules/vpn/profiles/wg_conf'
import { ProfileRegistry } from '#modules/vpn/profiles/profile_registry'
import {
  PERSISTENT_KEEPALIVE_SECONDS,
  PRIVATE_KEY_UNAVAILABLE,
  WG_TUNNEL_NAME,
  type ArtifactVariant,
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

/** Falha com mensagem legível quando a variante esperada não é gerada. */
function findVariant(variants: ArtifactVariant[], id: string): ArtifactVariant {
  const variant = variants.find((item) => item.id === id)
  if (!variant) {
    throw new Error(`Variante "${id}" não gerada — encontradas: ${variants.map((v) => v.id)}`)
  }
  return variant
}

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

  test('MikroTik: script deve ser ASCII puro — o console do RouterOS engole acentos', ({
    assert,
  }) => {
    const artifact = new MikrotikProfileGenerator().generate({
      ...context,
      peerName: 'Roteador Assistência Técnica',
    })

    assert.match(artifact.content, /^[\x20-\x7e\n]*$/)
    assert.include(artifact.content, 'NetMonitor - WireGuard - Roteador Assistencia Tecnica')
  })

  test('MikroTik: script deve poder ser colado de novo sem colidir com a instalação anterior', ({
    assert,
  }) => {
    const content = new MikrotikProfileGenerator().generate(context).content
    const position = (needle: string) => content.indexOf(needle)

    // A limpeza usa comment= porque `find interface=<nome>` falha quando a
    // interface não existe — foi o erro em cascata reportado no RB2011.
    assert.include(content, '/interface/wireguard/peers/remove [find comment="NetMonitor"]')
    assert.include(content, '/interface/wireguard/remove [find name="wg-netmonitor"]')
    // Endereços criados por versões antigas do script não têm comentário.
    assert.include(content, '/ip/address/remove [find address="10.8.0.11/24"]')
    assert.isBelow(
      position('/interface/wireguard/remove'),
      position('/interface/wireguard/add'),
      'a remoção precisa vir antes da criação'
    )

    // place-before=0 falha quando a chain está vazia; move resolve nos dois casos.
    assert.notInclude(content, 'place-before')
    assert.include(
      content,
      '/ip/firewall/filter/move [find comment="NetMonitor ICMP"] destination=0'
    )
  })

  test('cabeçalho deve avisar quando a chave privada já foi consumida', ({ assert }) => {
    const artifact = new MikrotikProfileGenerator().generate({
      ...context,
      clientPrivateKey: PRIVATE_KEY_UNAVAILABLE,
    })

    assert.include(artifact.content, 'ATENCAO: a chave privada deste dispositivo ja foi entregue')
    assert.include(artifact.content, 'Rotacionar chaves')
    // Sem o aviso, o RouterOS responderia apenas "failure: invalid private key".
    assert.notInclude(new MikrotikProfileGenerator().generate(context).content, 'ATENCAO')
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

  test('resumo do túnel deve trazer os dados de conferência sem vazar a chave privada', ({
    assert,
  }) => {
    const summary = createLinuxGenerator().generate(context).summary
    const value = (label: string) => summary.find((item) => item.label === label)?.value

    assert.equal(value('IP fixo na VPN'), '10.8.0.11/24')
    assert.equal(value('Endpoint do servidor'), 'vpn.exemplo.com.br:51820')
    assert.equal(value('Chave pública do servidor'), 'CHAVE-PUBLICA-SERVIDOR')
    assert.equal(value('SNMP'), 'community "netmonitor"')
    assert.notInclude(JSON.stringify(summary), context.clientPrivateKey)
  })

  test('Windows: variante winget deve instalar o cliente e abrir o WireGuard', ({ assert }) => {
    const artifact = createWindowsGenerator().generate(context)
    const winget = findVariant(artifact.variants, 'winget')

    assert.include(winget.content, 'winget install --exact --id WireGuard.WireGuard')
    assert.include(winget.content, 'Start-Process -FilePath $WgExe')
    // O perfil embutido é exatamente o .conf entregue na aba principal.
    assert.include(winget.content, artifact.content.trim())
    assert.equal(winget.language, 'powershell')
  })

  test('Windows: o corpo precisa ser um único scriptblock para que um erro aborte tudo', ({
    assert,
  }) => {
    const content = findVariant(
      createWindowsGenerator().generate(context).variants,
      'winget'
    ).content

    // Colado no console, cada linha de nível superior roda mesmo depois de uma
    // falha anterior — `$ErrorActionPreference` não segura a linha seguinte.
    // Dentro de `& { ... }` é um comando só, e o primeiro throw encerra.
    assert.include(content, '& {')
    assert.include(content, "  $ErrorActionPreference = 'Stop'")

    const guards = content.indexOf('$isAdmin')
    assert.isBelow(guards, content.indexOf('winget install'), 'as checagens vêm antes de instalar')
    assert.isBelow(guards, content.indexOf('Set-Content'), 'e antes de gravar qualquer arquivo')

    assert.include(content, '[Security.Principal.WindowsBuiltInRole]::Administrator')
    assert.notInclude(content, 'Win + X')
  })

  test('Windows: o perfil vai para o cofre do WireGuard, não para um caminho próprio', ({
    assert,
  }) => {
    const content = findVariant(
      createWindowsGenerator().generate(context).variants,
      'winget'
    ).content

    // O gerenciador só lista o que está em Data\Configurations — foi por isso
    // que a janela do WireGuard aparecia vazia com o túnel no ar.
    assert.include(content, "$Store = Join-Path $WgHome 'Data\\Configurations'")
    assert.include(content, '$File = Join-Path $Store "$Tunnel.conf"')
    assert.notInclude(content, 'GetTempPath')

    // Um .conf.dpapi de importação anterior venceria o .conf recém-gravado.
    assert.isBelow(
      content.indexOf('Remove-Item -Path "$File.dpapi"'),
      content.indexOf('$Conf | Set-Content'),
      'limpa o perfil cifrado antigo antes de gravar o novo'
    )
  })

  test('Windows: não pode criar serviço próprio — ele disputaria o adaptador com a UI', ({
    assert,
  }) => {
    const content = findVariant(
      createWindowsGenerator().generate(context).variants,
      'winget'
    ).content

    // Ao ativar pela janela, o WireGuard cria o serviço e cifra o perfil como
    // .conf.dpapi. Um /installtunnelservice nosso apontaria para o .conf que
    // some nessa conversão, e os dois túneis brigariam pelo mesmo adaptador.
    assert.notMatch(content, /(?<!un)installtunnelservice \$File/)
    assert.include(content, '& $WgExe /uninstalltunnelservice $Tunnel')
  })

  test('Windows: a ACL do perfil não pode trancar a própria reinstalação', ({ assert }) => {
    const content = findVariant(
      createWindowsGenerator().generate(context).variants,
      'winget'
    ).content

    // SYSTEM roda o gerenciador e precisa ler o perfil; (F) em vez de (R)
    // porque a ACL de leitura não inclui DELETE e travava a execução seguinte.
    assert.include(
      content,
      'icacls $File /inheritance:r /grant:r "*S-1-5-18:(F)" "*S-1-5-32-544:(F)"'
    )

    // E destranca o que as versões com (R) deixaram para trás em ProgramData.
    assert.include(content, 'takeown /f $item /a')
    assert.include(content, 'icacls $item /reset')
    assert.include(content, 'Remove-Item -Path $Legacy -Recurse -Force')
  })

  test('Windows: script deve ser ASCII puro — o console usa codepage legada', ({ assert }) => {
    const content = findVariant(
      createWindowsGenerator().generate({ ...context, peerName: 'Notebook Diretoria Técnica' })
        .variants,
      'winget'
    ).content

    assert.match(content, /^[\x20-\x7e\n]*$/)
  })

  test('scripts devem se recusar a rodar quando a chave privada já foi consumida', ({ assert }) => {
    const semChave = { ...context, clientPrivateKey: PRIVATE_KEY_UNAVAILABLE }

    const winget = findVariant(createWindowsGenerator().generate(semChave).variants, 'winget')
    assert.include(winget.content, "if ($Conf -match 'CHAVE-PRIVADA-INDISPONIVEL')")
    assert.include(winget.content, 'ATENCAO: a chave privada deste dispositivo ja foi entregue')

    const bash = findVariant(createLinuxGenerator().generate(semChave).variants, 'bash')
    assert.include(bash.content, 'if grep -q CHAVE-PRIVADA-INDISPONIVEL "$CONF"; then')
    assert.include(bash.content, 'ATENCAO: a chave privada deste dispositivo ja foi entregue')
  })

  test('Linux: variante bash deve cobrir apt, dnf e demais gerenciadores', ({ assert }) => {
    const artifact = createLinuxGenerator().generate(context)
    const bash = findVariant(artifact.variants, 'bash')

    assert.include(bash.content, 'if command -v apt-get')
    assert.include(bash.content, 'apt-get install -y wireguard-tools')
    assert.include(bash.content, 'elif command -v dnf')
    assert.include(bash.content, 'dnf install -y wireguard-tools')
    assert.include(bash.content, 'elif command -v pacman')
    assert.include(bash.content, `systemctl enable --now "wg-quick@\${IFACE}"`)
    assert.include(bash.content, `IFACE='${WG_TUNNEL_NAME}'`)
    assert.include(bash.content, artifact.content.trim())
    assert.notInclude(bash.content, '0.0.0.0/0')
  })

  test('Linux: chave privada gravada apenas com permissão de root', ({ assert }) => {
    const bash = findVariant(createLinuxGenerator().generate(context).variants, 'bash')

    assert.include(bash.content, 'install -d -m 700 /etc/wireguard')
    assert.include(bash.content, 'chmod 600 "$CONF"')
  })

  test('OpenWrt: script principal detecta o gerenciador e cada variante fixa um deles', ({
    assert,
  }) => {
    const artifact = new OpenWrtProfileGenerator().generate(context)

    assert.include(artifact.content, 'if command -v apk')
    assert.include(artifact.content, 'apk update && apk add wireguard-tools')
    assert.include(artifact.content, 'opkg update && opkg install wireguard-tools')

    const opkg = findVariant(artifact.variants, 'opkg')
    assert.include(opkg.content, 'opkg update && opkg install wireguard-tools')
    assert.notInclude(opkg.content, 'apk add')

    const apk = findVariant(artifact.variants, 'apk')
    assert.include(apk.content, 'apk update && apk add wireguard-tools')
    assert.include(apk.content, 'apk update && apk add snmpd')
    assert.notInclude(apk.content, 'opkg install')
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

test.group('VPN - Estado do túnel', () => {
  function peerWith(fields: Partial<VpnPeer>): VpnPeer {
    const peer = new VpnPeer()
    peer.persistentKeepalive = 25
    peer.lastHandshakeAt = null
    peer.lastSeenAt = null
    Object.assign(peer, fields)
    return peer
  }

  test('peer sem nenhum sinal deve ficar aguardando', ({ assert }) => {
    assert.equal(peerWith({}).connectionStatus, 'awaiting')
  })

  test('keepalive recente deve manter conectado mesmo com handshake antigo', ({ assert }) => {
    // Cenário real: túnel ocioso. O WireGuard só renegocia chaves quando tem o
    // que enviar, então o handshake envelhece além do REJECT_AFTER_TIME sem que
    // um único pacote tenha se perdido — quem prova a vida é o keepalive.
    const peer = peerWith({
      lastHandshakeAt: DateTime.now().minus({ minutes: 7 }),
      lastSeenAt: DateTime.now().minus({ seconds: 20 }),
    })

    assert.equal(peer.connectionStatus, 'connected')
  })

  test('keepalives perdidos além da janela devem marcar instável', ({ assert }) => {
    const peer = peerWith({
      lastHandshakeAt: DateTime.now().minus({ minutes: 7 }),
      lastSeenAt: DateTime.now().minus({ seconds: 200 }),
    })

    assert.equal(peer.connectionStatus, 'unstable')
  })

  test('silêncio prolongado deve marcar desconectado', ({ assert }) => {
    const peer = peerWith({
      lastHandshakeAt: DateTime.now().minus({ minutes: 30 }),
      lastSeenAt: DateTime.now().minus({ minutes: 30 }),
    })

    assert.equal(peer.connectionStatus, 'disconnected')
  })

  test('sem keepalive contabilizado o handshake sustenta o status', ({ assert }) => {
    // Peer sem PersistentKeepalive: não há sinal periódico, então a janela
    // precisa cobrir o REJECT_AFTER_TIME do protocolo mais a folga da coleta.
    const recente = peerWith({
      persistentKeepalive: 0,
      lastHandshakeAt: DateTime.now().minus({ seconds: 200 }),
    })
    const antigo = peerWith({
      persistentKeepalive: 0,
      lastHandshakeAt: DateTime.now().minus({ seconds: 400 }),
    })

    assert.equal(recente.connectionStatus, 'connected')
    assert.equal(antigo.connectionStatus, 'unstable')
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

test.group('VPN - Transições de estado do túnel (alertas)', () => {
  const facts = (
    status: VpnPeerConnectionStatus,
    previousStatus: VpnPeerConnectionStatus | null,
    secondsSinceActivity: number | null = 600
  ) => ({ peerName: 'Roteador Filial', status, previousStatus, secondsSinceActivity })

  test('queda do túnel deve produzir a transição alertável', ({ assert }) => {
    const dataset = buildVpnPeerDataset(facts('disconnected', 'connected'))

    assert.equal(dataset.vpnStatusTransition, VPN_STATUS_TRANSITION.disconnected)
    assert.equal(dataset.vpnPeerStatus, 'disconnected')
    assert.equal(dataset.vpnPreviousStatus, 'connected')
    assert.isTrue(hasVpnTransition(dataset))
    assert.isFalse(isVpnRecovery(dataset))
    assert.include(describeVpnPeerState(dataset), 'caiu')
  })

  test('instabilidade deve ser distinguida da queda', ({ assert }) => {
    const dataset = buildVpnPeerDataset(facts('unstable', 'connected'))

    assert.equal(dataset.vpnStatusTransition, VPN_STATUS_TRANSITION.destabilized)
    assert.include(describeVpnPeerState(dataset), 'instável')
  })

  test('degradação em cadeia (instável ➔ caído) ainda conta como queda', ({ assert }) => {
    const dataset = buildVpnPeerDataset(facts('disconnected', 'unstable'))

    assert.equal(dataset.vpnStatusTransition, VPN_STATUS_TRANSITION.disconnected)
  })

  test('retorno do túnel deve permitir a normalização do alerta', ({ assert }) => {
    const dataset = buildVpnPeerDataset(facts('connected', 'disconnected', 5))

    assert.equal(dataset.vpnStatusTransition, VPN_STATUS_TRANSITION.reconnected)
    assert.isTrue(isVpnRecovery(dataset))
  })

  test('primeira conexão de um peer novo não deve alertar', ({ assert }) => {
    const dataset = buildVpnPeerDataset(facts('awaiting', null, null))

    assert.isFalse(hasVpnTransition(dataset))
    assert.isUndefined(dataset.vpnSecondsSinceActivity)
  })

  test('estado inalterado não deve gerar transição a cada ciclo', ({ assert }) => {
    const dataset = buildVpnPeerDataset(facts('disconnected', 'disconnected'))

    assert.isFalse(hasVpnTransition(dataset))
  })

  test('sem estado anterior conhecido, o ciclo apenas estabelece a linha de base', ({ assert }) => {
    const dataset = buildVpnPeerDataset(facts('disconnected', null))

    assert.isFalse(hasVpnTransition(dataset))
  })
})
