/** Fork: o "há quanto tempo" de todas as telas num lugar só (antes eram cinco cópias).
 *
 *  - `curto`: "agora", "5 min", "3 h", "2 d" — cabe na segunda linha de uma lista.
 *  - `longo`: "agora", "há 5 min", "há 3 h", "há 2 d" — numa frase ("consultado há 5 min"). */
export function haQuantoTempo(ms: number, agora: number, estilo: "curto" | "longo" = "curto"): string {
  const minutos = Math.floor(Math.max(0, agora - ms) / 60_000);
  if (minutos < 1) return "agora";
  const ha = estilo === "longo" ? "há " : "";
  if (minutos < 60) return `${ha}${minutos} min`;
  if (minutos < 24 * 60) return `${ha}${Math.floor(minutos / 60)} h`;
  return `${ha}${Math.floor(minutos / (24 * 60))} d`;
}

/** Quanto faz desde a última atividade da conversa, no estilo curto; `null` sem data. */
export function haQuanto(ms: number | undefined, agora = Date.now()): string | null {
  if (!ms) return null;
  return haQuantoTempo(ms, agora);
}
