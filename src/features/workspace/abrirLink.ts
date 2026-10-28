import type { LayoutNode, PaneNode, WorkspaceAction, WorkspaceState } from "../../types/workspace";

/**
 * Link clicado no app (`ABRIR_LINK`) abre numa aba de navegador do layout, nunca na janela.
 *
 * - Já existe painel com aba de navegador: a página nova entra nele, como outra aba.
 * - Senão, o painel de onde veio o clique (ou o ativo) se divide e o navegador fica **à direita**
 *   dele — ao lado do agente com quem se está conversando, como nos apps do Claude e do Codex.
 * - O foco volta para o painel de origem: quem estava escrevendo continua escrevendo.
 * - É uma aba comum: arrasta, divide e fecha como qualquer outra.
 */
type AbrirLink = Extract<WorkspaceAction, { type: "ABRIR_LINK" }>;
type Reducer = (state: WorkspaceState, action: WorkspaceAction) => WorkspaceState;

function panes(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...panes(node.first), ...panes(node.second)];
}

export function abrirLink(state: WorkspaceState, action: AbrirLink, reducer: Reducer): WorkspaceState {
  const project = state.projects.find((item) => item.id === state.activeProjectId);
  if (!project) return state;
  const todos = panes(project.layout);
  const origem = todos.find((pane) => pane.id === action.paneId)?.id ?? project.activePaneId;

  const comNavegador = todos.find((pane) => pane.tabs.some((tab) => tab.kind === "web"));
  if (comNavegador) {
    const depois = reducer(state, { type: "CREATE_TAB", paneId: comNavegador.id, kind: "web", title: action.title, resourceId: action.url });
    return reducer(depois, { type: "FOCUS_PANE", paneId: origem });
  }

  let atual = reducer(state, { type: "CREATE_TAB", paneId: origem, kind: "web", title: action.title, resourceId: action.url });
  const pane = panes(atual.projects.find((item) => item.id === atual.activeProjectId)!.layout).find((item) => item.id === origem);
  const aba = pane?.tabs[pane.tabs.length - 1];
  if (!pane || !aba) return atual;
  // Painel que só tinha o navegador (estava vazio) não se divide: ele mesmo vira o navegador.
  if (pane.tabs.length > 1) {
    atual = reducer(atual, { type: "SPLIT_WITH_TAB", sourcePaneId: origem, tabId: aba.id, targetPaneId: origem, direction: "horizontal", position: "after" });
    // Página ao lado do chat fica um pouco mais estreita que ele.
    const raiz = atual.projects.find((item) => item.id === atual.activeProjectId)!.layout;
    const split = acharSplit(raiz, origem);
    if (split) atual = reducer(atual, { type: "RESIZE_SPLIT", splitId: split, ratio: 0.55 });
  }
  return reducer(atual, { type: "FOCUS_PANE", paneId: origem });
}

/** Split recém-criado cujo primeiro filho é o painel de origem. */
function acharSplit(node: LayoutNode, paneId: string): string | null {
  if (node.type === "pane") return null;
  if (node.first.type === "pane" && node.first.id === paneId && node.direction === "horizontal") return node.id;
  return acharSplit(node.first, paneId) ?? acharSplit(node.second, paneId);
}

/** Título curto da aba: o domínio, sem `www.`. */
export function tituloDoLink(url: string): string {
  try { return new URL(url).hostname.replace(/^www\./, "") || url; } catch { return url; }
}
