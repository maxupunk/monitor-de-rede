import { configApp } from '@adonisjs/eslint-config'

export default configApp({
  ignores: [
    // O frontend tem toolchain própria (frontend/eslint.config.js + npm run lint)
    'frontend/**',
    // Gerados por `node ace`: registry do Tuyau e schema do Lucid ("DO NOT EDIT")
    '.adonisjs/**',
    'database/schema.ts',
    'tmp/**',
  ],
})
