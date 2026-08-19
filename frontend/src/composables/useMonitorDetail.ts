import { ref } from 'vue'

/**
 * Como se abre o detalhe de um monitor — **uma regra, um lugar**.
 *
 * O produto lista monitores em três superfícies: a tabela compartilhada
 * (`MonitorsTable`, usada em `/monitors` e na aba do dispositivo), o widget
 * "Monitores de Rede" do painel e o ranking de alvos instáveis. Cada uma
 * tinha a sua própria noção de o que um clique faz — a tabela abria só pelo
 * nome, o widget também, e o ranking **navegava** para uma tela cheia,
 * tirando o operador do painel.
 *
 * Este composable é a resposta: quem lista monitor pega daqui o estado do
 * diálogo e a função de abrir, monta o `MonitorDetailDialog` e não decide
 * nada por conta própria. Não é um segundo componente de detalhe — a
 * `MonitorDetailView` continua sendo uma só, e a rota `/monitors/{id}` monta
 * o mesmo diálogo.
 *
 * O estado é por instância, e não global: dois diálogos abertos ao mesmo
 * tempo em telas diferentes nunca acontecem, e um estado de módulo faria a
 * tabela reabrir o monitor que o painel abriu.
 */
export function useMonitorDetail() {
  const detalheAberto = ref(false)
  const monitorEmDetalhe = ref<number | null>(null)

  /** Abre o detalhe do monitor informado. Aceita id ou o próprio monitor. */
  function abrirDetalhe(monitor: number | { id: number }): void {
    monitorEmDetalhe.value = typeof monitor === 'number' ? monitor : monitor.id
    detalheAberto.value = true
  }

  return { detalheAberto, monitorEmDetalhe, abrirDetalhe }
}
