import { describe, expect, it } from "vitest";
import { workspaceReducer } from "../features/workspace/workspaceReducer";
import { emptyWorkspace } from "../services/workspaceService";
import type { LayoutNode, PaneNode, WorkspaceState } from "../types/workspace";

// Conversa arrastada da lateral para o layout (`PLACE_SESSION`).
function panes(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...panes(node.first), ...panes(node.second)];
}
const layout = (state: WorkspaceState) => state.projects[0].layout;
const comConversa = (state: WorkspaceState, sessionId: string) =>
  panes(layout(state)).filter((pane) => pane.tabs.some((tab) => tab.resourceId === sessionId));

function projetoAberto(): { state: WorkspaceState; painel: string } {
  const state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/omni" });
  return { state, painel: state.projects[0].activePaneId };
}

describe("soltar conversa no layout", () => {
  it("no centro abre como aba no painel, em foco", () => {
    const { state, painel } = projetoAberto();
    const depois = workspaceReducer(state, { type: "PLACE_SESSION", sessionId: "s1", title: "Landing", targetPaneId: painel, zone: "center" });
    const [pane] = comConversa(depois, "s1");
    expect(pane.id).toBe(painel);
    expect(pane.tabs.find((tab) => tab.id === pane.activeTabId)?.resourceId).toBe("s1");
    expect(depois.projects[0].activePaneId).toBe(painel);
  });

  it("na direita divide lado a lado e na de baixo divide de novo, uma conversa por painel", () => {
    const { state, painel } = projetoAberto();
    let atual = workspaceReducer(state, { type: "PLACE_SESSION", sessionId: "s1", title: "A", targetPaneId: painel, zone: "right" });
    const raiz = layout(atual);
    expect(raiz.type === "split" && raiz.direction).toBe("horizontal");
    const direita = comConversa(atual, "s1")[0];
    expect(raiz.type === "split" && raiz.second.id).toBe(direita.id);
    expect(atual.projects[0].activePaneId).toBe(direita.id);

    atual = workspaceReducer(atual, { type: "PLACE_SESSION", sessionId: "s2", title: "B", targetPaneId: direita.id, zone: "bottom" });
    expect(panes(layout(atual))).toHaveLength(3);
    const baixo = comConversa(atual, "s2")[0];
    const r = layout(atual);
    const lado = r.type === "split" ? r.second : null;
    expect(lado?.type === "split" && lado.direction).toBe("vertical");
    expect(lado?.type === "split" && lado.second.id).toBe(baixo.id);

    atual = workspaceReducer(atual, { type: "PLACE_SESSION", sessionId: "s3", title: "C", targetPaneId: painel, zone: "top" });
    expect(panes(layout(atual))).toHaveLength(4);
  });

  it("conversa já aberta é movida, nunca duplicada", () => {
    const { state, painel } = projetoAberto();
    let atual = workspaceReducer(state, { type: "PLACE_SESSION", sessionId: "s1", title: "A", targetPaneId: painel, zone: "right" });
    atual = workspaceReducer(atual, { type: "PLACE_SESSION", sessionId: "s1", title: "A", targetPaneId: painel, zone: "bottom" });
    expect(comConversa(atual, "s1")).toHaveLength(1);
    // O painel da direita ficou vazio e saiu: sobram o original e o novo de baixo.
    expect(panes(layout(atual))).toHaveLength(2);
    const raiz = layout(atual);
    expect(raiz.type === "split" && raiz.direction).toBe("vertical");
  });

  it("borda do painel que só tem esta conversa não divide (ficaria vazio)", () => {
    const { state, painel } = projetoAberto();
    let atual = workspaceReducer(state, { type: "PLACE_SESSION", sessionId: "s1", title: "A", targetPaneId: painel, zone: "right" });
    const direita = comConversa(atual, "s1")[0];
    atual = workspaceReducer(atual, { type: "PLACE_SESSION", sessionId: "s1", title: "A", targetPaneId: direita.id, zone: "left" });
    expect(panes(layout(atual))).toHaveLength(2);
    expect(comConversa(atual, "s1")[0].id).toBe(direita.id);
  });

  it("painel que não existe não muda nada", () => {
    const { state } = projetoAberto();
    expect(workspaceReducer(state, { type: "PLACE_SESSION", sessionId: "s1", title: "A", targetPaneId: "nao-existe", zone: "center" })).toBe(state);
  });
});
