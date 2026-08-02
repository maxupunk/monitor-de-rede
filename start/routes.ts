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

    // Devices
    router.post('devices/:id/snmp/poll', [SnmpController, 'poll'])
    router.get('devices/:id/interfaces', [SnmpController, 'interfaces'])
    router.get('devices/:id/monitors', [DevicesController, 'monitors'])
    router.get('devices/:id/metrics', [DevicesController, 'metrics'])
    router.get('devices/:id/events', [DevicesController, 'events'])
    router.resource('devices', DevicesController).apiOnly()

    // Monitors
    router.post('monitors/:id/run', [MonitorsController, 'run'])
    router.post('monitors/:id/enable', [MonitorsController, 'enable'])
    router.post('monitors/:id/disable', [MonitorsController, 'disable'])
    router.resource('monitors', MonitorsController).apiOnly()

    // Discovery
    router.get('discovery/runs', [DiscoveryController, 'runs'])
    router.get('discovery/runs/:id', [DiscoveryController, 'runDetails'])
    router.get('discovery/results', [DiscoveryController, 'results'])
    router.post('discovery/results/:id/accept', [DiscoveryController, 'accept'])
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
    router.get('alert-rules', [AlertsController, 'rulesIndex'])
    router.post('alert-rules', [AlertsController, 'rulesStore'])
    router.put('alert-rules/:id', [AlertsController, 'rulesUpdate'])
    router.delete('alert-rules/:id', [AlertsController, 'rulesDestroy'])

    router.get('alerts', [AlertsController, 'index'])
    router.post('alerts/:id/acknowledge', [AlertsController, 'acknowledge'])
    router.post('alerts/:id/silence', [AlertsController, 'silence'])

    // Realtime Events (SSE)
    router.get('events/stream', [EventsController, 'stream'])
  })
  .prefix('/api')
