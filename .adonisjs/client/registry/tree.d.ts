/* eslint-disable prettier/prettier */
import type { routes } from './index.ts'

export interface ApiDefinition {
  auth: {
    login: typeof routes['auth.login']
    logout: typeof routes['auth.logout']
    me: typeof routes['auth.me']
  }
  sites: {
    index: typeof routes['sites.index']
    store: typeof routes['sites.store']
    show: typeof routes['sites.show']
    update: typeof routes['sites.update']
    destroy: typeof routes['sites.destroy']
  }
  networks: {
    scan: typeof routes['networks.scan']
    index: typeof routes['networks.index']
    store: typeof routes['networks.store']
    show: typeof routes['networks.show']
    update: typeof routes['networks.update']
    destroy: typeof routes['networks.destroy']
  }
  snmp: {
    test: typeof routes['snmp.test']
    poll: typeof routes['snmp.poll']
    scan: typeof routes['snmp.scan']
    applyMonitors: typeof routes['snmp.apply_monitors']
    interfaces: typeof routes['snmp.interfaces']
  }
  portScan: {
    scan: typeof routes['port_scan.scan']
  }
  dns: {
    benchmark: typeof routes['dns.benchmark']
    lookup: typeof routes['dns.lookup']
    performance: typeof routes['dns.performance']
  }
  dnsServers: {
    index: typeof routes['dns_servers.index']
    store: typeof routes['dns_servers.store']
    update: typeof routes['dns_servers.update']
    destroy: typeof routes['dns_servers.destroy']
  }
  devices: {
    monitors: typeof routes['devices.monitors']
    metrics: typeof routes['devices.metrics']
    events: typeof routes['devices.events']
    index: typeof routes['devices.index']
    store: typeof routes['devices.store']
    show: typeof routes['devices.show']
    update: typeof routes['devices.update']
    destroy: typeof routes['devices.destroy']
  }
  monitors: {
    run: typeof routes['monitors.run']
    enable: typeof routes['monitors.enable']
    disable: typeof routes['monitors.disable']
    index: typeof routes['monitors.index']
    store: typeof routes['monitors.store']
    show: typeof routes['monitors.show']
    update: typeof routes['monitors.update']
    destroy: typeof routes['monitors.destroy']
  }
  discovery: {
    runs: typeof routes['discovery.runs']
    runDetails: typeof routes['discovery.run_details']
    results: typeof routes['discovery.results']
    accept: typeof routes['discovery.accept']
    ignore: typeof routes['discovery.ignore']
    merge: typeof routes['discovery.merge']
  }
  topology: {
    index: typeof routes['topology.index']
    storeLink: typeof routes['topology.store_link']
    recalculate: typeof routes['topology.recalculate']
    destroyLink: typeof routes['topology.destroy_link']
  }
  probes: {
    heartbeat: typeof routes['probes.heartbeat']
    getTasks: typeof routes['probes.get_tasks']
    postResults: typeof routes['probes.post_results']
    revoke: typeof routes['probes.revoke']
    test: typeof routes['probes.test']
    index: typeof routes['probes.index']
    store: typeof routes['probes.store']
    show: typeof routes['probes.show']
    update: typeof routes['probes.update']
    destroy: typeof routes['probes.destroy']
  }
  alerts: {
    catalogIndex: typeof routes['alerts.catalog_index']
    catalogApply: typeof routes['alerts.catalog_apply']
    rulesIndex: typeof routes['alerts.rules_index']
    rulesStore: typeof routes['alerts.rules_store']
    rulesUpdate: typeof routes['alerts.rules_update']
    rulesDestroy: typeof routes['alerts.rules_destroy']
    index: typeof routes['alerts.index']
    acknowledge: typeof routes['alerts.acknowledge']
    silence: typeof routes['alerts.silence']
  }
  vpnServers: {
    show: typeof routes['vpn_servers.show']
    update: typeof routes['vpn_servers.update']
    preflight: typeof routes['vpn_servers.preflight']
    detectEndpoint: typeof routes['vpn_servers.detect_endpoint']
  }
  vpnPeers: {
    index: typeof routes['vpn_peers.index']
    nextIp: typeof routes['vpn_peers.next_ip']
    store: typeof routes['vpn_peers.store']
    config: typeof routes['vpn_peers.config']
    qrcode: typeof routes['vpn_peers.qrcode']
    rotate: typeof routes['vpn_peers.rotate']
    firewallHints: typeof routes['vpn_peers.firewall_hints']
    destroy: typeof routes['vpn_peers.destroy']
  }
  zabbixTemplates: {
    index: typeof routes['zabbix_templates.index']
    store: typeof routes['zabbix_templates.store']
    show: typeof routes['zabbix_templates.show']
    destroy: typeof routes['zabbix_templates.destroy']
  }
  events: {
    stream: typeof routes['events.stream']
  }
}
