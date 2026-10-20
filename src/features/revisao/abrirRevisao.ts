import type { LayoutNode, PaneNode, WorkspaceAction, WorkspaceState } from "../../types/workspace";

const paines = (n: LayoutNode): PaneNode[] => n.type === "pane" ? [n] : [...paines(n.first), ...paines(n.second)];

/** Revisão acompanha a sessão, inclusive quando sua aba pertence a outro projeto. */
export function abrirRevisao(state: WorkspaceState, action: Extract<WorkspaceAction, { type: "ABRIR_REVISAO" }>, reducer: (s: WorkspaceState, a: WorkspaceAction) => WorkspaceState): WorkspaceState {
  const projeto = state.projects.find(p => p.id === state.activeProjectId);
  if (!projeto) return state;
  for (const pane of paines(projeto.layout)) {
    const aba = pane.tabs.find(t => t.kind === "revisao" && t.resourceId === action.sessionId);
    if (aba) return reducer(state, { type: "SELECT_TAB", paneId: pane.id, tabId: aba.id });
  }
  let atual = reducer(state, { type: "CREATE_TAB", paneId: action.paneId, kind: "revisao", title: "Revisão", resourceId: action.sessionId, projectId: action.projectId });
  const layout = atual.projects.find(p => p.id === atual.activeProjectId)?.layout;
  const pane = layout && paines(layout).find(p => p.id === action.paneId);
  const aba = pane?.tabs[pane.tabs.length - 1];
  if (pane && aba && pane.tabs.length > 1) atual = reducer(atual, { type: "SPLIT_WITH_TAB", sourcePaneId: pane.id, targetPaneId: pane.id, tabId: aba.id, direction: "horizontal", position: "after" });
  return atual;
}
