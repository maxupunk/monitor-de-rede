import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useEventsStore = defineStore('events', () => {
  const events = ref<Array<Record<string, unknown>>>([])

  function addEvent(event: Record<string, unknown>) {
    events.value.unshift(event)
  }

  return { events, addEvent }
})
