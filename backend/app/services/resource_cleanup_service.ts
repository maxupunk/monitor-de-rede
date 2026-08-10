import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
import Metric from '#models/metric'
import AlertEvent from '#models/alert_event'
import AlertRule from '#models/alert_rule'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'
import DeviceLink from '#models/device_link'
import Probe from '#models/probe'
import Site from '#models/site'
import ZabbixTemplate from '#models/zabbix_template'
import ZabbixTemplateItem from '#models/zabbix_template_item'

/**
 * Serviço central para garantir a remoção completa de itens e todo o seu histórico
 * (resultados, métricas, alertas, regras e relacionamentos) ao serem excluídos do sistema.
 */
export class ResourceCleanupService {
  /**
   * Apaga completamente um monitor e TODO o seu histórico (resultados, métricas e alertas).
   */
  async deleteMonitor(monitorId: number): Promise<void> {
    await MonitorResult.query().where('monitorId', monitorId).delete()
    await Metric.query().where('monitorId', monitorId).delete()
    await AlertEvent.query().where('monitorId', monitorId).delete()
    await AlertRule.query().where('monitorId', monitorId).delete()
    await Monitor.query().where('id', monitorId).delete()
  }

  /**
   * Apaga um equipamento, todos os seus monitores vinculados, todo o histórico
   * de métricas, eventos de alerta, interfaces e ligações de topologia.
   */
  async deleteDevice(deviceId: number): Promise<void> {
    const monitors = await Monitor.query().where('deviceId', deviceId)
    for (const monitor of monitors) {
      await this.deleteMonitor(monitor.id)
    }

    const interfaces = await DeviceInterface.query().where('deviceId', deviceId)
    const interfaceIds = interfaces.map((i) => i.id)

    if (interfaceIds.length > 0) {
      await Metric.query().whereIn('interfaceId', interfaceIds).delete()
      await DeviceInterface.query().whereIn('id', interfaceIds).delete()
    }

    await Metric.query().where('deviceId', deviceId).delete()
    await AlertEvent.query().where('deviceId', deviceId).delete()
    await AlertRule.query().where('deviceId', deviceId).delete()

    await DeviceLink.query()
      .where('sourceDeviceId', deviceId)
      .orWhere('targetDeviceId', deviceId)
      .delete()

    await Device.query().where('id', deviceId).delete()
  }

  /**
   * Apaga um site, seus equipamentos, monitores isolados e todo o histórico associado.
   */
  async deleteSite(siteId: number): Promise<void> {
    const devices = await Device.query().where('siteId', siteId)
    for (const device of devices) {
      await this.deleteDevice(device.id)
    }

    await AlertRule.query().where('siteId', siteId).delete()
    await Probe.query().where('siteId', siteId).update({ siteId: null })
    await Site.query().where('id', siteId).delete()
  }

  /**
   * Apaga um probe, todos os seus monitores vinculados e o histórico de resultados.
   */
  async deleteProbe(probeId: number): Promise<void> {
    const monitors = await Monitor.query().where('probeId', probeId)
    for (const monitor of monitors) {
      await this.deleteMonitor(monitor.id)
    }

    await MonitorResult.query().where('probeId', probeId).delete()
    await Probe.query().where('id', probeId).delete()
  }

  /**
   * Apaga um template Zabbix, seus itens e desvincula os equipamentos.
   */
  async deleteZabbixTemplate(templateId: number): Promise<void> {
    await Device.query().where('zabbixTemplateId', templateId).update({ zabbixTemplateId: null })
    await ZabbixTemplateItem.query().where('templateId', templateId).delete()
    await ZabbixTemplate.query().where('id', templateId).delete()
  }
}
