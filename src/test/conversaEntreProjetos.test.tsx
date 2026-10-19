import { act, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { workspaceReducer } from "../features/workspace/workspaceReducer";
import { workspacesReducer } from "../features/workspace/workspacesReducer";
import { PreviaDoLayout, preverLayout } from "../features/workspace/PreviaDoLayout";
import { montarProjetosDasAbas } from "../features/workspace/projetosDasAbas";
import { TAB_DRAG_EVENT, type TabDragState } from "../features/workspace/tabPointerDrag";
import { emptyWorkspace } from "../services/workspaceService";
import type { LayoutNode, PaneNode, WorkspaceState } from "../types/workspace";

// Fork: prévia do layout final, conversas de outro projeto no layout e a aba duplicada na mesma sessão.
function panes(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...panes(node.first), ...panes(node.second)];
}
const abasDa = (state: WorkspaceState, projeto: number, sessionId: string) =>
  panes(state.projects[projeto].layout).flatMap((pane) => pane.tabs.filter((tab) => tab.resourceId === sessionId));

function doisProjetos(): WorkspaceState {
  let state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/a" });
  state = workspaceReducer(state, { type: "ADD_PROJECT", path: "/dev/b" });
  return state; // ativo: b
}

describe("aba duplicada na mesma sessão", () => {
  it("abrir pela lateral uma conversa que está no outro painel foca a aba dela, sem criar outra", () => {
    let state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/a" });
    const painel = state.projects[0].activePaneId;
    state = workspaceReducer(state, { type: "PLACE_SESSION", sessionId: "s1", title: "Cima", targetPaneId: painel, zone: "top" });
    const cima = panes(state.projects[0].layout).find((pane) => pane.tabs.some((tab) => tab.resourceId === "s1"))!;
    state = workspaceReducer(state, { type: "FOCUS_PANE", paneId: painel });

    state = workspaceReducer(state, { type: "ATTACH_TERMINAL", sessionId: "s1", title: "Cima" });

    expect(abasDa(state, 0, "s1")).toHaveLength(1);
    expect(state.projects[0].activePaneId).toBe(cima.id);
  });

  it("conta as abas de cada sessão em todos os projetos (fechar uma cópia não encerra a outra)", () => {
    const state = doisProjetos();
    const comUma = workspaceReducer(state, { type: "CREATE_TAB", paneId: state.projects[1].activePaneId, kind: "agent", resourceId: "s1" });
    expect(montarProjetosDasAbas(comUma).abasDaSessao("s1")).toBe(1);
    const comDuas = workspaceReducer(comUma, { type: "CREATE_TAB", paneId: state.projects[1].activePaneId, kind: "agent", resourceId: "s1" });
    expect(montarProjetosDasAbas(comDuas).abasDaSessao("s1")).toBe(2);
  });
});

describe("conversa de outro projeto no layout", () => {
  it("é movida do layout do projeto dela para o que está na tela, sem duplicar", () => {
    let state = doisProjetos();
    state = workspaceReducer(state, { type: "SELECT_PROJECT", projectId: state.projects[0].id });
    state = workspaceReducer(state, { type: "CREATE_TAB", paneId: state.projects[0].activePaneId, kind: "agent", resourceId: "s1" });
    state = workspaceReducer(state, { type: "SELECT_PROJECT", projectId: state.projects[1].id });

    state = workspaceReducer(state, { type: "PLACE_SESSION", sessionId: "s1", title: "De A", targetPaneId: state.projects[1].activePaneId, zone: "right" });

    expect(abasDa(state, 0, "s1")).toHaveLength(0);
    expect(abasDa(state, 1, "s1")).toHaveLength(1);
    expect(state.activeProjectId).toBe(state.projects[1].id);
  });

  it("focar pela lateral leva ao projeto onde ela está aberta, não ao dela", () => {
    let state = doisProjetos();
    state = workspaceReducer(state, { type: "PLACE_SESSION", sessionId: "s1", title: "De A", targetPaneId: state.projects[1].activePaneId, zone: "center" });
    const colecao = { version: 1 as const, workspaces: [state], activeWorkspaceId: state.id };

    const depois = workspacesReducer(colecao, { type: "FOCUS_SESSION", workspaceId: state.id, projectId: state.projects[0].id, sessionId: "s1", title: "De A" }).workspaces[0];

    expect(depois.activeProjectId).toBe(state.projects[1].id);
    expect(abasDa(depois, 0, "s1")).toHaveLength(0);
    expect(abasDa(depois, 1, "s1")).toHaveLength(1);
  });
});

describe("prévia do layout final", () => {
  it("conversa da lateral na borda de baixo: o painel atual em cima e a nova embaixo, destacada", () => {
    const state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/a" });
    const project = state.projects[0];
    const previsto = preverLayout(project, { tab: { paneId: "lateral", tabId: "s1" }, target: { paneId: project.activePaneId, zone: "bottom" } }, "Nova");
    const raiz = previsto!.layout;
    expect(raiz.type === "split" && raiz.direction).toBe("vertical");
    if (raiz.type !== "split" || raiz.first.type !== "pane" || raiz.second.type !== "pane") throw new Error("layout inesperado");
    expect(raiz.first.id).toBe(project.activePaneId);
    expect(raiz.second.tabs.some(previsto!.destaque)).toBe(true);
    // A prévia não mexe no projeto de verdade.
    expect(project.layout.type).toBe("pane");
  });

  it("aba solta no centro do próprio painel não tem prévia", () => {
    const state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/a" });
    const project = state.projects[0];
    const pane = project.layout as PaneNode;
    expect(preverLayout(project, { tab: { paneId: pane.id, tabId: pane.tabs[0].id }, target: { paneId: pane.id, zone: "center" } }, "")).toBeNull();
  });

  it("aparece durante o arraste, acompanha a zona e some ao soltar", () => {
    let state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/a" });
    state = workspaceReducer(state, { type: "PLACE_SESSION", sessionId: "s0", title: "Antiga", targetPaneId: state.projects[0].activePaneId, zone: "center" });
    const project = state.projects[0];
    const avisar = (detail: TabDragState) => act(() => { window.dispatchEvent(new CustomEvent(TAB_DRAG_EVENT, { detail })); });
    render(<PreviaDoLayout project={project} />);
    expect(screen.queryByRole("status", { name: "Prévia do layout" })).not.toBeInTheDocument();

    avisar({ tab: { paneId: "lateral", tabId: "s1" }, target: { paneId: project.activePaneId, zone: "left" } });
    const previa = screen.getByRole("status", { name: "Prévia do layout" });
    expect(previa).toHaveTextContent("Vai abrir aqui");
    expect(previa).toHaveTextContent("Antiga");
    expect(previa.querySelector(".previa-split")).toHaveClass("flex-row");

    avisar({ tab: { paneId: "lateral", tabId: "s1" }, target: { paneId: project.activePaneId, zone: "top" } });
    expect(screen.getByRole("status", { name: "Prévia do layout" }).querySelector(".previa-split")).toHaveClass("flex-col");

    avisar(null);
    expect(screen.queryByRole("status", { name: "Prévia do layout" })).not.toBeInTheDocument();
  });
});

describe("aba nova num painel de outro projeto", () => {
  it("guarda o projeto dela quando não é o dono do layout, e não guarda quando é", () => {
    const state = doisProjetos();
    const painel = state.projects[1].activePaneId;
    const outro = workspaceReducer(state, { type: "CREATE_TAB", paneId: painel, kind: "agent", projectId: state.projects[0].id });
    const mesmo = workspaceReducer(state, { type: "CREATE_TAB", paneId: painel, kind: "agent", projectId: state.projects[1].id });
    const ultima = (s: WorkspaceState) => panes(s.projects[1].layout)[0].tabs.at(-1)!;
    expect(ultima(outro).projectId).toBe(state.projects[0].id);
    expect(ultima(mesmo).projectId).toBeUndefined();
  });
});
