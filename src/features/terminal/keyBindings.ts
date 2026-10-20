// Fork: Cmd copia/cola no Mac; Ctrl+C sempre chega à PTY como SIGINT.
import { NO_MAC, teclaModificadora } from "../../lib/plataforma";
/** Atalhos que o xterm.js não entrega do jeito que este app precisa.
 *
 *  Armadilha central: `attachCustomKeyEventHandler` devolvendo `false` só diz "xterm, não processa
 *  essa tecla" — **não** cancela o evento. A ação padrão do browser continua acontecendo. Quem
 *  quiser realmente substituir a tecla precisa chamar `preventDefault()` na mão; quem quiser a
 *  ação padrão (o paste nativo) precisa justamente NÃO chamar. Errar isso foi o que fez o Ctrl+V
 *  colar duas vezes e o Shift+Enter enviar a mensagem.
 */
export interface TerminalKeyContext {
  hasSelection: () => boolean;
  copySelection: () => void;
  /** Escreve direto na PTY da sessão (ignora se não houver sessão viva). */
  write: (data: string) => void;
}

/** `true` = deixa o xterm tratar a tecla. `false` = xterm não trata. */
export function handleTerminalKey(event: KeyboardEvent, ctx: TerminalKeyContext): boolean {
  if (event.type !== "keydown") return true;

  // Fork: modificador da plataforma (Cmd no Mac, Ctrl fora). No Mac o Ctrl+C nem entra aqui e
  // segue para o xterm como SIGINT; Cmd+C copia. Fora do Mac, Ctrl+C só copia com seleção.
  // Sem `preventDefault` no V: a ação padrão (paste nativo) cai no textarea do xterm, que cola uma
  // vez só e respeita bracketed paste (ler o clipboard e escrever na PTY colava em dobro).
  if (teclaModificadora(event) && !event.shiftKey && !event.altKey) {
    const key = event.key.toLowerCase();
    if (key === "v") return false;
    if (key === "c" && (NO_MAC || ctx.hasSelection())) {
      if (ctx.hasSelection()) ctx.copySelection();
      event.preventDefault();
      return false;
    }
  }

  // xterm.js real só olha `altKey` pro Enter (Alt+Enter manda ESC+CR) — Shift é ignorado, então
  // Shift+Enter saía idêntico a Enter puro (\r) e a CLI (Claude Code etc.) nunca via "quebra de
  // linha sem enviar". A sequência que essas CLIs (Ink) leem é ESC+CR (meta+Enter) — a mesma que
  // o `/terminal-setup` do Claude Code instala no VS Code e no iTerm2. CSI-u (ESC[13;2u) só vale
  // se a CLI ligar o protocolo kitty/fixterms; o Ink não liga, então aquilo chegava como lixo.
  if (event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey && event.key === "Enter") {
    // Aqui o `preventDefault` é obrigatório: sem ele o browser ainda insere o "\n" no textarea do
    // xterm, o xterm manda esse "\n" pra PTY e a CLI entende como "enviar" — a mensagem ia embora
    // mesmo depois de a sequência certa ter sido escrita.
    event.preventDefault();
    ctx.write("\x1b\r");
    return false;
  }

  return true;
}
