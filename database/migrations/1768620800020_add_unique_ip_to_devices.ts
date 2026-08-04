import { BaseSchema } from '@adonisjs/lucid/schema'

/**
 * Integridade do IPAM: impede dois dispositivos com o mesmo IP na mesma rede.
 * NULLs são distintos tanto no PostgreSQL quanto no SQLite, então dispositivos
 * sem IP definido não colidem entre si.
 */
export default class extends BaseSchema {
  protected tableName = 'devices'

  async up() {
    this.schema.alterTable(this.tableName, (table) => {
      table.unique(['network_id', 'ip_address'], { indexName: 'devices_network_ip_unique' })
    })
  }

  async down() {
    this.schema.alterTable(this.tableName, (table) => {
      table.dropUnique(['network_id', 'ip_address'], 'devices_network_ip_unique')
    })
  }
}
