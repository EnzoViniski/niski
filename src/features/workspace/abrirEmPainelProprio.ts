// Fork: clicar numa conversa da lateral abre ela num painel próprio, ao lado do painel ativo (pedido do
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

/** Fork: conversas visíveis agora — a aba em foco de cada painel. A lateral clareia essas linhas. */
export function sessoesNaTela(layout: LayoutNode | undefined): string[] {
  if (!layout) return [];
  return paineis(layout).flatMap((pane) => {
    const id = pane.tabs.find((tab) => tab.id === pane.activeTabId)?.resourceId;
    return id ? [id] : [];
  });
}
