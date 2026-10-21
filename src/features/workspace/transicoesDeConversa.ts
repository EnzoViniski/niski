// Fork: abertura, troca e fechamento com snapshots nativos do WebKit; só há um terminal vivo.
// https://webkit.org/blog/15865/webkit-features-in-safari-18-0/
import { flushSync } from "react-dom";
import { motionOn } from "../../lib/motion";
import type { WorkspacesAction } from "../../types/workspace";
import "./transicoesDeConversa.css";

interface Transicao {
  ready: Promise<void>;
  finished: Promise<void>;
  skipTransition: () => void;
}
type DocumentoComTransicao = Document & { startViewTransition?: (atualizar: () => void) => Transicao };

const ACOES_VISUAIS = new Set([
  "SELECT_WORKSPACE", "SELECT_PROJECT", "SELECT_TAB", "FOCUS_SESSION", "FOCUS_OPEN_TAB",
  "CREATE_TAB", "NEW_CONVERSATION_OWN_PANE", "ATTACH_TERMINAL", "BIND_TAB_RESOURCE",
  "CLOSE_TAB", "CLOSE_PANE", "CLOSE_PROJECT", "CLOSE_WORKSPACE", "CLOSE_TABS_BY_RESOURCE_PREFIX",
  "PLACE_SESSION", "SPLIT_PANE", "SPLIT_WITH_TAB", "MOVE_TAB", "MOVE_PANE", "SWAP_PANE",
  "TOGGLE_MAXIMIZE", "APPLY_LAYOUT_PRESET", "ABRIR_LINK", "ABRIR_FLUXO", "ABRIR_REVISAO",
]);

/** Codificação injetiva: ids salvos nunca viram seletor CSS nem repetem nomes de outro painel. */
export function nomeDaTransicao(paneId: string): string {
  return `medza-painel-${Array.from(paneId).map((c) => c.codePointAt(0)!.toString(16)).join("-")}`;
}

function paineis(documento: Document): Map<string, string> {
  return new Map(Array.from(documento.querySelectorAll<HTMLElement>("[data-transicao-painel]"))
    .map((painel) => [nomeDaTransicao(painel.dataset.dropPane!), painel.dataset.transicaoPainel ?? ""]));
}

function regras(antes: Map<string, string>, depois: Map<string, string>, reduzida: boolean): string {
  const mudaram = [...new Set([...antes.keys(), ...depois.keys()])].filter((nome) => antes.get(nome) !== depois.get(nome));
  return mudaram.map((nome) => `
    ::view-transition-old(${nome}) { animation: ${reduzida ? "medza-conversa-apagar" : "medza-conversa-fechar"} ${reduzida ? 110 : 200}ms cubic-bezier(.4, 0, .7, .3) both; }
    ::view-transition-new(${nome}) { animation: ${reduzida ? "medza-conversa-revelar" : "medza-conversa-abrir"} ${reduzida ? 140 : 320}ms cubic-bezier(.2, .85, .2, 1) both; }
  `).join("\n");
}

/** Navega uma vez, na ordem recebida. Cliques durante a captura entram no mesmo commit; durante
 *  a animação, pulam para o destino atual e começam a próxima. Polls e redimensionar não animam. */
export function criarDespachoComTransicao(aplicar: (acao: WorkspacesAction) => void, documento: DocumentoComTransicao = document) {
  let capturando = false;
  let emVoo: Transicao | null = null;
  let fila: WorkspacesAction[] = [];

  return (acao: WorkspacesAction) => {
    if (capturando) { fila.push(acao); return; }
    if (!ACOES_VISUAIS.has(acao.type) || !motionOn() || documento.hidden || !documento.startViewTransition) {
      aplicar(acao); return;
    }
    emVoo?.skipTransition();
    const antes = paineis(documento);
    const estilo = documento.createElement("style");
    estilo.dataset.transicaoConversa = "";
    documento.head.appendChild(estilo);
    capturando = true;
    fila.push(acao);
    let aplicado = false;
    const atualizar = () => {
      if (aplicado) return;
      aplicado = true;
      capturando = false;
      const lote = fila; fila = [];
      flushSync(() => { for (const proxima of lote) aplicar(proxima); });
      estilo.textContent = regras(antes, paineis(documento), documento.documentElement.dataset.motion === "reduzidas");
    };
    try {
      const transicao = documento.startViewTransition(atualizar);
      emVoo = transicao;
      // Ocultar/minimizar a janela pode cancelar a captura. O commit continua sendo o da API.
      void transicao.ready.catch(() => undefined);
      void transicao.finished.catch(() => undefined).then(() => {
        estilo.remove();
        if (emVoo === transicao) emVoo = null;
      });
    } catch {
      try { atualizar(); } finally { estilo.remove(); }
    }
  };
}
