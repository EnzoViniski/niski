import { useEffect, useRef } from "react";
import type { LayoutNode, PaneNode, WorkspaceAction, WorkspaceState } from "../../types/workspace";

/**
 * Abrir o fluxo do orquestrador e, a partir dele, a conversa de um nó.
 *
 * Quem pede (a faixa da conversa, a lateral, o nó do fluxo) não tem o `dispatch` do layout: manda um
 * evento da janela, e `useAberturasDoOrquestrador` (no App) traduz em ação. O fluxo abre numa aba nova
 * à direita do painel de origem (`ABRIR_FLUXO`); se já está aberto, só ganha o foco.
 */
export const EVENTO_ORQUESTRACAO = "medza-orquestracao";

type Pedido =
  | { tipo: "abrir-fluxo"; orquestrador: string; titulo: string; paneId?: string }
  | { tipo: "abrir-conversa"; sessionId: string; titulo: string; paneId?: string };

export function pedirAbrirFluxo(orquestrador: string, titulo: string, paneId?: string) {
  window.dispatchEvent(new CustomEvent<Pedido>(EVENTO_ORQUESTRACAO, { detail: { tipo: "abrir-fluxo", orquestrador, titulo, paneId } }));
}

export function pedirAbrirConversa(sessionId: string, titulo: string, paneId?: string) {
  window.dispatchEvent(new CustomEvent<Pedido>(EVENTO_ORQUESTRACAO, { detail: { tipo: "abrir-conversa", sessionId, titulo, paneId } }));
}

type AbrirFluxo = Extract<WorkspaceAction, { type: "ABRIR_FLUXO" }>;
type Reducer = (state: WorkspaceState, action: WorkspaceAction) => WorkspaceState;

function panes(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...panes(node.first), ...panes(node.second)];
}

/** Painel e aba onde um recurso (sessão, fluxo) já está aberto no projeto ativo. */
export function ondeEsta(state: WorkspaceState, kind: string, resourceId: string): { paneId: string; tabId: string } | null {
  const project = state.projects.find((item) => item.id === state.activeProjectId);
  if (!project) return null;
  for (const pane of panes(project.layout)) {
    const tab = pane.tabs.find((item) => item.kind === kind && item.resourceId === resourceId);
    if (tab) return { paneId: pane.id, tabId: tab.id };
  }
  return null;
}

export function abrirFluxo(state: WorkspaceState, action: AbrirFluxo, reducer: Reducer): WorkspaceState {
  const project = state.projects.find((item) => item.id === state.activeProjectId);
  if (!project) return state;
  const aberto = ondeEsta(state, "orquestracao", action.orquestrador);
  if (aberto) return reducer(state, { type: "SELECT_TAB", ...aberto });
  const origem = panes(project.layout).find((pane) => pane.id === action.paneId)?.id ?? project.activePaneId;
  let atual = reducer(state, { type: "CREATE_TAB", paneId: origem, kind: "orquestracao", title: action.title, resourceId: action.orquestrador });
  const pane = panes(atual.projects.find((item) => item.id === atual.activeProjectId)!.layout).find((item) => item.id === origem);
  const aba = pane?.tabs[pane.tabs.length - 1];
  // Painel com outra coisa: o fluxo fica ao lado (à direita), a conversa continua à vista.
  if (pane && aba && pane.tabs.length > 1) {
    atual = reducer(atual, { type: "SPLIT_WITH_TAB", sourcePaneId: origem, tabId: aba.id, targetPaneId: origem, direction: "horizontal", position: "after" });
  }
  return atual;
}

/** Liga os pedidos às ações do layout. `aoAbrir` volta para a tela do workspace. */
export function useAberturasDoOrquestrador(workspace: WorkspaceState, dispatch: (action: WorkspaceAction) => void, aoAbrir: () => void) {
  const estado = useRef(workspace);
  estado.current = workspace;
  const aoAbrirRef = useRef(aoAbrir);
  aoAbrirRef.current = aoAbrir;
  useEffect(() => {
    const ouvir = (evento: Event) => {
      const pedido = (evento as CustomEvent<Pedido>).detail;
      if (pedido.tipo === "abrir-fluxo") {
        dispatch({ type: "ABRIR_FLUXO", orquestrador: pedido.orquestrador, title: pedido.titulo, paneId: pedido.paneId });
      } else if (ondeEsta(estado.current, "agent", pedido.sessionId) || ondeEsta(estado.current, "terminal", pedido.sessionId) || !pedido.paneId) {
        // Já aberta em algum painel: só foca (o ATTACH procura em todos).
        dispatch({ type: "ATTACH_TERMINAL", sessionId: pedido.sessionId, title: pedido.titulo });
      } else {
        // Ao lado do fluxo, para os dois ficarem à vista.
        dispatch({ type: "PLACE_SESSION", sessionId: pedido.sessionId, title: pedido.titulo, targetPaneId: pedido.paneId, zone: "right" });
      }
      aoAbrirRef.current();
    };
    window.addEventListener(EVENTO_ORQUESTRACAO, ouvir);
    return () => window.removeEventListener(EVENTO_ORQUESTRACAO, ouvir);
  }, [dispatch]);
}
