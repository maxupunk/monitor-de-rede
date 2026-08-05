import '@adonisjs/core/types/http'

type ParamValue = string | number | bigint | boolean

export type ScannedRoutes = {
  ALL: {
    'auth.login': { paramsTuple?: []; params?: {} }
    'auth.logout': { paramsTuple?: []; params?: {} }
    'auth.me': { paramsTuple?: []; params?: {} }
    'sites.index': { paramsTuple?: []; params?: {} }
    'sites.store': { paramsTuple?: []; params?: {} }
    'sites.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'sites.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'sites.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'networks.scan': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'networks.index': { paramsTuple?: []; params?: {} }
    'networks.store': { paramsTuple?: []; params?: {} }
    'networks.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'networks.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'networks.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'snmp.test': { paramsTuple?: []; params?: {} }
    'port_scan.scan': { paramsTuple?: []; params?: {} }
    'dns.benchmark': { paramsTuple?: []; params?: {} }
    'dns.lookup': { paramsTuple?: []; params?: {} }
    'dns.performance': { paramsTuple?: []; params?: {} }
    'dns_servers.index': { paramsTuple?: []; params?: {} }
    'dns_servers.store': { paramsTuple?: []; params?: {} }
    'dns_servers.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'dns_servers.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'snmp.poll': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'snmp.scan': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'snmp.apply_monitors': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'snmp.interfaces': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.monitors': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.metrics': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.events': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.index': { paramsTuple?: []; params?: {} }
    'devices.store': { paramsTuple?: []; params?: {} }
    'devices.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.run': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.enable': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.disable': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.index': { paramsTuple?: []; params?: {} }
    'monitors.store': { paramsTuple?: []; params?: {} }
    'monitors.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'discovery.runs': { paramsTuple?: []; params?: {} }
    'discovery.run_details': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'discovery.results': { paramsTuple?: []; params?: {} }
    'discovery.accept': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'discovery.ignore': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'discovery.merge': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'topology.index': { paramsTuple?: []; params?: {} }
    'topology.store_link': { paramsTuple?: []; params?: {} }
    'topology.recalculate': { paramsTuple?: []; params?: {} }
    'topology.destroy_link': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'probes.heartbeat': { paramsTuple?: []; params?: {} }
    'probes.get_tasks': { paramsTuple?: []; params?: {} }
    'probes.post_results': { paramsTuple?: []; params?: {} }
    'probes.revoke': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'probes.test': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'probes.index': { paramsTuple?: []; params?: {} }
    'probes.store': { paramsTuple?: []; params?: {} }
    'probes.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'probes.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'probes.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'alerts.catalog_index': { paramsTuple?: []; params?: {} }
    'alerts.catalog_apply': { paramsTuple?: []; params?: {} }
    'alerts.rules_index': { paramsTuple?: []; params?: {} }
    'alerts.rules_store': { paramsTuple?: []; params?: {} }
    'alerts.rules_update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'alerts.rules_destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'alerts.index': { paramsTuple?: []; params?: {} }
    'alerts.acknowledge': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'alerts.silence': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_servers.show': { paramsTuple?: []; params?: {} }
    'vpn_servers.update': { paramsTuple?: []; params?: {} }
    'vpn_servers.preflight': { paramsTuple?: []; params?: {} }
    'vpn_servers.detect_endpoint': { paramsTuple?: []; params?: {} }
    'vpn_peers.index': { paramsTuple?: []; params?: {} }
    'vpn_peers.next_ip': { paramsTuple?: []; params?: {} }
    'vpn_peers.store': { paramsTuple?: []; params?: {} }
    'vpn_peers.config': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_peers.qrcode': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_peers.rotate': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_peers.firewall_hints': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_peers.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'zabbix_templates.index': { paramsTuple?: []; params?: {} }
    'zabbix_templates.store': { paramsTuple?: []; params?: {} }
    'zabbix_templates.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'zabbix_templates.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'events.stream': { paramsTuple?: []; params?: {} }
  }
  GET: {
    'auth.me': { paramsTuple?: []; params?: {} }
    'sites.index': { paramsTuple?: []; params?: {} }
    'sites.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'networks.index': { paramsTuple?: []; params?: {} }
    'networks.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'dns.performance': { paramsTuple?: []; params?: {} }
    'dns_servers.index': { paramsTuple?: []; params?: {} }
    'snmp.interfaces': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.monitors': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.metrics': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.events': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.index': { paramsTuple?: []; params?: {} }
    'devices.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.index': { paramsTuple?: []; params?: {} }
    'monitors.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'discovery.runs': { paramsTuple?: []; params?: {} }
    'discovery.run_details': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'discovery.results': { paramsTuple?: []; params?: {} }
    'topology.index': { paramsTuple?: []; params?: {} }
    'probes.get_tasks': { paramsTuple?: []; params?: {} }
    'probes.index': { paramsTuple?: []; params?: {} }
    'probes.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'alerts.catalog_index': { paramsTuple?: []; params?: {} }
    'alerts.rules_index': { paramsTuple?: []; params?: {} }
    'alerts.index': { paramsTuple?: []; params?: {} }
    'vpn_servers.show': { paramsTuple?: []; params?: {} }
    'vpn_peers.index': { paramsTuple?: []; params?: {} }
    'vpn_peers.next_ip': { paramsTuple?: []; params?: {} }
    'vpn_peers.config': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_peers.qrcode': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'zabbix_templates.index': { paramsTuple?: []; params?: {} }
    'zabbix_templates.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'events.stream': { paramsTuple?: []; params?: {} }
  }
  HEAD: {
    'auth.me': { paramsTuple?: []; params?: {} }
    'sites.index': { paramsTuple?: []; params?: {} }
    'sites.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'networks.index': { paramsTuple?: []; params?: {} }
    'networks.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'dns.performance': { paramsTuple?: []; params?: {} }
    'dns_servers.index': { paramsTuple?: []; params?: {} }
    'snmp.interfaces': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.monitors': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.metrics': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.events': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.index': { paramsTuple?: []; params?: {} }
    'devices.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.index': { paramsTuple?: []; params?: {} }
    'monitors.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'discovery.runs': { paramsTuple?: []; params?: {} }
    'discovery.run_details': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'discovery.results': { paramsTuple?: []; params?: {} }
    'topology.index': { paramsTuple?: []; params?: {} }
    'probes.get_tasks': { paramsTuple?: []; params?: {} }
    'probes.index': { paramsTuple?: []; params?: {} }
    'probes.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'alerts.catalog_index': { paramsTuple?: []; params?: {} }
    'alerts.rules_index': { paramsTuple?: []; params?: {} }
    'alerts.index': { paramsTuple?: []; params?: {} }
    'vpn_servers.show': { paramsTuple?: []; params?: {} }
    'vpn_peers.index': { paramsTuple?: []; params?: {} }
    'vpn_peers.next_ip': { paramsTuple?: []; params?: {} }
    'vpn_peers.config': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_peers.qrcode': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'zabbix_templates.index': { paramsTuple?: []; params?: {} }
    'zabbix_templates.show': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'events.stream': { paramsTuple?: []; params?: {} }
  }
  POST: {
    'auth.login': { paramsTuple?: []; params?: {} }
    'auth.logout': { paramsTuple?: []; params?: {} }
    'sites.store': { paramsTuple?: []; params?: {} }
    'networks.scan': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'networks.store': { paramsTuple?: []; params?: {} }
    'snmp.test': { paramsTuple?: []; params?: {} }
    'port_scan.scan': { paramsTuple?: []; params?: {} }
    'dns.benchmark': { paramsTuple?: []; params?: {} }
    'dns.lookup': { paramsTuple?: []; params?: {} }
    'dns_servers.store': { paramsTuple?: []; params?: {} }
    'snmp.poll': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'snmp.scan': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'snmp.apply_monitors': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.store': { paramsTuple?: []; params?: {} }
    'monitors.run': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.enable': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.disable': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.store': { paramsTuple?: []; params?: {} }
    'discovery.accept': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'discovery.ignore': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'discovery.merge': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'topology.store_link': { paramsTuple?: []; params?: {} }
    'topology.recalculate': { paramsTuple?: []; params?: {} }
    'probes.heartbeat': { paramsTuple?: []; params?: {} }
    'probes.post_results': { paramsTuple?: []; params?: {} }
    'probes.revoke': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'probes.test': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'probes.store': { paramsTuple?: []; params?: {} }
    'alerts.catalog_apply': { paramsTuple?: []; params?: {} }
    'alerts.rules_store': { paramsTuple?: []; params?: {} }
    'alerts.acknowledge': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'alerts.silence': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_servers.preflight': { paramsTuple?: []; params?: {} }
    'vpn_servers.detect_endpoint': { paramsTuple?: []; params?: {} }
    'vpn_peers.store': { paramsTuple?: []; params?: {} }
    'vpn_peers.rotate': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_peers.firewall_hints': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'zabbix_templates.store': { paramsTuple?: []; params?: {} }
  }
  PUT: {
    'sites.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'networks.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'dns_servers.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'probes.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'alerts.rules_update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_servers.update': { paramsTuple?: []; params?: {} }
  }
  PATCH: {
    'sites.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'networks.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'probes.update': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
  }
  DELETE: {
    'sites.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'networks.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'dns_servers.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'devices.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'monitors.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'topology.destroy_link': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'probes.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'alerts.rules_destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'vpn_peers.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
    'zabbix_templates.destroy': { paramsTuple: [ParamValue]; params: {'id': ParamValue} }
  }
}
declare module '@adonisjs/core/types/http' {
  export interface RoutesList extends ScannedRoutes {}
}