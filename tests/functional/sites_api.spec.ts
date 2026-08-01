import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import Site from '#models/site'

test.group('Sites API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('GET /api/sites deve retornar lista de sites cadastrados', async ({ client, assert }) => {
    await Site.create({ name: 'Matriz SP', description: 'Escritório Central', active: true })

    const response = await client.get('/api/sites')

    response.assertStatus(200)
    assert.isArray(response.body())
    assert.lengthOf(response.body(), 1)
    assert.equal(response.body()[0].name, 'Matriz SP')
  })

  test('POST /api/sites deve criar um novo site com sucesso', async ({ client, assert }) => {
    const response = await client.post('/api/sites').json({
      name: 'Filial RJ',
      description: 'Unidade Comercial',
      location: 'Rio de Janeiro - RJ',
      active: true,
    })

    response.assertStatus(201)
    assert.exists(response.body().id)
    assert.equal(response.body().name, 'Filial RJ')

    const dbSite = await Site.find(response.body().id)
    assert.exists(dbSite)
    assert.equal(dbSite?.name, 'Filial RJ')
  })

  test('PUT /api/sites/:id deve atualizar os dados de um site', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Site Antigo', active: true })

    const response = await client.put(`/api/sites/${site.id}`).json({
      name: 'Site Atualizado',
      description: 'Descrição nova',
      active: false,
    })

    response.assertStatus(200)
    assert.equal(response.body().name, 'Site Atualizado')

    await site.refresh()
    assert.equal(site.name, 'Site Atualizado')
  })

  test('DELETE /api/sites/:id deve remover um site', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Site Deletar', active: true })

    const response = await client.delete(`/api/sites/${site.id}`)

    response.assertStatus(204) // 204 No Content

    const dbSite = await Site.find(site.id)
    assert.isNull(dbSite)
  })
})
