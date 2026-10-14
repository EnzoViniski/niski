/** Uso de conta (janela de 5 h e semanal) — o mesmo cálculo no rodapé do app e na web. Sem Tauri:
 *  a web importa daqui. */
export interface UsageWindow { used_percent: number; resets_at: number | null; reset_label: string | null }

/**
 * Faixas de consumo pedidas: verde até 60%, amarelo até 85%, vermelho de 86% em diante.
 * O corte é por valor, não por arredondamento — 85,4% já é vermelho.
 */
export function usageTone(used: number): { bar: string; text: string } {
  if (used <= 60) return { bar: "bg-success", text: "text-success" };
  if (used <= 85) return { bar: "bg-warning", text: "text-warning" };
  return { bar: "bg-danger", text: "text-danger" };
}

/** Hoje: só a hora ("11:40"). Outro dia: "17/09 14:00". */
function shortDate(ms: number) {
  const date = new Date(ms);
  const time = date.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
  return date.toDateString() === new Date().toDateString()
    ? time
    : `${date.toLocaleDateString("pt-BR", { day: "2-digit", month: "2-digit" })} ${time}`;
}

/** Reset curto: data do Codex formatada, ou o texto da TUI sem "Resets" e sem o fuso. */
export const resetShort = (window: UsageWindow | null) =>
  !window ? null
    : window.resets_at ? shortDate(window.resets_at * 1000)
    : window.reset_label?.replace(/^Resets\s+/, "").replace(/\s*\(.*\)\s*$/, "") ?? null;
