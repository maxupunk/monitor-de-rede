/*
|--------------------------------------------------------------------------
| Routes file
|--------------------------------------------------------------------------
*/

import router from '@adonisjs/core/services/router'

const AuthController = () => import('#controllers/auth_controller')
const SitesController = () => import('#controllers/sites_controller')
const NetworksController = () => import('#controllers/networks_controller')
const DevicesController = () => import('#controllers/devices_controller')
const MonitorsController = () => import('#controllers/monitors_controller')
const DiscoveryController = () => import('#controllers/discovery_controller')
const TopologyController = () => import('#controllers/topology_controller')
const SnmpController = () => import('#controllers/snmp_controller')
const ProbesController = () => import('#controllers/probes_controller')
const AlertsController = () => import('#controllers/alerts_controller')
const EventsController = () => import('#controllers/events_controller')
const VpnServersController = () => import('#controllers/vpn_servers_controller')
const VpnPeersController = () => import('#controllers/vpn_peers_controller')
const ZabbixTemplatesController = () => import('#controllers/zabbix_templates_controller')
const PortScanController = () => import('#controllers/port_scan_controller')
const DnsController = () => import('#controllers/dns_controller')
const DnsServersController = () => import('#controllers/dns_servers_controller')

router.get('/', () => {
  return { status: 'online', service: 'Network Monitor API', version: '1.0.0' }
})

router
  .group(() => {
    // Auth
    router.post('auth/login', [AuthController, 'login'])
    router.post('auth/logout', [AuthController, 'logout'])
    router.get('auth/me', [AuthController, 'me'])

    // Sites
    router.resource('sites', SitesController).apiOnly()

    // Networks
    router.post('networks/:id/scan', [NetworksController, 'scan'])
    router.resource('networks', NetworksController).apiOnly()

    // SNMP (teste avulso, sem exigir dispositivo cadastrado)
    router.post('snmp/test', [SnmpController, 'test'])

    // Port Scanner (ferramenta reutilizável de varredura de portas TCP/UDP)
    router.post('port-scan', [PortScanController, 'scan'])

    // DNS (latência de resolução: medição avulsa, comparação e ranking histórico)
    router.post('dns/benchmark', [DnsController, 'benchmark'])
    router.post('dns/lookup', [DnsController, 'lookup'])
    router.get('dns/performance', [DnsController, 'performance'])

    // Cadastro de servidores DNS usados nos monitores e na comparação
    router.get('dns/servers', [DnsServersController, 'index'])
    router.post('dns/servers', [DnsServersController, 'store'])
    router.put('dns/servers/:id', [DnsServersController, 'update'])
    router.delete('dns/servers/:id', [DnsServersController, 'destroy'])

    // Devices
    router.post('devices/:id/snmp/poll', [SnmpController, 'poll'])
    router.post('devices/:id/snmp/scan', [SnmpController, 'scan'])
    router.post('devices/:id/snmp/apply-monitors', [SnmpController, 'applyMonitors'])
    router.get('devices/:id/interfaces', [SnmpController, 'interfaces'])
    router.get('devices/:id/monitors', [DevicesController, 'monitors'])
    router.get('devices/:id/metrics', [DevicesController, 'metrics'])
    router.get('devices/:id/events', [DevicesController, 'events'])
    router.resource('devices', DevicesController).apiOnly()

    // Monitors
    router.get('monitors/:id/alerts', [MonitorsController, 'alerts'])
    router.get('monitors/:id/results', [MonitorsController, 'results'])
    router.post('monitors/:id/run', [MonitorsController, 'run'])
    router.post('monitors/:id/enable', [MonitorsController, 'enable'])
    router.post('monitors/:id/disable', [MonitorsController, 'disable'])
    router.resource('monitors', MonitorsController).apiOnly()

    // Discovery
    router.get('discovery/runs', [DiscoveryController, 'runs'])
    router.get('discovery/runs/:id', [DiscoveryController, 'runDetails'])
    router.get('discovery/results', [DiscoveryController, 'results'])
    router.get('discovery/results/latest', [DiscoveryController, 'latestResults'])
    router.delete('discovery/cleanup', [DiscoveryController, 'cleanup'])
    router.post('discovery/results/:id/accept', [DiscoveryController, 'accept'])
    router.post('discovery/results/:id/mark-accepted', [DiscoveryController, 'markAccepted'])
    router.post('discovery/results/:id/ignore', [DiscoveryController, 'ignore'])
    router.post('discovery/results/:id/merge', [DiscoveryController, 'merge'])

    // Topology
    router.get('topology', [TopologyController, 'index'])
    router.post('topology/links', [TopologyController, 'storeLink'])
    router.post('topology/recalculate', [TopologyController, 'recalculate'])
    router.delete('topology/links/:id', [TopologyController, 'destroyLink'])

    // Probes
    router.post('probes/heartbeat', [ProbesController, 'heartbeat'])
    router.get('probes/tasks', [ProbesController, 'getTasks'])
    router.post('probes/results', [ProbesController, 'postResults'])
    router.post('probes/:id/revoke', [ProbesController, 'revoke'])
    router.post('probes/:id/test', [ProbesController, 'test'])
    router.resource('probes', ProbesController).apiOnly()

    // Alerts
    router.get('alert-rules/catalog', [AlertsController, 'catalogIndex'])
    router.post('alert-rules/catalog', [AlertsController, 'catalogApply'])
    router.get('alert-rules', [AlertsController, 'rulesIndex'])
    router.post('alert-rules', [AlertsController, 'rulesStore'])
    router.put('alert-rules/:id', [AlertsController, 'rulesUpdate'])
    router.delete('alert-rules/:id', [AlertsController, 'rulesDestroy'])

    router.get('alerts', [AlertsController, 'index'])
    router.post('alerts/:id/acknowledge', [AlertsController, 'acknowledge'])
    router.post('alerts/:id/silence', [AlertsController, 'silence'])

    // VPN WireGuard
    router.get('vpn/server', [VpnServersController, 'show'])
    router.put('vpn/server', [VpnServersController, 'update'])
    router.post('vpn/server/preflight', [VpnServersController, 'preflight'])
    router.post('vpn/server/detect-endpoint', [VpnServersController, 'detectEndpoint'])

    router.get('vpn/peers', [VpnPeersController, 'index'])
    router.get('vpn/peers/next-ip', [VpnPeersController, 'nextIp'])
    router.post('vpn/peers', [VpnPeersController, 'store'])
    router.patch('vpn/peers/:id', [VpnPeersController, 'update'])
    router.get('vpn/peers/:id/config', [VpnPeersController, 'config'])
    router.get('vpn/peers/:id/qrcode', [VpnPeersController, 'qrcode'])
    router.post('vpn/peers/:id/rotate', [VpnPeersController, 'rotate'])
    router.post('vpn/peers/:id/firewall-hints', [VpnPeersController, 'firewallHints'])
    router.delete('vpn/peers/:id', [VpnPeersController, 'destroy'])

    // Templates Zabbix (import/gestão)
    router.get('zabbix-templates', [ZabbixTemplatesController, 'index'])
    router.post('zabbix-templates', [ZabbixTemplatesController, 'store'])
    router.get('zabbix-templates/:id', [ZabbixTemplatesController, 'show'])
    router.delete('zabbix-templates/:id', [ZabbixTemplatesController, 'destroy'])

    // Realtime Events (SSE) & Historical Events
    router.get('events', [EventsController, 'index'])
    router.get('events/stream', [EventsController, 'stream'])
  })
  .prefix('/api')
