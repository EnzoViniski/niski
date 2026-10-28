import type { LayoutNode, PaneNode, WorkspaceAction, WorkspaceState } from "../../types/workspace";

/**
 * Conversa arrastada da lateral para um painel do layout (`PLACE_SESSION`).
 *
 * - Centro: abre como aba naquele painel.
 * - Borda: divide o painel ao meio (esquerda/direita lado a lado, cima/baixo um sobre o outro) e a
 *   conversa fica no painel novo — como soltar uma aba ali. Cada nova conversa solta divide de novo.
 * - Conversa já aberta em algum painel do projeto é **movida**, nunca duplicada: duas abas na mesma
 *   sessão disputariam o mesmo terminal.
 * - Pode vir de **qualquer projeto** da lateral, não só do que está na tela. Se ela estava aberta no
 *   layout de outro projeto, sai de lá (`soltarDosOutrosProjetos`); a aba mostra de qual projeto é
 *   (`projetosDasAbas.tsx`).
 *
 * Feita com as ações que já existem (`CREATE_TAB`, `MOVE_TAB`, `SPLIT_WITH_TAB`), para herdar as
 * regras delas (painel que esvazia some, foco vai para o painel novo…).
 */
type PlaceSession = Extract<WorkspaceAction, { type: "PLACE_SESSION" }>;
type Reducer = (state: WorkspaceState, action: WorkspaceAction) => WorkspaceState;

function panes(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...panes(node.first), ...panes(node.second)];
}

function abaDaSessao(state: WorkspaceState, sessionId: string): { paneId: string; tabId: string } | null {
  const project = state.projects.find((item) => item.id === state.activeProjectId);
  if (!project) return null;
  for (const pane of panes(project.layout)) {
    const tab = pane.tabs.find((item) => item.resourceId === sessionId);
    if (tab) return { paneId: pane.id, tabId: tab.id };
  }
  return null;
}

function layoutAtivo(state: WorkspaceState): LayoutNode | undefined {
  return state.projects.find((item) => item.id === state.activeProjectId)?.layout;
}

/** Fecha (sem encerrar a sessão) as abas desta conversa nos layouts dos outros projetos. Usa o
 *  `CLOSE_TABS_BY_RESOURCE_PREFIX`, que só age no projeto ativo — por isso troca o ativo e devolve. */
function soltarDosOutrosProjetos(state: WorkspaceState, sessionId: string, reducer: Reducer): WorkspaceState {
  let atual = state;
  for (const outro of state.projects) {
    if (outro.id === state.activeProjectId) continue;
    if (!panes(outro.layout).some((pane) => pane.tabs.some((tab) => tab.resourceId === sessionId))) continue;
    atual = reducer({ ...atual, activeProjectId: outro.id }, { type: "CLOSE_TABS_BY_RESOURCE_PREFIX", prefix: sessionId });
  }
  return atual === state ? state : { ...atual, activeProjectId: state.activeProjectId };
}

export function colocarConversa(state: WorkspaceState, action: PlaceSession, reducer: Reducer): WorkspaceState {
  const project = state.projects.find((item) => item.id === state.activeProjectId);
  if (!project || !panes(project.layout).some((pane) => pane.id === action.targetPaneId)) return state;

  let atual = soltarDosOutrosProjetos(state, action.sessionId, reducer);
  let origem = abaDaSessao(atual, action.sessionId);
  if (!origem) {
    // Ainda não aberta: nasce como aba no painel alvo, e de lá vai para onde foi solta.
    atual = reducer(atual, { type: "CREATE_TAB", paneId: action.targetPaneId, kind: "agent", title: action.title, resourceId: action.sessionId });
    origem = abaDaSessao(atual, action.sessionId);
    if (!origem) return state;
  }

  if (action.zone === "center") {
    if (origem.paneId !== action.targetPaneId) {
      atual = reducer(atual, { type: "MOVE_TAB", sourcePaneId: origem.paneId, tabId: origem.tabId, targetPaneId: action.targetPaneId });
      origem = abaDaSessao(atual, action.sessionId) ?? origem;
    }
    atual = reducer(atual, { type: "SELECT_TAB", paneId: origem.paneId, tabId: origem.tabId });
    return reducer(atual, { type: "FOCUS_PANE", paneId: origem.paneId });
  }

  const dividido = reducer(atual, {
    type: "SPLIT_WITH_TAB",
    sourcePaneId: origem.paneId,
    tabId: origem.tabId,
    targetPaneId: action.targetPaneId,
    direction: action.zone === "left" || action.zone === "right" ? "horizontal" : "vertical",
    position: action.zone === "left" || action.zone === "top" ? "before" : "after",
  });
  // Painel que só tinha esta aba não se divide (ficaria vazio): ela fica nele, em foco. O reducer
  // devolve estado novo mesmo recusando (`updatedAt`), então quem diz se dividiu é o layout.
  if (layoutAtivo(dividido) !== layoutAtivo(atual)) return dividido;
  atual = reducer(atual, { type: "SELECT_TAB", paneId: origem.paneId, tabId: origem.tabId });
  return reducer(atual, { type: "FOCUS_PANE", paneId: origem.paneId });
}
