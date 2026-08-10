/**
 * `catch (err: unknown)` não garante uma instância de `Error` — pode ser
 * qualquer valor lançado por uma lib de terceiros. Esta é a única conversão
 * usada no projeto para extrair uma mensagem exibível desse valor.
 */
export function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err)
}
