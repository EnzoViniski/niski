/** macOS (WKWebView): a janela usa os semáforos nativos (`tauri.macos.conf.json`), então a barra
 *  própria esconde os botões de janela e abre espaço à esquerda. */
export const NO_MAC = typeof navigator !== "undefined" && /Macintosh|Mac OS X/.test(navigator.userAgent);
