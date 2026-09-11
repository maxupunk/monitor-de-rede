import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface SensorStateSpec {
  label: string
  color?: string
  icon?: string
}

export interface SnmpProfileSensor {
  key: string
  label: string
  oid: string
  unit?: string
  scale?: number
  category?: string
  dataType?: 'float' | 'integer' | 'boolean' | 'state'
  description?: string | null
  icon?: string
  color?: string
  states?: Record<string, string | SensorStateSpec>
}

export interface SnmpDeviceProfile {
  id: string
  name: string
  vendor: string
  category: string
  sysObjectIdPrefix?: string | null
  sysDescrPattern?: string | null
  sensors: SnmpProfileSensor[]
  isBuiltin?: boolean
}

export const useSnmpProfilesStore = defineStore('snmpProfiles', () => {
  const profiles = ref<SnmpDeviceProfile[]>([])
  const loading = ref(false)
  const saving = ref(false)
  const deleting = ref(false)
  const error = ref<string | null>(null)

  async function fetchProfiles(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      const data = await apiService.get<SnmpDeviceProfile[]>('/snmp/profiles')
      profiles.value = Array.isArray(data) ? data : []
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar perfis SNMP'
    } finally {
      loading.value = false
    }
  }

  async function saveProfile(profile: SnmpDeviceProfile): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      await apiService.post('/snmp/profiles', profile)
      await fetchProfiles()
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao salvar perfil SNMP'
      return false
    } finally {
      saving.value = false
    }
  }

  async function deleteProfile(profileId: string): Promise<boolean> {
    deleting.value = true
    error.value = null
    try {
      await apiService.delete(`/snmp/profiles/${encodeURIComponent(profileId)}`)
      await fetchProfiles()
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao excluir perfil SNMP'
      return false
    } finally {
      deleting.value = false
    }
  }

  return {
    profiles,
    loading,
    saving,
    deleting,
    error,
    fetchProfiles,
    saveProfile,
    deleteProfile,
  }
})
