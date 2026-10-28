// Clicar numa conversa da lateral abre ela num painel próprio, ao lado do painel ativo (pedido do
// Enzo, 21/10/2026), em vez de virar mais uma aba no painel que estava em foco.
//
// - Já está na tela (em qualquer painel/projeto): só ganha o foco, como antes (`FOCUS_SESSION`).
// - Painel ativo vazio (só o "Novo agente", sem conversa): ela entra ali, sem dividir à toa.
// - Já há `MAX_PAINEIS` painéis: vira aba no painel ativo, para a tela não picotar.
// - Senão: divide o painel ativo e ela fica no painel novo, à direita (`PLACE_SESSION` zona "right").
import type { LayoutNode, PaneNode, WorkspaceState } from "../../types/workspace";
import { collectResourceIds, workspaceReducer } from "./workspaceReducer";

export const MAX_PAINEIS = 4;

function paineis(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...paineis(node.first), ...paineis(node.second)];
}

export function abrirEmPainelProprio(workspace: WorkspaceState, projectId: string, sessionId: string, title: string): WorkspaceState {
  // Já aberta em algum layout deste workspace: o FOCUS de sempre (seleciona onde está e foca a aba).
  const ondeEsta = workspace.projects.find((project) => collectResourceIds(project.layout).includes(sessionId));
  const selecionado = workspaceReducer(workspace, { type: "SELECT_PROJECT", projectId: ondeEsta?.id ?? projectId });
  if (ondeEsta) return workspaceReducer(selecionado, { type: "ATTACH_TERMINAL", sessionId, title });

  const projeto = selecionado.projects.find((project) => project.id === selecionado.activeProjectId);
  if (!projeto) return selecionado;
  const todos = paineis(projeto.layout);
  const ativo = todos.find((pane) => pane.id === projeto.activePaneId) ?? todos[0];
  const vazio = !ativo || ativo.tabs.every((tab) => !tab.resourceId);
  if (!ativo || vazio || todos.length >= MAX_PAINEIS) {
    return workspaceReducer(selecionado, { type: "ATTACH_TERMINAL", sessionId, title });
  }
  return workspaceReducer(selecionado, { type: "PLACE_SESSION", sessionId, title, targetPaneId: ativo.id, zone: "right" });
}

/** Conversas visíveis agora — a aba em foco de cada painel. A lateral clareia essas linhas. */
export function sessoesNaTela(layout: LayoutNode | undefined): string[] {
  if (!layout) return [];
  return paineis(layout).flatMap((pane) => {
    const id = pane.tabs.find((tab) => tab.id === pane.activeTabId)?.resourceId;
    return id ? [id] : [];
  });
}

type Redutor = (state: WorkspaceState, action: import("../../types/workspace").WorkspaceAction) => WorkspaceState;

function mapearPaineis(node: LayoutNode, f: (pane: PaneNode) => PaneNode): LayoutNode {
  return node.type === "pane" ? f(node) : { ...node, first: mapearPaineis(node.first, f), second: mapearPaineis(node.second, f) };
}

/**
 * O "+" da pasta — conversa nova do projeto num painel próprio (mesma regra do clique numa
 * conversa), sem trocar o layout da direita. O seletor aguarda a escolha de agente, conta e modelo
 * antes de abrir qualquer CLI (`AgentLauncher`).
 */
export function novaConversaEmPainelProprio(state: WorkspaceState, projectId: string, reducer: Redutor): WorkspaceState {
  let atual = state.activeProjectId ? state : reducer(state, { type: "SELECT_PROJECT", projectId });
  const projeto = atual.projects.find((project) => project.id === atual.activeProjectId);
  if (!projeto) return state;
  const todos = paineis(projeto.layout);
  const ativo = todos.find((pane) => pane.id === projeto.activePaneId) ?? todos[0];
  if (!ativo) return state;
  const vazio = ativo.tabs.every((tab) => !tab.resourceId);
  let alvo = ativo.id;
  if (!vazio && todos.length < MAX_PAINEIS) {
    atual = reducer(atual, { type: "SPLIT_PANE", paneId: ativo.id, direction: "horizontal" });
    alvo = atual.projects.find((project) => project.id === atual.activeProjectId)!.activePaneId;
  }
  const deFora = projectId !== projeto.id ? projectId : undefined;
  atual = reducer(atual, { type: "CREATE_TAB", paneId: alvo, kind: "agent", title: "Novo agente", projectId: deFora });
  const novaAbaId = paineis(atual.projects.find((project) => project.id === atual.activeProjectId)!.layout)
    .find((pane) => pane.id === alvo)!.activeTabId;
  // O painel novo (ou vazio) não precisa da aba de boas-vindas/"Novo agente" parada que já tinha.
  return {
    ...atual,
    projects: atual.projects.map((project) => project.id !== atual.activeProjectId ? project : {
      ...project,
      layout: mapearPaineis(project.layout, (pane) => pane.id !== alvo ? pane : {
        ...pane, tabs: pane.tabs.filter((tab) => tab.id === novaAbaId || tab.resourceId || tab.kind !== "agent"),
      }),
    }),
  };
}

/** Os painéis do layout com as abas de conversa de cada um (para o "Na tela" da lateral). */
export function paineisNaTela(projeto: { id: string; layout: LayoutNode; activePaneId: string } | null | undefined,
  nomeDoProjeto: (id: string) => string | undefined) {
  if (!projeto) return [];
  return paineis(projeto.layout).map((pane) => ({
    paneId: pane.id,
    emFoco: pane.id === projeto.activePaneId,
    abas: pane.tabs.filter((tab) => tab.kind === "agent" && tab.resourceId).map((tab) => ({
      tabId: tab.id, titulo: tab.title, sessionId: tab.resourceId!, provider: tab.provider,
      projeto: tab.projectId && tab.projectId !== projeto.id ? nomeDoProjeto(tab.projectId) : undefined,
      ativa: tab.id === pane.activeTabId,
    })),
  }));
}
