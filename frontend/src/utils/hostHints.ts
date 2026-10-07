/** O endereço aponta para a própria máquina (`localhost`, `127.x`, `::1`)? */
export function isLoopbackHost(host: string): boolean {
  const value = host
    .trim()
    .replace(/^\[|\]$/g, '')
    .toLowerCase()
  return value === 'localhost' || value === '::1' || /^127\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(value)
}

/**
 * Dica para o campo "Servidor": quem testa o endereço é o NetMonitor, não o
 * navegador. Com o NetMonitor no Docker, `127.0.0.1` é o próprio container.
 */
export function loopbackFieldHint(host: string): { hint?: string; persistentHint?: boolean } {
  return isLoopbackHost(host)
    ? {
        hint: 'É o servidor do NetMonitor, não o seu computador. Com o NetMonitor no Docker, use host.docker.internal para alcançar um serviço desta máquina.',
        persistentHint: true,
      }
    : {}
}
