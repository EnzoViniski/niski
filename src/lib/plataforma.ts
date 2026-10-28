/** macOS (WKWebView): a janela usa os semáforos nativos (`tauri.macos.conf.json`), então a barra
 *  própria esconde os botões de janela e abre espaço à esquerda. */
export const NO_MAC = typeof navigator !== "undefined" && /Macintosh|Mac OS X/.test(navigator.userAgent);

// Modificador único para os atalhos do app; Ctrl da PTY continua independente.
type Modificadores = Pick<KeyboardEvent, "ctrlKey" | "metaKey">;
export function teclaModificadora(evento: Modificadores, noMac = NO_MAC): boolean {
  return noMac ? evento.metaKey && !evento.ctrlKey : evento.ctrlKey && !evento.metaKey;
}

/** Preferências antigas com Ctrl passam a usar Cmd no Mac, sem migrar o arquivo de estado. */
export function comboDaPlataforma(combo: string, noMac = NO_MAC): string {
  const adaptado = noMac ? combo.replace(/^Ctrl(?=\+|$)/, "Meta") : combo;
  // A gravação antiga colocava Meta depois de Shift. Normaliza também essas preferências.
  const partes = adaptado.split("+");
  const modificadores = ["Ctrl", "Meta", "Alt", "Shift"];
  return [...modificadores.filter((tecla) => partes.includes(tecla)), ...partes.filter((tecla) => !modificadores.includes(tecla))].join("+");
}

export function rotuloDoAtalho(combo: string, noMac = NO_MAC): string {
  return comboDaPlataforma(combo, noMac).replace(/Meta/g, noMac ? "⌘" : "Meta");
}
