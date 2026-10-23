import type { IBufferCellPosition, ILink, ILinkHandler, ILinkProvider, Terminal } from "@xterm/xterm";
import { abrirLinkNoApp } from "./registroDoNavegador";

/**
 * Fork: links no terminal abrem numa aba de navegador do app, como os da Conversa.
 *
 * - Link "de verdade" (OSC 8, o que o Claude Code e outras CLIs imprimem): o xterm, sem `linkHandler`,
 *   pedia `confirm()` e chamava `window.open()` — no app nenhum dos dois funciona, e o clique não fazia
 *   nada. Agora abre com um clique.
 * - Endereço escrito na saída (o Codex, o `npx hyperframes preview`…): vira link com ⌘ + clique (Ctrl fora
 *   do Mac), para clicar no terminal para focar não abrir página sem querer.
 * Só http e https.
 */

const URL_NA_LINHA = /https?:\/\/[^\s"'<>`│┃─]+/g;
/** Pontuação colada no fim que é da frase, não do endereço. */
const FIM_DA_FRASE = /[.,;:!?'"»]+$/;

/** Endereços http(s) de uma linha do terminal, com a coluna (0) onde cada um começa. */
export function urlsDaLinha(texto: string): { inicio: number; url: string }[] {
  const achadas: { inicio: number; url: string }[] = [];
  for (const casamento of texto.matchAll(URL_NA_LINHA)) {
    let url = casamento[0].replace(FIM_DA_FRASE, "");
    // ")" no fim só fica se abriu dentro do endereço ("…/wiki/Foo_(bar)").
    while (/[)\]}]$/.test(url) && (url.match(/[([{]/g)?.length ?? 0) < (url.match(/[)\]}]/g)?.length ?? 0)) url = url.slice(0, -1).replace(FIM_DA_FRASE, "");
    if (url.length > "https://".length) achadas.push({ inicio: casamento.index ?? 0, url });
  }
  return achadas;
}

const comTecla = (evento: MouseEvent) => evento.metaKey || evento.ctrlKey;

/** OSC 8: um clique abre (o `title` do elemento ensina o ⌘ para os endereços soltos). */
export function linkHandlerDoTerminal(elemento: () => HTMLElement | null): ILinkHandler {
  return {
    allowNonHttpProtocols: false,
    activate: (_evento, uri) => { abrirLinkNoApp(uri, elemento()); },
  };
}

/** Endereços soltos na saída viram link (⌘ + clique). Endereço quebrado em duas linhas fica de fora. */
export function provedorDeUrls(terminal: Terminal, elemento: () => HTMLElement | null): ILinkProvider {
  return {
    provideLinks(linha, devolver) {
      const texto = terminal.buffer.active.getLine(linha - 1)?.translateToString(true) ?? "";
      const links: ILink[] = urlsDaLinha(texto).map(({ inicio, url }) => {
        const start: IBufferCellPosition = { x: inicio + 1, y: linha };
        const end: IBufferCellPosition = { x: inicio + url.length, y: linha };
        return {
          range: { start, end },
          text: url,
          decorations: { pointerCursor: true, underline: true },
          activate: (evento, texto) => { if (comTecla(evento)) abrirLinkNoApp(texto, elemento()); },
          hover: () => { const el = elemento(); if (el) el.title = "⌘ + clique abre numa aba do app"; },
          leave: () => { const el = elemento(); if (el) el.removeAttribute("title"); },
        };
      });
      devolver(links.length ? links : undefined);
    },
  };
}
