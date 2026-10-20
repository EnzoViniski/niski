// Fork (Onda 2.0): saiu do `MobileApp.tsx` sem mudança — usado pela conversa (rascunho) e pela navegação (filtro).

/** Guarda uma preferência só desta aba. Navegador sem armazenamento segue sem ela. */
export function lerSessao(chave: string): string | null {
  try { return sessionStorage.getItem(chave); } catch { return null; }
}
export function gravarSessao(chave: string, valor: string | null) {
  try { if (valor === null) sessionStorage.removeItem(chave); else sessionStorage.setItem(chave, valor); } catch { /* nada */ }
}
