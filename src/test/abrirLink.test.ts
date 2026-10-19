import { describe, expect, it } from "vitest";
import { workspaceReducer } from "../features/workspace/workspaceReducer";
import { emptyWorkspace } from "../services/workspaceService";
import type { LayoutNode, PaneNode, WorkspaceState } from "../types/workspace";

// Fork: link clicado abre aba de navegador à direita de quem o mostrou (`abrirLink.ts`).
function panes(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...panes(node.first), ...panes(node.second)];
}
const projeto = (s: WorkspaceState) => s.projects[0];

describe("abrir link", () => {
  it("divide o painel da conversa e abre o navegador à direita, com o foco de volta na conversa", () => {
    let state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/a" });
    const conversa = projeto(state).activePaneId;
    state = workspaceReducer(state, { type: "ABRIR_LINK", url: "https://claude.ai/x", title: "claude.ai", paneId: conversa });
    const raiz = projeto(state).layout;
    expect(raiz.type === "split" && raiz.direction).toBe("horizontal");
    if (raiz.type !== "split" || raiz.second.type !== "pane") throw new Error("layout");
    expect(raiz.first.id).toBe(conversa);
    expect(raiz.second.tabs).toEqual([expect.objectContaining({ kind: "web", resourceId: "https://claude.ai/x" })]);
    expect(projeto(state).activePaneId).toBe(conversa);
  });

  it("o segundo link entra no mesmo painel de navegador, como outra aba", () => {
    let state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/a" });
    state = workspaceReducer(state, { type: "ABRIR_LINK", url: "https://a.com", title: "a.com" });
    state = workspaceReducer(state, { type: "ABRIR_LINK", url: "https://b.com", title: "b.com" });
    const web = panes(projeto(state).layout).filter((p) => p.tabs.some((t) => t.kind === "web"));
    expect(web).toHaveLength(1);
    expect(web[0].tabs.map((t) => t.resourceId)).toEqual(["https://a.com", "https://b.com"]);
  });
});
