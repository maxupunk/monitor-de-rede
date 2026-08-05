/* eslint-disable prettier/prettier */
/// <reference path="../manifest.d.ts" />

import type { ExtractBody, ExtractErrorResponse, ExtractQuery, ExtractQueryForGet, ExtractResponse } from '@tuyau/core/types'
import type { InferInput, SimpleError } from '@vinejs/vine/types'

export type ParamValue = string | number | bigint | boolean

export interface Registry {
  'auth.login': {
    methods: ["POST"]
    pattern: '/api/auth/login'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/auth_controller').default['login']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/auth_controller').default['login']>>>
    }
  }
  'auth.logout': {
    methods: ["POST"]
    pattern: '/api/auth/logout'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/auth_controller').default['logout']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/auth_controller').default['logout']>>>
    }
  }
  'auth.me': {
    methods: ["GET","HEAD"]
    pattern: '/api/auth/me'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/auth_controller').default['me']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/auth_controller').default['me']>>>
    }
  }
  'sites.index': {
    methods: ["GET","HEAD"]
    pattern: '/api/sites'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/sites_controller').default['index']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/sites_controller').default['index']>>>
    }
  }
  'sites.store': {
    methods: ["POST"]
    pattern: '/api/sites'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/sites_controller').default['store']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/sites_controller').default['store']>>>
    }
  }
  'sites.show': {
    methods: ["GET","HEAD"]
    pattern: '/api/sites/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/sites_controller').default['show']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/sites_controller').default['show']>>>
    }
  }
  'sites.update': {
    methods: ["PUT","PATCH"]
    pattern: '/api/sites/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/sites_controller').default['update']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/sites_controller').default['update']>>>
    }
  }
  'sites.destroy': {
    methods: ["DELETE"]
    pattern: '/api/sites/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/sites_controller').default['destroy']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/sites_controller').default['destroy']>>>
    }
  }
  'networks.scan': {
    methods: ["POST"]
    pattern: '/api/networks/:id/scan'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['scan']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['scan']>>>
    }
  }
  'networks.index': {
    methods: ["GET","HEAD"]
    pattern: '/api/networks'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['index']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['index']>>>
    }
  }
  'networks.store': {
    methods: ["POST"]
    pattern: '/api/networks'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['store']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['store']>>>
    }
  }
  'networks.show': {
    methods: ["GET","HEAD"]
    pattern: '/api/networks/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['show']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['show']>>>
    }
  }
  'networks.update': {
    methods: ["PUT","PATCH"]
    pattern: '/api/networks/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['update']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['update']>>>
    }
  }
  'networks.destroy': {
    methods: ["DELETE"]
    pattern: '/api/networks/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['destroy']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/networks_controller').default['destroy']>>>
    }
  }
  'snmp.test': {
    methods: ["POST"]
    pattern: '/api/snmp/test'
    types: {
      body: ExtractBody<InferInput<(typeof import('@vinejs/vine').default)>>
      paramsTuple: []
      params: {}
      query: ExtractQuery<InferInput<(typeof import('@vinejs/vine').default)>>
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/snmp_controller').default['test']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/snmp_controller').default['test']>>> | { status: 422; response: { errors: SimpleError[] } }
    }
  }
  'port_scan.scan': {
    methods: ["POST"]
    pattern: '/api/port-scan'
    types: {
      body: ExtractBody<InferInput<(typeof import('@vinejs/vine').default)>>
      paramsTuple: []
      params: {}
      query: ExtractQuery<InferInput<(typeof import('@vinejs/vine').default)>>
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/port_scan_controller').default['scan']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/port_scan_controller').default['scan']>>> | { status: 422; response: { errors: SimpleError[] } }
    }
  }
  'snmp.poll': {
    methods: ["POST"]
    pattern: '/api/devices/:id/snmp/poll'
    types: {
      body: ExtractBody<InferInput<(typeof import('@vinejs/vine').default)>>
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: ExtractQuery<InferInput<(typeof import('@vinejs/vine').default)>>
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/snmp_controller').default['poll']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/snmp_controller').default['poll']>>> | { status: 422; response: { errors: SimpleError[] } }
    }
  }
  'snmp.scan': {
    methods: ["POST"]
    pattern: '/api/devices/:id/snmp/scan'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/snmp_controller').default['scan']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/snmp_controller').default['scan']>>>
    }
  }
  'snmp.apply_monitors': {
    methods: ["POST"]
    pattern: '/api/devices/:id/snmp/apply-monitors'
    types: {
      body: ExtractBody<InferInput<(typeof import('@vinejs/vine').default)>>
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: ExtractQuery<InferInput<(typeof import('@vinejs/vine').default)>>
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/snmp_controller').default['applyMonitors']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/snmp_controller').default['applyMonitors']>>> | { status: 422; response: { errors: SimpleError[] } }
    }
  }
  'snmp.interfaces': {
    methods: ["GET","HEAD"]
    pattern: '/api/devices/:id/interfaces'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/snmp_controller').default['interfaces']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/snmp_controller').default['interfaces']>>>
    }
  }
  'devices.monitors': {
    methods: ["GET","HEAD"]
    pattern: '/api/devices/:id/monitors'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['monitors']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['monitors']>>>
    }
  }
  'devices.metrics': {
    methods: ["GET","HEAD"]
    pattern: '/api/devices/:id/metrics'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['metrics']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['metrics']>>>
    }
  }
  'devices.events': {
    methods: ["GET","HEAD"]
    pattern: '/api/devices/:id/events'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['events']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['events']>>>
    }
  }
  'devices.index': {
    methods: ["GET","HEAD"]
    pattern: '/api/devices'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['index']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['index']>>>
    }
  }
  'devices.store': {
    methods: ["POST"]
    pattern: '/api/devices'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['store']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['store']>>>
    }
  }
  'devices.show': {
    methods: ["GET","HEAD"]
    pattern: '/api/devices/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['show']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['show']>>>
    }
  }
  'devices.update': {
    methods: ["PUT","PATCH"]
    pattern: '/api/devices/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['update']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['update']>>>
    }
  }
  'devices.destroy': {
    methods: ["DELETE"]
    pattern: '/api/devices/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['destroy']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/devices_controller').default['destroy']>>>
    }
  }
  'monitors.run': {
    methods: ["POST"]
    pattern: '/api/monitors/:id/run'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['run']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['run']>>>
    }
  }
  'monitors.enable': {
    methods: ["POST"]
    pattern: '/api/monitors/:id/enable'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['enable']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['enable']>>>
    }
  }
  'monitors.disable': {
    methods: ["POST"]
    pattern: '/api/monitors/:id/disable'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['disable']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['disable']>>>
    }
  }
  'monitors.index': {
    methods: ["GET","HEAD"]
    pattern: '/api/monitors'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['index']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['index']>>>
    }
  }
  'monitors.store': {
    methods: ["POST"]
    pattern: '/api/monitors'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['store']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['store']>>>
    }
  }
  'monitors.show': {
    methods: ["GET","HEAD"]
    pattern: '/api/monitors/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['show']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['show']>>>
    }
  }
  'monitors.update': {
    methods: ["PUT","PATCH"]
    pattern: '/api/monitors/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['update']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['update']>>>
    }
  }
  'monitors.destroy': {
    methods: ["DELETE"]
    pattern: '/api/monitors/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['destroy']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/monitors_controller').default['destroy']>>>
    }
  }
  'discovery.runs': {
    methods: ["GET","HEAD"]
    pattern: '/api/discovery/runs'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['runs']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['runs']>>>
    }
  }
  'discovery.run_details': {
    methods: ["GET","HEAD"]
    pattern: '/api/discovery/runs/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['runDetails']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['runDetails']>>>
    }
  }
  'discovery.results': {
    methods: ["GET","HEAD"]
    pattern: '/api/discovery/results'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['results']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['results']>>>
    }
  }
  'discovery.accept': {
    methods: ["POST"]
    pattern: '/api/discovery/results/:id/accept'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['accept']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['accept']>>>
    }
  }
  'discovery.ignore': {
    methods: ["POST"]
    pattern: '/api/discovery/results/:id/ignore'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['ignore']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['ignore']>>>
    }
  }
  'discovery.merge': {
    methods: ["POST"]
    pattern: '/api/discovery/results/:id/merge'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['merge']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/discovery_controller').default['merge']>>>
    }
  }
  'topology.index': {
    methods: ["GET","HEAD"]
    pattern: '/api/topology'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/topology_controller').default['index']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/topology_controller').default['index']>>>
    }
  }
  'topology.store_link': {
    methods: ["POST"]
    pattern: '/api/topology/links'
    types: {
      body: ExtractBody<InferInput<(typeof import('@vinejs/vine').default)>>
      paramsTuple: []
      params: {}
      query: ExtractQuery<InferInput<(typeof import('@vinejs/vine').default)>>
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/topology_controller').default['storeLink']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/topology_controller').default['storeLink']>>> | { status: 422; response: { errors: SimpleError[] } }
    }
  }
  'topology.recalculate': {
    methods: ["POST"]
    pattern: '/api/topology/recalculate'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/topology_controller').default['recalculate']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/topology_controller').default['recalculate']>>>
    }
  }
  'topology.destroy_link': {
    methods: ["DELETE"]
    pattern: '/api/topology/links/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/topology_controller').default['destroyLink']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/topology_controller').default['destroyLink']>>>
    }
  }
  'probes.heartbeat': {
    methods: ["POST"]
    pattern: '/api/probes/heartbeat'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['heartbeat']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['heartbeat']>>>
    }
  }
  'probes.get_tasks': {
    methods: ["GET","HEAD"]
    pattern: '/api/probes/tasks'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['getTasks']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['getTasks']>>>
    }
  }
  'probes.post_results': {
    methods: ["POST"]
    pattern: '/api/probes/results'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['postResults']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['postResults']>>>
    }
  }
  'probes.revoke': {
    methods: ["POST"]
    pattern: '/api/probes/:id/revoke'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['revoke']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['revoke']>>>
    }
  }
  'probes.test': {
    methods: ["POST"]
    pattern: '/api/probes/:id/test'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['test']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['test']>>>
    }
  }
  'probes.index': {
    methods: ["GET","HEAD"]
    pattern: '/api/probes'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['index']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['index']>>>
    }
  }
  'probes.store': {
    methods: ["POST"]
    pattern: '/api/probes'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['store']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['store']>>>
    }
  }
  'probes.show': {
    methods: ["GET","HEAD"]
    pattern: '/api/probes/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['show']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['show']>>>
    }
  }
  'probes.update': {
    methods: ["PUT","PATCH"]
    pattern: '/api/probes/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['update']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['update']>>>
    }
  }
  'probes.destroy': {
    methods: ["DELETE"]
    pattern: '/api/probes/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['destroy']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/probes_controller').default['destroy']>>>
    }
  }
  'alerts.catalog_index': {
    methods: ["GET","HEAD"]
    pattern: '/api/alert-rules/catalog'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['catalogIndex']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['catalogIndex']>>>
    }
  }
  'alerts.catalog_apply': {
    methods: ["POST"]
    pattern: '/api/alert-rules/catalog'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['catalogApply']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['catalogApply']>>>
    }
  }
  'alerts.rules_index': {
    methods: ["GET","HEAD"]
    pattern: '/api/alert-rules'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['rulesIndex']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['rulesIndex']>>>
    }
  }
  'alerts.rules_store': {
    methods: ["POST"]
    pattern: '/api/alert-rules'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['rulesStore']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['rulesStore']>>>
    }
  }
  'alerts.rules_update': {
    methods: ["PUT"]
    pattern: '/api/alert-rules/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['rulesUpdate']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['rulesUpdate']>>>
    }
  }
  'alerts.rules_destroy': {
    methods: ["DELETE"]
    pattern: '/api/alert-rules/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['rulesDestroy']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['rulesDestroy']>>>
    }
  }
  'alerts.index': {
    methods: ["GET","HEAD"]
    pattern: '/api/alerts'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['index']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['index']>>>
    }
  }
  'alerts.acknowledge': {
    methods: ["POST"]
    pattern: '/api/alerts/:id/acknowledge'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['acknowledge']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['acknowledge']>>>
    }
  }
  'alerts.silence': {
    methods: ["POST"]
    pattern: '/api/alerts/:id/silence'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['silence']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/alerts_controller').default['silence']>>>
    }
  }
  'vpn_servers.show': {
    methods: ["GET","HEAD"]
    pattern: '/api/vpn/server'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_servers_controller').default['show']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_servers_controller').default['show']>>>
    }
  }
  'vpn_servers.update': {
    methods: ["PUT"]
    pattern: '/api/vpn/server'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_servers_controller').default['update']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_servers_controller').default['update']>>>
    }
  }
  'vpn_servers.preflight': {
    methods: ["POST"]
    pattern: '/api/vpn/server/preflight'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_servers_controller').default['preflight']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_servers_controller').default['preflight']>>>
    }
  }
  'vpn_servers.detect_endpoint': {
    methods: ["POST"]
    pattern: '/api/vpn/server/detect-endpoint'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_servers_controller').default['detectEndpoint']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_servers_controller').default['detectEndpoint']>>>
    }
  }
  'vpn_peers.index': {
    methods: ["GET","HEAD"]
    pattern: '/api/vpn/peers'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['index']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['index']>>>
    }
  }
  'vpn_peers.next_ip': {
    methods: ["GET","HEAD"]
    pattern: '/api/vpn/peers/next-ip'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['nextIp']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['nextIp']>>>
    }
  }
  'vpn_peers.store': {
    methods: ["POST"]
    pattern: '/api/vpn/peers'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['store']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['store']>>>
    }
  }
  'vpn_peers.config': {
    methods: ["GET","HEAD"]
    pattern: '/api/vpn/peers/:id/config'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['config']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['config']>>>
    }
  }
  'vpn_peers.qrcode': {
    methods: ["GET","HEAD"]
    pattern: '/api/vpn/peers/:id/qrcode'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['qrcode']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['qrcode']>>>
    }
  }
  'vpn_peers.rotate': {
    methods: ["POST"]
    pattern: '/api/vpn/peers/:id/rotate'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['rotate']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['rotate']>>>
    }
  }
  'vpn_peers.firewall_hints': {
    methods: ["POST"]
    pattern: '/api/vpn/peers/:id/firewall-hints'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['firewallHints']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['firewallHints']>>>
    }
  }
  'vpn_peers.destroy': {
    methods: ["DELETE"]
    pattern: '/api/vpn/peers/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['destroy']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/vpn_peers_controller').default['destroy']>>>
    }
  }
  'zabbix_templates.index': {
    methods: ["GET","HEAD"]
    pattern: '/api/zabbix-templates'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/zabbix_templates_controller').default['index']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/zabbix_templates_controller').default['index']>>>
    }
  }
  'zabbix_templates.store': {
    methods: ["POST"]
    pattern: '/api/zabbix-templates'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/zabbix_templates_controller').default['store']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/zabbix_templates_controller').default['store']>>>
    }
  }
  'zabbix_templates.show': {
    methods: ["GET","HEAD"]
    pattern: '/api/zabbix-templates/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/zabbix_templates_controller').default['show']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/zabbix_templates_controller').default['show']>>>
    }
  }
  'zabbix_templates.destroy': {
    methods: ["DELETE"]
    pattern: '/api/zabbix-templates/:id'
    types: {
      body: {}
      paramsTuple: [ParamValue]
      params: { id: ParamValue }
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/zabbix_templates_controller').default['destroy']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/zabbix_templates_controller').default['destroy']>>>
    }
  }
  'events.stream': {
    methods: ["GET","HEAD"]
    pattern: '/api/events/stream'
    types: {
      body: {}
      paramsTuple: []
      params: {}
      query: {}
      response: ExtractResponse<Awaited<ReturnType<import('#controllers/events_controller').default['stream']>>>
      errorResponse: ExtractErrorResponse<Awaited<ReturnType<import('#controllers/events_controller').default['stream']>>>
    }
  }
}
