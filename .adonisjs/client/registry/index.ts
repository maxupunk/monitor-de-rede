/* eslint-disable prettier/prettier */
import type { AdonisEndpoint } from '@tuyau/core/types'
import type { Registry } from './schema.d.ts'
import type { ApiDefinition } from './tree.d.ts'

const placeholder: any = {}

const routes = {
  'auth.login': {
    methods: ["POST"],
    pattern: '/api/auth/login',
    tokens: [{"old":"/api/auth/login","type":0,"val":"api","end":""},{"old":"/api/auth/login","type":0,"val":"auth","end":""},{"old":"/api/auth/login","type":0,"val":"login","end":""}],
    types: placeholder as Registry['auth.login']['types'],
  },
  'auth.logout': {
    methods: ["POST"],
    pattern: '/api/auth/logout',
    tokens: [{"old":"/api/auth/logout","type":0,"val":"api","end":""},{"old":"/api/auth/logout","type":0,"val":"auth","end":""},{"old":"/api/auth/logout","type":0,"val":"logout","end":""}],
    types: placeholder as Registry['auth.logout']['types'],
  },
  'auth.me': {
    methods: ["GET","HEAD"],
    pattern: '/api/auth/me',
    tokens: [{"old":"/api/auth/me","type":0,"val":"api","end":""},{"old":"/api/auth/me","type":0,"val":"auth","end":""},{"old":"/api/auth/me","type":0,"val":"me","end":""}],
    types: placeholder as Registry['auth.me']['types'],
  },
  'sites.index': {
    methods: ["GET","HEAD"],
    pattern: '/api/sites',
    tokens: [{"old":"/api/sites","type":0,"val":"api","end":""},{"old":"/api/sites","type":0,"val":"sites","end":""}],
    types: placeholder as Registry['sites.index']['types'],
  },
  'sites.store': {
    methods: ["POST"],
    pattern: '/api/sites',
    tokens: [{"old":"/api/sites","type":0,"val":"api","end":""},{"old":"/api/sites","type":0,"val":"sites","end":""}],
    types: placeholder as Registry['sites.store']['types'],
  },
  'sites.show': {
    methods: ["GET","HEAD"],
    pattern: '/api/sites/:id',
    tokens: [{"old":"/api/sites/:id","type":0,"val":"api","end":""},{"old":"/api/sites/:id","type":0,"val":"sites","end":""},{"old":"/api/sites/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['sites.show']['types'],
  },
  'sites.update': {
    methods: ["PUT","PATCH"],
    pattern: '/api/sites/:id',
    tokens: [{"old":"/api/sites/:id","type":0,"val":"api","end":""},{"old":"/api/sites/:id","type":0,"val":"sites","end":""},{"old":"/api/sites/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['sites.update']['types'],
  },
  'sites.destroy': {
    methods: ["DELETE"],
    pattern: '/api/sites/:id',
    tokens: [{"old":"/api/sites/:id","type":0,"val":"api","end":""},{"old":"/api/sites/:id","type":0,"val":"sites","end":""},{"old":"/api/sites/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['sites.destroy']['types'],
  },
  'networks.scan': {
    methods: ["POST"],
    pattern: '/api/networks/:id/scan',
    tokens: [{"old":"/api/networks/:id/scan","type":0,"val":"api","end":""},{"old":"/api/networks/:id/scan","type":0,"val":"networks","end":""},{"old":"/api/networks/:id/scan","type":1,"val":"id","end":""},{"old":"/api/networks/:id/scan","type":0,"val":"scan","end":""}],
    types: placeholder as Registry['networks.scan']['types'],
  },
  'networks.index': {
    methods: ["GET","HEAD"],
    pattern: '/api/networks',
    tokens: [{"old":"/api/networks","type":0,"val":"api","end":""},{"old":"/api/networks","type":0,"val":"networks","end":""}],
    types: placeholder as Registry['networks.index']['types'],
  },
  'networks.store': {
    methods: ["POST"],
    pattern: '/api/networks',
    tokens: [{"old":"/api/networks","type":0,"val":"api","end":""},{"old":"/api/networks","type":0,"val":"networks","end":""}],
    types: placeholder as Registry['networks.store']['types'],
  },
  'networks.show': {
    methods: ["GET","HEAD"],
    pattern: '/api/networks/:id',
    tokens: [{"old":"/api/networks/:id","type":0,"val":"api","end":""},{"old":"/api/networks/:id","type":0,"val":"networks","end":""},{"old":"/api/networks/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['networks.show']['types'],
  },
  'networks.update': {
    methods: ["PUT","PATCH"],
    pattern: '/api/networks/:id',
    tokens: [{"old":"/api/networks/:id","type":0,"val":"api","end":""},{"old":"/api/networks/:id","type":0,"val":"networks","end":""},{"old":"/api/networks/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['networks.update']['types'],
  },
  'networks.destroy': {
    methods: ["DELETE"],
    pattern: '/api/networks/:id',
    tokens: [{"old":"/api/networks/:id","type":0,"val":"api","end":""},{"old":"/api/networks/:id","type":0,"val":"networks","end":""},{"old":"/api/networks/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['networks.destroy']['types'],
  },
  'snmp.test': {
    methods: ["POST"],
    pattern: '/api/snmp/test',
    tokens: [{"old":"/api/snmp/test","type":0,"val":"api","end":""},{"old":"/api/snmp/test","type":0,"val":"snmp","end":""},{"old":"/api/snmp/test","type":0,"val":"test","end":""}],
    types: placeholder as Registry['snmp.test']['types'],
  },
  'port_scan.scan': {
    methods: ["POST"],
    pattern: '/api/port-scan',
    tokens: [{"old":"/api/port-scan","type":0,"val":"api","end":""},{"old":"/api/port-scan","type":0,"val":"port-scan","end":""}],
    types: placeholder as Registry['port_scan.scan']['types'],
  },
  'snmp.poll': {
    methods: ["POST"],
    pattern: '/api/devices/:id/snmp/poll',
    tokens: [{"old":"/api/devices/:id/snmp/poll","type":0,"val":"api","end":""},{"old":"/api/devices/:id/snmp/poll","type":0,"val":"devices","end":""},{"old":"/api/devices/:id/snmp/poll","type":1,"val":"id","end":""},{"old":"/api/devices/:id/snmp/poll","type":0,"val":"snmp","end":""},{"old":"/api/devices/:id/snmp/poll","type":0,"val":"poll","end":""}],
    types: placeholder as Registry['snmp.poll']['types'],
  },
  'snmp.scan': {
    methods: ["POST"],
    pattern: '/api/devices/:id/snmp/scan',
    tokens: [{"old":"/api/devices/:id/snmp/scan","type":0,"val":"api","end":""},{"old":"/api/devices/:id/snmp/scan","type":0,"val":"devices","end":""},{"old":"/api/devices/:id/snmp/scan","type":1,"val":"id","end":""},{"old":"/api/devices/:id/snmp/scan","type":0,"val":"snmp","end":""},{"old":"/api/devices/:id/snmp/scan","type":0,"val":"scan","end":""}],
    types: placeholder as Registry['snmp.scan']['types'],
  },
  'snmp.apply_monitors': {
    methods: ["POST"],
    pattern: '/api/devices/:id/snmp/apply-monitors',
    tokens: [{"old":"/api/devices/:id/snmp/apply-monitors","type":0,"val":"api","end":""},{"old":"/api/devices/:id/snmp/apply-monitors","type":0,"val":"devices","end":""},{"old":"/api/devices/:id/snmp/apply-monitors","type":1,"val":"id","end":""},{"old":"/api/devices/:id/snmp/apply-monitors","type":0,"val":"snmp","end":""},{"old":"/api/devices/:id/snmp/apply-monitors","type":0,"val":"apply-monitors","end":""}],
    types: placeholder as Registry['snmp.apply_monitors']['types'],
  },
  'snmp.interfaces': {
    methods: ["GET","HEAD"],
    pattern: '/api/devices/:id/interfaces',
    tokens: [{"old":"/api/devices/:id/interfaces","type":0,"val":"api","end":""},{"old":"/api/devices/:id/interfaces","type":0,"val":"devices","end":""},{"old":"/api/devices/:id/interfaces","type":1,"val":"id","end":""},{"old":"/api/devices/:id/interfaces","type":0,"val":"interfaces","end":""}],
    types: placeholder as Registry['snmp.interfaces']['types'],
  },
  'devices.monitors': {
    methods: ["GET","HEAD"],
    pattern: '/api/devices/:id/monitors',
    tokens: [{"old":"/api/devices/:id/monitors","type":0,"val":"api","end":""},{"old":"/api/devices/:id/monitors","type":0,"val":"devices","end":""},{"old":"/api/devices/:id/monitors","type":1,"val":"id","end":""},{"old":"/api/devices/:id/monitors","type":0,"val":"monitors","end":""}],
    types: placeholder as Registry['devices.monitors']['types'],
  },
  'devices.metrics': {
    methods: ["GET","HEAD"],
    pattern: '/api/devices/:id/metrics',
    tokens: [{"old":"/api/devices/:id/metrics","type":0,"val":"api","end":""},{"old":"/api/devices/:id/metrics","type":0,"val":"devices","end":""},{"old":"/api/devices/:id/metrics","type":1,"val":"id","end":""},{"old":"/api/devices/:id/metrics","type":0,"val":"metrics","end":""}],
    types: placeholder as Registry['devices.metrics']['types'],
  },
  'devices.events': {
    methods: ["GET","HEAD"],
    pattern: '/api/devices/:id/events',
    tokens: [{"old":"/api/devices/:id/events","type":0,"val":"api","end":""},{"old":"/api/devices/:id/events","type":0,"val":"devices","end":""},{"old":"/api/devices/:id/events","type":1,"val":"id","end":""},{"old":"/api/devices/:id/events","type":0,"val":"events","end":""}],
    types: placeholder as Registry['devices.events']['types'],
  },
  'devices.index': {
    methods: ["GET","HEAD"],
    pattern: '/api/devices',
    tokens: [{"old":"/api/devices","type":0,"val":"api","end":""},{"old":"/api/devices","type":0,"val":"devices","end":""}],
    types: placeholder as Registry['devices.index']['types'],
  },
  'devices.store': {
    methods: ["POST"],
    pattern: '/api/devices',
    tokens: [{"old":"/api/devices","type":0,"val":"api","end":""},{"old":"/api/devices","type":0,"val":"devices","end":""}],
    types: placeholder as Registry['devices.store']['types'],
  },
  'devices.show': {
    methods: ["GET","HEAD"],
    pattern: '/api/devices/:id',
    tokens: [{"old":"/api/devices/:id","type":0,"val":"api","end":""},{"old":"/api/devices/:id","type":0,"val":"devices","end":""},{"old":"/api/devices/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['devices.show']['types'],
  },
  'devices.update': {
    methods: ["PUT","PATCH"],
    pattern: '/api/devices/:id',
    tokens: [{"old":"/api/devices/:id","type":0,"val":"api","end":""},{"old":"/api/devices/:id","type":0,"val":"devices","end":""},{"old":"/api/devices/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['devices.update']['types'],
  },
  'devices.destroy': {
    methods: ["DELETE"],
    pattern: '/api/devices/:id',
    tokens: [{"old":"/api/devices/:id","type":0,"val":"api","end":""},{"old":"/api/devices/:id","type":0,"val":"devices","end":""},{"old":"/api/devices/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['devices.destroy']['types'],
  },
  'monitors.run': {
    methods: ["POST"],
    pattern: '/api/monitors/:id/run',
    tokens: [{"old":"/api/monitors/:id/run","type":0,"val":"api","end":""},{"old":"/api/monitors/:id/run","type":0,"val":"monitors","end":""},{"old":"/api/monitors/:id/run","type":1,"val":"id","end":""},{"old":"/api/monitors/:id/run","type":0,"val":"run","end":""}],
    types: placeholder as Registry['monitors.run']['types'],
  },
  'monitors.enable': {
    methods: ["POST"],
    pattern: '/api/monitors/:id/enable',
    tokens: [{"old":"/api/monitors/:id/enable","type":0,"val":"api","end":""},{"old":"/api/monitors/:id/enable","type":0,"val":"monitors","end":""},{"old":"/api/monitors/:id/enable","type":1,"val":"id","end":""},{"old":"/api/monitors/:id/enable","type":0,"val":"enable","end":""}],
    types: placeholder as Registry['monitors.enable']['types'],
  },
  'monitors.disable': {
    methods: ["POST"],
    pattern: '/api/monitors/:id/disable',
    tokens: [{"old":"/api/monitors/:id/disable","type":0,"val":"api","end":""},{"old":"/api/monitors/:id/disable","type":0,"val":"monitors","end":""},{"old":"/api/monitors/:id/disable","type":1,"val":"id","end":""},{"old":"/api/monitors/:id/disable","type":0,"val":"disable","end":""}],
    types: placeholder as Registry['monitors.disable']['types'],
  },
  'monitors.index': {
    methods: ["GET","HEAD"],
    pattern: '/api/monitors',
    tokens: [{"old":"/api/monitors","type":0,"val":"api","end":""},{"old":"/api/monitors","type":0,"val":"monitors","end":""}],
    types: placeholder as Registry['monitors.index']['types'],
  },
  'monitors.store': {
    methods: ["POST"],
    pattern: '/api/monitors',
    tokens: [{"old":"/api/monitors","type":0,"val":"api","end":""},{"old":"/api/monitors","type":0,"val":"monitors","end":""}],
    types: placeholder as Registry['monitors.store']['types'],
  },
  'monitors.show': {
    methods: ["GET","HEAD"],
    pattern: '/api/monitors/:id',
    tokens: [{"old":"/api/monitors/:id","type":0,"val":"api","end":""},{"old":"/api/monitors/:id","type":0,"val":"monitors","end":""},{"old":"/api/monitors/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['monitors.show']['types'],
  },
  'monitors.update': {
    methods: ["PUT","PATCH"],
    pattern: '/api/monitors/:id',
    tokens: [{"old":"/api/monitors/:id","type":0,"val":"api","end":""},{"old":"/api/monitors/:id","type":0,"val":"monitors","end":""},{"old":"/api/monitors/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['monitors.update']['types'],
  },
  'monitors.destroy': {
    methods: ["DELETE"],
    pattern: '/api/monitors/:id',
    tokens: [{"old":"/api/monitors/:id","type":0,"val":"api","end":""},{"old":"/api/monitors/:id","type":0,"val":"monitors","end":""},{"old":"/api/monitors/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['monitors.destroy']['types'],
  },
  'discovery.runs': {
    methods: ["GET","HEAD"],
    pattern: '/api/discovery/runs',
    tokens: [{"old":"/api/discovery/runs","type":0,"val":"api","end":""},{"old":"/api/discovery/runs","type":0,"val":"discovery","end":""},{"old":"/api/discovery/runs","type":0,"val":"runs","end":""}],
    types: placeholder as Registry['discovery.runs']['types'],
  },
  'discovery.run_details': {
    methods: ["GET","HEAD"],
    pattern: '/api/discovery/runs/:id',
    tokens: [{"old":"/api/discovery/runs/:id","type":0,"val":"api","end":""},{"old":"/api/discovery/runs/:id","type":0,"val":"discovery","end":""},{"old":"/api/discovery/runs/:id","type":0,"val":"runs","end":""},{"old":"/api/discovery/runs/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['discovery.run_details']['types'],
  },
  'discovery.results': {
    methods: ["GET","HEAD"],
    pattern: '/api/discovery/results',
    tokens: [{"old":"/api/discovery/results","type":0,"val":"api","end":""},{"old":"/api/discovery/results","type":0,"val":"discovery","end":""},{"old":"/api/discovery/results","type":0,"val":"results","end":""}],
    types: placeholder as Registry['discovery.results']['types'],
  },
  'discovery.accept': {
    methods: ["POST"],
    pattern: '/api/discovery/results/:id/accept',
    tokens: [{"old":"/api/discovery/results/:id/accept","type":0,"val":"api","end":""},{"old":"/api/discovery/results/:id/accept","type":0,"val":"discovery","end":""},{"old":"/api/discovery/results/:id/accept","type":0,"val":"results","end":""},{"old":"/api/discovery/results/:id/accept","type":1,"val":"id","end":""},{"old":"/api/discovery/results/:id/accept","type":0,"val":"accept","end":""}],
    types: placeholder as Registry['discovery.accept']['types'],
  },
  'discovery.ignore': {
    methods: ["POST"],
    pattern: '/api/discovery/results/:id/ignore',
    tokens: [{"old":"/api/discovery/results/:id/ignore","type":0,"val":"api","end":""},{"old":"/api/discovery/results/:id/ignore","type":0,"val":"discovery","end":""},{"old":"/api/discovery/results/:id/ignore","type":0,"val":"results","end":""},{"old":"/api/discovery/results/:id/ignore","type":1,"val":"id","end":""},{"old":"/api/discovery/results/:id/ignore","type":0,"val":"ignore","end":""}],
    types: placeholder as Registry['discovery.ignore']['types'],
  },
  'discovery.merge': {
    methods: ["POST"],
    pattern: '/api/discovery/results/:id/merge',
    tokens: [{"old":"/api/discovery/results/:id/merge","type":0,"val":"api","end":""},{"old":"/api/discovery/results/:id/merge","type":0,"val":"discovery","end":""},{"old":"/api/discovery/results/:id/merge","type":0,"val":"results","end":""},{"old":"/api/discovery/results/:id/merge","type":1,"val":"id","end":""},{"old":"/api/discovery/results/:id/merge","type":0,"val":"merge","end":""}],
    types: placeholder as Registry['discovery.merge']['types'],
  },
  'topology.index': {
    methods: ["GET","HEAD"],
    pattern: '/api/topology',
    tokens: [{"old":"/api/topology","type":0,"val":"api","end":""},{"old":"/api/topology","type":0,"val":"topology","end":""}],
    types: placeholder as Registry['topology.index']['types'],
  },
  'topology.store_link': {
    methods: ["POST"],
    pattern: '/api/topology/links',
    tokens: [{"old":"/api/topology/links","type":0,"val":"api","end":""},{"old":"/api/topology/links","type":0,"val":"topology","end":""},{"old":"/api/topology/links","type":0,"val":"links","end":""}],
    types: placeholder as Registry['topology.store_link']['types'],
  },
  'topology.recalculate': {
    methods: ["POST"],
    pattern: '/api/topology/recalculate',
    tokens: [{"old":"/api/topology/recalculate","type":0,"val":"api","end":""},{"old":"/api/topology/recalculate","type":0,"val":"topology","end":""},{"old":"/api/topology/recalculate","type":0,"val":"recalculate","end":""}],
    types: placeholder as Registry['topology.recalculate']['types'],
  },
  'topology.destroy_link': {
    methods: ["DELETE"],
    pattern: '/api/topology/links/:id',
    tokens: [{"old":"/api/topology/links/:id","type":0,"val":"api","end":""},{"old":"/api/topology/links/:id","type":0,"val":"topology","end":""},{"old":"/api/topology/links/:id","type":0,"val":"links","end":""},{"old":"/api/topology/links/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['topology.destroy_link']['types'],
  },
  'probes.heartbeat': {
    methods: ["POST"],
    pattern: '/api/probes/heartbeat',
    tokens: [{"old":"/api/probes/heartbeat","type":0,"val":"api","end":""},{"old":"/api/probes/heartbeat","type":0,"val":"probes","end":""},{"old":"/api/probes/heartbeat","type":0,"val":"heartbeat","end":""}],
    types: placeholder as Registry['probes.heartbeat']['types'],
  },
  'probes.get_tasks': {
    methods: ["GET","HEAD"],
    pattern: '/api/probes/tasks',
    tokens: [{"old":"/api/probes/tasks","type":0,"val":"api","end":""},{"old":"/api/probes/tasks","type":0,"val":"probes","end":""},{"old":"/api/probes/tasks","type":0,"val":"tasks","end":""}],
    types: placeholder as Registry['probes.get_tasks']['types'],
  },
  'probes.post_results': {
    methods: ["POST"],
    pattern: '/api/probes/results',
    tokens: [{"old":"/api/probes/results","type":0,"val":"api","end":""},{"old":"/api/probes/results","type":0,"val":"probes","end":""},{"old":"/api/probes/results","type":0,"val":"results","end":""}],
    types: placeholder as Registry['probes.post_results']['types'],
  },
  'probes.revoke': {
    methods: ["POST"],
    pattern: '/api/probes/:id/revoke',
    tokens: [{"old":"/api/probes/:id/revoke","type":0,"val":"api","end":""},{"old":"/api/probes/:id/revoke","type":0,"val":"probes","end":""},{"old":"/api/probes/:id/revoke","type":1,"val":"id","end":""},{"old":"/api/probes/:id/revoke","type":0,"val":"revoke","end":""}],
    types: placeholder as Registry['probes.revoke']['types'],
  },
  'probes.test': {
    methods: ["POST"],
    pattern: '/api/probes/:id/test',
    tokens: [{"old":"/api/probes/:id/test","type":0,"val":"api","end":""},{"old":"/api/probes/:id/test","type":0,"val":"probes","end":""},{"old":"/api/probes/:id/test","type":1,"val":"id","end":""},{"old":"/api/probes/:id/test","type":0,"val":"test","end":""}],
    types: placeholder as Registry['probes.test']['types'],
  },
  'probes.index': {
    methods: ["GET","HEAD"],
    pattern: '/api/probes',
    tokens: [{"old":"/api/probes","type":0,"val":"api","end":""},{"old":"/api/probes","type":0,"val":"probes","end":""}],
    types: placeholder as Registry['probes.index']['types'],
  },
  'probes.store': {
    methods: ["POST"],
    pattern: '/api/probes',
    tokens: [{"old":"/api/probes","type":0,"val":"api","end":""},{"old":"/api/probes","type":0,"val":"probes","end":""}],
    types: placeholder as Registry['probes.store']['types'],
  },
  'probes.show': {
    methods: ["GET","HEAD"],
    pattern: '/api/probes/:id',
    tokens: [{"old":"/api/probes/:id","type":0,"val":"api","end":""},{"old":"/api/probes/:id","type":0,"val":"probes","end":""},{"old":"/api/probes/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['probes.show']['types'],
  },
  'probes.update': {
    methods: ["PUT","PATCH"],
    pattern: '/api/probes/:id',
    tokens: [{"old":"/api/probes/:id","type":0,"val":"api","end":""},{"old":"/api/probes/:id","type":0,"val":"probes","end":""},{"old":"/api/probes/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['probes.update']['types'],
  },
  'probes.destroy': {
    methods: ["DELETE"],
    pattern: '/api/probes/:id',
    tokens: [{"old":"/api/probes/:id","type":0,"val":"api","end":""},{"old":"/api/probes/:id","type":0,"val":"probes","end":""},{"old":"/api/probes/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['probes.destroy']['types'],
  },
  'alerts.catalog_index': {
    methods: ["GET","HEAD"],
    pattern: '/api/alert-rules/catalog',
    tokens: [{"old":"/api/alert-rules/catalog","type":0,"val":"api","end":""},{"old":"/api/alert-rules/catalog","type":0,"val":"alert-rules","end":""},{"old":"/api/alert-rules/catalog","type":0,"val":"catalog","end":""}],
    types: placeholder as Registry['alerts.catalog_index']['types'],
  },
  'alerts.catalog_apply': {
    methods: ["POST"],
    pattern: '/api/alert-rules/catalog',
    tokens: [{"old":"/api/alert-rules/catalog","type":0,"val":"api","end":""},{"old":"/api/alert-rules/catalog","type":0,"val":"alert-rules","end":""},{"old":"/api/alert-rules/catalog","type":0,"val":"catalog","end":""}],
    types: placeholder as Registry['alerts.catalog_apply']['types'],
  },
  'alerts.rules_index': {
    methods: ["GET","HEAD"],
    pattern: '/api/alert-rules',
    tokens: [{"old":"/api/alert-rules","type":0,"val":"api","end":""},{"old":"/api/alert-rules","type":0,"val":"alert-rules","end":""}],
    types: placeholder as Registry['alerts.rules_index']['types'],
  },
  'alerts.rules_store': {
    methods: ["POST"],
    pattern: '/api/alert-rules',
    tokens: [{"old":"/api/alert-rules","type":0,"val":"api","end":""},{"old":"/api/alert-rules","type":0,"val":"alert-rules","end":""}],
    types: placeholder as Registry['alerts.rules_store']['types'],
  },
  'alerts.rules_update': {
    methods: ["PUT"],
    pattern: '/api/alert-rules/:id',
    tokens: [{"old":"/api/alert-rules/:id","type":0,"val":"api","end":""},{"old":"/api/alert-rules/:id","type":0,"val":"alert-rules","end":""},{"old":"/api/alert-rules/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['alerts.rules_update']['types'],
  },
  'alerts.rules_destroy': {
    methods: ["DELETE"],
    pattern: '/api/alert-rules/:id',
    tokens: [{"old":"/api/alert-rules/:id","type":0,"val":"api","end":""},{"old":"/api/alert-rules/:id","type":0,"val":"alert-rules","end":""},{"old":"/api/alert-rules/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['alerts.rules_destroy']['types'],
  },
  'alerts.index': {
    methods: ["GET","HEAD"],
    pattern: '/api/alerts',
    tokens: [{"old":"/api/alerts","type":0,"val":"api","end":""},{"old":"/api/alerts","type":0,"val":"alerts","end":""}],
    types: placeholder as Registry['alerts.index']['types'],
  },
  'alerts.acknowledge': {
    methods: ["POST"],
    pattern: '/api/alerts/:id/acknowledge',
    tokens: [{"old":"/api/alerts/:id/acknowledge","type":0,"val":"api","end":""},{"old":"/api/alerts/:id/acknowledge","type":0,"val":"alerts","end":""},{"old":"/api/alerts/:id/acknowledge","type":1,"val":"id","end":""},{"old":"/api/alerts/:id/acknowledge","type":0,"val":"acknowledge","end":""}],
    types: placeholder as Registry['alerts.acknowledge']['types'],
  },
  'alerts.silence': {
    methods: ["POST"],
    pattern: '/api/alerts/:id/silence',
    tokens: [{"old":"/api/alerts/:id/silence","type":0,"val":"api","end":""},{"old":"/api/alerts/:id/silence","type":0,"val":"alerts","end":""},{"old":"/api/alerts/:id/silence","type":1,"val":"id","end":""},{"old":"/api/alerts/:id/silence","type":0,"val":"silence","end":""}],
    types: placeholder as Registry['alerts.silence']['types'],
  },
  'vpn_servers.show': {
    methods: ["GET","HEAD"],
    pattern: '/api/vpn/server',
    tokens: [{"old":"/api/vpn/server","type":0,"val":"api","end":""},{"old":"/api/vpn/server","type":0,"val":"vpn","end":""},{"old":"/api/vpn/server","type":0,"val":"server","end":""}],
    types: placeholder as Registry['vpn_servers.show']['types'],
  },
  'vpn_servers.update': {
    methods: ["PUT"],
    pattern: '/api/vpn/server',
    tokens: [{"old":"/api/vpn/server","type":0,"val":"api","end":""},{"old":"/api/vpn/server","type":0,"val":"vpn","end":""},{"old":"/api/vpn/server","type":0,"val":"server","end":""}],
    types: placeholder as Registry['vpn_servers.update']['types'],
  },
  'vpn_servers.preflight': {
    methods: ["POST"],
    pattern: '/api/vpn/server/preflight',
    tokens: [{"old":"/api/vpn/server/preflight","type":0,"val":"api","end":""},{"old":"/api/vpn/server/preflight","type":0,"val":"vpn","end":""},{"old":"/api/vpn/server/preflight","type":0,"val":"server","end":""},{"old":"/api/vpn/server/preflight","type":0,"val":"preflight","end":""}],
    types: placeholder as Registry['vpn_servers.preflight']['types'],
  },
  'vpn_servers.detect_endpoint': {
    methods: ["POST"],
    pattern: '/api/vpn/server/detect-endpoint',
    tokens: [{"old":"/api/vpn/server/detect-endpoint","type":0,"val":"api","end":""},{"old":"/api/vpn/server/detect-endpoint","type":0,"val":"vpn","end":""},{"old":"/api/vpn/server/detect-endpoint","type":0,"val":"server","end":""},{"old":"/api/vpn/server/detect-endpoint","type":0,"val":"detect-endpoint","end":""}],
    types: placeholder as Registry['vpn_servers.detect_endpoint']['types'],
  },
  'vpn_peers.index': {
    methods: ["GET","HEAD"],
    pattern: '/api/vpn/peers',
    tokens: [{"old":"/api/vpn/peers","type":0,"val":"api","end":""},{"old":"/api/vpn/peers","type":0,"val":"vpn","end":""},{"old":"/api/vpn/peers","type":0,"val":"peers","end":""}],
    types: placeholder as Registry['vpn_peers.index']['types'],
  },
  'vpn_peers.next_ip': {
    methods: ["GET","HEAD"],
    pattern: '/api/vpn/peers/next-ip',
    tokens: [{"old":"/api/vpn/peers/next-ip","type":0,"val":"api","end":""},{"old":"/api/vpn/peers/next-ip","type":0,"val":"vpn","end":""},{"old":"/api/vpn/peers/next-ip","type":0,"val":"peers","end":""},{"old":"/api/vpn/peers/next-ip","type":0,"val":"next-ip","end":""}],
    types: placeholder as Registry['vpn_peers.next_ip']['types'],
  },
  'vpn_peers.store': {
    methods: ["POST"],
    pattern: '/api/vpn/peers',
    tokens: [{"old":"/api/vpn/peers","type":0,"val":"api","end":""},{"old":"/api/vpn/peers","type":0,"val":"vpn","end":""},{"old":"/api/vpn/peers","type":0,"val":"peers","end":""}],
    types: placeholder as Registry['vpn_peers.store']['types'],
  },
  'vpn_peers.config': {
    methods: ["GET","HEAD"],
    pattern: '/api/vpn/peers/:id/config',
    tokens: [{"old":"/api/vpn/peers/:id/config","type":0,"val":"api","end":""},{"old":"/api/vpn/peers/:id/config","type":0,"val":"vpn","end":""},{"old":"/api/vpn/peers/:id/config","type":0,"val":"peers","end":""},{"old":"/api/vpn/peers/:id/config","type":1,"val":"id","end":""},{"old":"/api/vpn/peers/:id/config","type":0,"val":"config","end":""}],
    types: placeholder as Registry['vpn_peers.config']['types'],
  },
  'vpn_peers.qrcode': {
    methods: ["GET","HEAD"],
    pattern: '/api/vpn/peers/:id/qrcode',
    tokens: [{"old":"/api/vpn/peers/:id/qrcode","type":0,"val":"api","end":""},{"old":"/api/vpn/peers/:id/qrcode","type":0,"val":"vpn","end":""},{"old":"/api/vpn/peers/:id/qrcode","type":0,"val":"peers","end":""},{"old":"/api/vpn/peers/:id/qrcode","type":1,"val":"id","end":""},{"old":"/api/vpn/peers/:id/qrcode","type":0,"val":"qrcode","end":""}],
    types: placeholder as Registry['vpn_peers.qrcode']['types'],
  },
  'vpn_peers.rotate': {
    methods: ["POST"],
    pattern: '/api/vpn/peers/:id/rotate',
    tokens: [{"old":"/api/vpn/peers/:id/rotate","type":0,"val":"api","end":""},{"old":"/api/vpn/peers/:id/rotate","type":0,"val":"vpn","end":""},{"old":"/api/vpn/peers/:id/rotate","type":0,"val":"peers","end":""},{"old":"/api/vpn/peers/:id/rotate","type":1,"val":"id","end":""},{"old":"/api/vpn/peers/:id/rotate","type":0,"val":"rotate","end":""}],
    types: placeholder as Registry['vpn_peers.rotate']['types'],
  },
  'vpn_peers.firewall_hints': {
    methods: ["POST"],
    pattern: '/api/vpn/peers/:id/firewall-hints',
    tokens: [{"old":"/api/vpn/peers/:id/firewall-hints","type":0,"val":"api","end":""},{"old":"/api/vpn/peers/:id/firewall-hints","type":0,"val":"vpn","end":""},{"old":"/api/vpn/peers/:id/firewall-hints","type":0,"val":"peers","end":""},{"old":"/api/vpn/peers/:id/firewall-hints","type":1,"val":"id","end":""},{"old":"/api/vpn/peers/:id/firewall-hints","type":0,"val":"firewall-hints","end":""}],
    types: placeholder as Registry['vpn_peers.firewall_hints']['types'],
  },
  'vpn_peers.destroy': {
    methods: ["DELETE"],
    pattern: '/api/vpn/peers/:id',
    tokens: [{"old":"/api/vpn/peers/:id","type":0,"val":"api","end":""},{"old":"/api/vpn/peers/:id","type":0,"val":"vpn","end":""},{"old":"/api/vpn/peers/:id","type":0,"val":"peers","end":""},{"old":"/api/vpn/peers/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['vpn_peers.destroy']['types'],
  },
  'zabbix_templates.index': {
    methods: ["GET","HEAD"],
    pattern: '/api/zabbix-templates',
    tokens: [{"old":"/api/zabbix-templates","type":0,"val":"api","end":""},{"old":"/api/zabbix-templates","type":0,"val":"zabbix-templates","end":""}],
    types: placeholder as Registry['zabbix_templates.index']['types'],
  },
  'zabbix_templates.store': {
    methods: ["POST"],
    pattern: '/api/zabbix-templates',
    tokens: [{"old":"/api/zabbix-templates","type":0,"val":"api","end":""},{"old":"/api/zabbix-templates","type":0,"val":"zabbix-templates","end":""}],
    types: placeholder as Registry['zabbix_templates.store']['types'],
  },
  'zabbix_templates.show': {
    methods: ["GET","HEAD"],
    pattern: '/api/zabbix-templates/:id',
    tokens: [{"old":"/api/zabbix-templates/:id","type":0,"val":"api","end":""},{"old":"/api/zabbix-templates/:id","type":0,"val":"zabbix-templates","end":""},{"old":"/api/zabbix-templates/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['zabbix_templates.show']['types'],
  },
  'zabbix_templates.destroy': {
    methods: ["DELETE"],
    pattern: '/api/zabbix-templates/:id',
    tokens: [{"old":"/api/zabbix-templates/:id","type":0,"val":"api","end":""},{"old":"/api/zabbix-templates/:id","type":0,"val":"zabbix-templates","end":""},{"old":"/api/zabbix-templates/:id","type":1,"val":"id","end":""}],
    types: placeholder as Registry['zabbix_templates.destroy']['types'],
  },
  'events.stream': {
    methods: ["GET","HEAD"],
    pattern: '/api/events/stream',
    tokens: [{"old":"/api/events/stream","type":0,"val":"api","end":""},{"old":"/api/events/stream","type":0,"val":"events","end":""},{"old":"/api/events/stream","type":0,"val":"stream","end":""}],
    types: placeholder as Registry['events.stream']['types'],
  },
} as const satisfies Record<string, AdonisEndpoint>

export { routes }

export const registry = {
  routes,
  $tree: {} as ApiDefinition,
}

declare module '@tuyau/core/types' {
  export interface UserRegistry {
    routes: typeof routes
    $tree: ApiDefinition
  }
}
