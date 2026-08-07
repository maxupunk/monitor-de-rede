import fs from 'node:fs'
import app from '@adonisjs/core/services/app'
import env from '#start/env'
import { defineConfig } from '@adonisjs/lucid'

// Garante que o diretório tmp/ exista caso a conexão seja SQLite
const tmpDir = app.tmpPath()
if (!fs.existsSync(tmpDir)) {
  fs.mkdirSync(tmpDir, { recursive: true })
}

const dbConfig = defineConfig({
  /**
   * Default connection used for all queries.
   */
  connection: env.get('DB_CONNECTION') || 'sqlite',

  connections: {
    /**
     * SQLite connection.
     */
    sqlite: {
      client: 'better-sqlite3',

      connection: {
        /**
         * O arquivo é configurável porque a suíte funcional roda
         * `testUtils.db().truncate()`: apontando para o mesmo `db.sqlite3` do
         * desenvolvimento, um `node ace test` apaga os dados de trabalho.
         * O `.env.test` isola o banco dos testes.
         */
        filename: app.tmpPath(env.get('DB_FILENAME') || 'db.sqlite3'),
      },

      useNullAsDefault: true,

      migrations: {
        naturalSort: true,
        paths: ['database/migrations'],
      },

      schemaGeneration: {
        enabled: true,
        rulesPaths: ['./database/schema_rules.js'],
      },
    },

    /**
     * PostgreSQL connection.
     */
    pg: {
      client: 'pg',
      connection: {
        host: env.get('DB_HOST', '127.0.0.1'),
        port: env.get('DB_PORT', 5432),
        user: env.get('DB_USER', 'netmonitor'),
        password: env.get('DB_PASSWORD', 'secret'),
        database: env.get('DB_DATABASE', 'netmonitor'),
      },
      migrations: {
        naturalSort: true,
        paths: ['database/migrations'],
      },
      debug: app.inDev,
    },
  },
})

export default dbConfig
