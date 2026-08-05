import type { HttpContext } from '@adonisjs/core/http'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'
import Monitor from '#models/monitor'
import Metric from '#models/metric'
import { SnmpService } from '#modules/snmp/snmp_service'
import { syncZabbixTemplateMonitor } from '#modules/zabbix/zabbix_template_monitor_sync'
import vine from '@vinejs/vine'

export default class SnmpController {
  private snmpService = new SnmpService()

  /**
   * POST /api/devices/:id/snmp/poll
   * Executa varredura SNMP sob demanda para um dispositivo.
   */
  async poll({ params, request, response }: HttpContext) {
    const device = await Device.find(params.id)
    if (!device) {
      return response.notFound({ message: 'Dispositivo não encontrado' })
    }

    // Autocorrige dispositivos com template vinculado antes de existir o monitor de
    // sincronização (ver comentário em syncZabbixTemplateMonitor) — assim um clique
    // manual em "Poll SNMP Agora" já resolve a coleta periódica também, sem precisar
    // reabrir e salvar o formulário do dispositivo.
    await syncZabbixTemplateMonitor(device)

    const schema = vine.object({
      host: vine.string().optional(),
      version: vine.enum(['v1', 'v2c', 'v3']).optional(),
      community: vine.string().optional(),
      port: vine.number().optional(),
    })

    const payload = await vine.validate({
      schema,
      data: request.all(),
    })

    const version = (payload.version || device.snmpVersion || 'v2c') as 'v1' | 'v2c' | 'v3'
    const config = {
      host: payload.host || device.ipAddress || device.name,
      version,
      community: payload.community || device.snmpCommunity || 'public',
      port: payload.port || 161,
    }

    try {
      const result = await this.snmpService.pollDevice(device, config)
      return response.ok({
        message: 'Varredura SNMP executada com sucesso',
        result,
      })
    } catch (error) {
      return response.badRequest({
        message: 'Falha ao executar varredura SNMP',
        error: error instanceof Error ? error.message : String(error),
      })
    }
  }

  /**
   * POST /api/snmp/test
   * Testa conectividade SNMP com um host arbitrário, sem exigir um dispositivo já
   * cadastrado — usado pelo botão "Testar SNMP" no formulário de cadastro/edição.
   * Com `autoDetect`, tenta combinações comuns de versão/comunidade (public/private,
   * v1/v2c) e retorna a primeira que responder, para auto-preencher o formulário.
   */
  async test({ request, response }: HttpContext) {
    const schema = vine.object({
      host: vine.string().trim().minLength(1),
      port: vine.number().range([1, 65535]).optional(),
      version: vine.enum(['v1', 'v2c', 'v3']).optional(),
      community: vine.string().optional(),
      autoDetect: vine.boolean().optional(),
    })

    const payload = await vine.validate({ schema, data: request.all() })
    const port = payload.port || 161

    try {
      if (payload.autoDetect) {
        const result = await this.snmpService.detectConnection(payload.host, port, {
          version: payload.version,
          community: payload.community,
        })
        return response.ok(result)
      }

      const result = await this.snmpService.testConnection({
        host: payload.host,
        port,
        version: (payload.version || 'v2c') as 'v1' | 'v2c' | 'v3',
        community: payload.community || 'public',
      })
      return response.ok(result)
    } catch (error) {
      return response.badRequest({
        message: 'Falha ao testar conexão SNMP',
        error: error instanceof Error ? error.message : String(error),
      })
    }
  }

  /**
   * POST /api/devices/:id/snmp/scan
   * Escaneia e lista os componentes monitoráveis de um dispositivo (Interfaces, CPU, Memória).
   */
  async scan({ params, response }: HttpContext) {
    const device = await Device.find(params.id)
    if (!device) {
      return response.notFound({ message: 'Dispositivo não encontrado' })
    }

    await syncZabbixTemplateMonitor(device)

    const config = {
      host: device.ipAddress || device.name,
      version: (device.snmpVersion || 'v2c') as 'v1' | 'v2c' | 'v3',
      community: device.snmpCommunity || 'public',
      port: 161,
    }

    try {
      const scanResult = await this.snmpService.scanDevice(device, config)
      return response.ok(scanResult)
    } catch (error) {
      return response.badRequest({
        message: 'Falha ao escanear equipamento via SNMP',
        error: error instanceof Error ? error.message : String(error),
      })
    }
  }

  /**
   * POST /api/devices/:id/snmp/apply-monitors
   * Salva e atualiza os itens de monitoramento selecionados (interfaces, CPU, memória).
   */
  async applyMonitors({ params, request, response }: HttpContext) {
    const device = await Device.find(params.id)
    if (!device) {
      return response.notFound({ message: 'Dispositivo não encontrado' })
    }

    const schema = vine.object({
      enableCpuMonitor: vine.boolean().optional(),
      enableMemoryMonitor: vine.boolean().optional(),
      monitoredIfIndexes: vine.array(vine.number()).optional(),
    })

    const payload = await vine.validate({
      schema,
      data: request.all(),
    })

    // Garante que o dispositivo tem SNMP habilitado
    device.snmpEnabled = true
    device.isMonitored = true
    await device.save()

    const config = {
      host: device.ipAddress || device.name,
      version: (device.snmpVersion || 'v2c') as 'v1' | 'v2c' | 'v3',
      community: device.snmpCommunity || 'public',
      port: 161,
    }

    // 1. Escaneia para obter a lista completa de interfaces
    const scanResult = await this.snmpService.scanDevice(device, config)

    // 2. Persiste e atualiza as interfaces e cria monitores correspondentes
    const selectedIfIndexes = payload.monitoredIfIndexes || []
    for (const scanIface of scanResult.interfaces) {
      let iface = await DeviceInterface.query()
        .where('deviceId', device.id)
        .where('snmpIndex', scanIface.ifIndex)
        .first()

      const isSelected = selectedIfIndexes.includes(scanIface.ifIndex)

      if (!iface) {
        iface = new DeviceInterface()
        iface.deviceId = device.id
        iface.snmpIndex = scanIface.ifIndex
      }

      iface.name = scanIface.ifName
      iface.description = scanIface.ifDescr || null
      iface.macAddress = scanIface.macAddress || null
      iface.speed = scanIface.ifSpeed || null
      iface.adminStatus = isSelected ? 'up' : 'down'
      iface.operStatus = scanIface.ifOperStatus
      await iface.save()

      // Trata criação do monitor da interface
      const monitorName = `Interface ${scanIface.ifName}`
      let ifaceMonitor = await Monitor.query()
        .where('deviceId', device.id)
        .where('name', monitorName)
        .first()

      if (isSelected) {
        if (!ifaceMonitor) {
          ifaceMonitor = new Monitor()
          ifaceMonitor.deviceId = device.id
          ifaceMonitor.type = 'snmp'
          ifaceMonitor.name = monitorName
          ifaceMonitor.configuration = {
            host: device.ipAddress || device.name,
            ifIndex: scanIface.ifIndex,
            ifName: scanIface.ifName,
          }
          ifaceMonitor.intervalSeconds = 60
          ifaceMonitor.timeoutSeconds = 5
          ifaceMonitor.retryCount = 3
        }
        ifaceMonitor.enabled = true
        ifaceMonitor.status = scanIface.ifOperStatus === 'up' ? 'up' : 'down'
        await ifaceMonitor.save()
      } else {
        if (ifaceMonitor) {
          ifaceMonitor.enabled = false
          await ifaceMonitor.save()
        }
        // Remove métricas acumuladas de interfaces não selecionadas para não poluir relatórios
        await Metric.query().where('interfaceId', iface.id).delete()
      }
    }

    // 3. Trata monitor de CPU
    if (payload.enableCpuMonitor !== undefined) {
      let cpuMonitor = await Monitor.query()
        .where('deviceId', device.id)
        .whereILike('name', '%cpu%')
        .first()

      if (payload.enableCpuMonitor) {
        if (!cpuMonitor) {
          cpuMonitor = new Monitor()
          cpuMonitor.deviceId = device.id
          cpuMonitor.type = 'snmp'
          cpuMonitor.name = 'Monitor de Uso de CPU'
          cpuMonitor.configuration = { host: device.ipAddress || device.name, metric: 'cpu_usage' }
          cpuMonitor.intervalSeconds = 60
          cpuMonitor.timeoutSeconds = 5
          cpuMonitor.retryCount = 3
        }
        cpuMonitor.enabled = true
        cpuMonitor.status = 'up'
        await cpuMonitor.save()
      } else {
        if (cpuMonitor) {
          cpuMonitor.enabled = false
          await cpuMonitor.save()
        }
        await Metric.query()
          .where('deviceId', device.id)
          .whereIn('name', ['cpu_usage', 'cpu_load_1min'])
          .delete()
      }
    }

    // 4. Trata monitor de Memória
    if (payload.enableMemoryMonitor !== undefined) {
      let memMonitor = await Monitor.query()
        .where('deviceId', device.id)
        .whereILike('name', '%mem%')
        .first()

      if (payload.enableMemoryMonitor) {
        if (!memMonitor) {
          memMonitor = new Monitor()
          memMonitor.deviceId = device.id
          memMonitor.type = 'snmp'
          memMonitor.name = 'Monitor de Uso de Memória'
          memMonitor.configuration = {
            host: device.ipAddress || device.name,
            metric: 'memory_usage',
          }
          memMonitor.intervalSeconds = 60
          memMonitor.timeoutSeconds = 5
          memMonitor.retryCount = 3
        }
        memMonitor.enabled = true
        memMonitor.status = 'up'
        await memMonitor.save()
      } else {
        if (memMonitor) {
          memMonitor.enabled = false
          await memMonitor.save()
        }
        await Metric.query().where('deviceId', device.id).where('name', 'memory_usage').delete()
      }
    }

    // 5. Executa poll inicial imediatamente para atualizar métricas e status dos itens selecionados
    try {
      await this.snmpService.pollDevice(device, config)
    } catch {}

    return response.ok({
      message: 'Configurações de monitoramento atualizadas com sucesso',
    })
  }

  /**
   * GET /api/devices/:id/interfaces
   * Retorna a lista de interfaces de um dispositivo com métricas de tráfego.
   */
  async interfaces({ params, response }: HttpContext) {
    const device = await Device.find(params.id)
    if (!device) {
      return response.notFound({ message: 'Dispositivo não encontrado' })
    }

    const interfaces = await DeviceInterface.query()
      .where('deviceId', device.id)
      .preload('metrics', (q) => {
        q.orderBy('recordedAt', 'desc').limit(10)
      })

    const formatted = interfaces.map((intf) => {
      const json = intf.serialize()
      return {
        ...json,
        ifIndex: intf.snmpIndex,
        ifName: intf.name,
        ifDescr: intf.description,
        ifAdminStatus: intf.adminStatus,
        ifOperStatus: intf.operStatus,
        ifSpeed: intf.speed,
        ifType: intf.type,
      }
    })

    return response.ok(formatted)
  }
}
