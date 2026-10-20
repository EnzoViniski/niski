import { workspaceReducer, createProject } from "../features/workspace/workspaceReducer";
import type { PaneNode, WorkspaceState } from "../types/workspace";
import { loadWorkspace, saveWorkspace } from "../services/workspaceService";

it("abre ao lado do agente e não duplica revisão da mesma sessão", () => {
  const projeto = createProject("/projeto", "p");
  const origem = projeto.layout as PaneNode;
  let estado: WorkspaceState = { version: 1, id: "w", name: "Teste", projects: [projeto], activeProjectId: "p", updatedAt: "" };
  const acao = { type: "ABRIR_REVISAO" as const, sessionId: "sessao", paneId: origem.id, projectId: "outro-projeto" };
  estado = workspaceReducer(estado, acao);
  const layout = estado.projects[0].layout;
  expect(layout.type).toBe("split");
  if (layout.type !== "split" || layout.second.type !== "pane") throw new Error("Esperado split");
  expect(layout.second.tabs[0]).toMatchObject({ kind: "revisao", title: "Revisão", resourceId: "sessao", projectId: "outro-projeto" });
  const repetido = workspaceReducer(estado, acao);
  expect(repetido.projects[0].layout).toEqual(layout);
  expect(repetido.projects[0].activePaneId).toBe(layout.second.id);
  saveWorkspace(repetido);
  expect(loadWorkspace().projects[0].layout).toEqual(layout);
  localStorage.clear();
});
