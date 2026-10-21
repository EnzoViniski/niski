import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { workspaceReducer } from "../features/workspace/workspaceReducer";
import { abrirEmPainelProprio, paineisNaTela } from "../features/workspace/abrirEmPainelProprio";
import { emptyWorkspace } from "../services/workspaceService";
import { AgentLauncher } from "../features/terminal/AgentLauncher";
import { NaTela, deveMostrarNaTela } from "../features/projects/NaTela";
import type { LayoutNode, PaneNode, WorkspaceState } from "../types/workspace";

/*
 * Fork (21/10/2026): o "+" da pasta começa uma conversa nova num painel próprio, já com o último
 * agente/conta; Fork (0.9.2): agora aguarda a escolha. "Na tela" agrupa conversas no mesmo painel.
 */

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function paineis(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...paineis(node.first), ...paineis(node.second)];
}
const projeto = (state: WorkspaceState) => state.projects.find((p) => p.id === state.activeProjectId)!;

describe("+ da pasta", () => {
  it("painel vazio: a conversa nova entra nele com o seletor e sem a aba parada", () => {
    let state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/omni" });
    state = workspaceReducer(state, { type: "NEW_CONVERSATION_OWN_PANE", projectId: projeto(state).id });
    const [painel] = paineis(projeto(state).layout);
    expect(paineis(projeto(state).layout)).toHaveLength(1);
    expect(painel.tabs).toHaveLength(1);
    expect(painel.tabs[0]).toMatchObject({ kind: "agent" });
    expect(painel.tabs[0].iniciarDireto).not.toBe(true);
  });

  it("com conversa na tela: abre num painel novo à direita; de outra pasta, a aba guarda o projeto", () => {
    let state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/site" });
    const site = projeto(state).id;
    state = workspaceReducer(state, { type: "ADD_PROJECT", path: "/dev/omni" });
    const omni = projeto(state).id;
    state = abrirEmPainelProprio(state, omni, "s1", "Login");
    state = workspaceReducer(state, { type: "NEW_CONVERSATION_OWN_PANE", projectId: site });
    expect(state.activeProjectId).toBe(omni); // o layout da direita não troca
    const todos = paineis(projeto(state).layout);
    expect(todos).toHaveLength(2);
    const novo = todos.find((p) => p.id === projeto(state).activePaneId)!;
    expect(novo.tabs).toEqual([expect.objectContaining({ kind: "agent", projectId: site })]);
    expect(novo.tabs[0].iniciarDireto).not.toBe(true);
  });

  it("até o Novo agente salvo com a marca antiga aguarda o clique para abrir", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "agent_cli_statuses") return [{ id: "claude", label: "Claude", command: "claude", path: "x", available: true, authenticated: true }];
      if (command === "list_profiles") return [];
      if (command === "git_branches") return [];
      return null;
    });
    const onLaunch = vi.fn();
    render(<AgentLauncher projectName="p" projectPath="/p" onLaunch={onLaunch} iniciarDireto />);
    const abrir = await screen.findByRole("button", { name: "Abrir Claude" });
    expect(onLaunch).not.toHaveBeenCalled();
    expect(screen.getByRole("radiogroup", { name: "CLI do agente" })).toBeInTheDocument();
    fireEvent.click(abrir);
    await waitFor(() => expect(onLaunch).toHaveBeenCalledTimes(1));
    expect(onLaunch.mock.calls[0][0].agent.id).toBe("claude");
  });

  it("sem agente instalado, fica a tela de escolha de sempre", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "agent_cli_statuses") return [{ id: "claude", label: "Claude", command: "claude", path: null, available: false, authenticated: false }];
      if (command === "list_profiles") return [];
      return null;
    });
    const onLaunch = vi.fn();
    render(<AgentLauncher projectName="p" projectPath="/p" onLaunch={onLaunch} iniciarDireto />);
    await screen.findAllByText(/Claude/);
    await new Promise((r) => setTimeout(r, 30));
    expect(onLaunch).not.toHaveBeenCalled();
  });
});

describe("Na tela", () => {
  it("agrupa por painel as conversas juntas e clicar foca a aba", () => {
    let state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/omni" });
    const id = projeto(state).id;
    state = abrirEmPainelProprio(state, id, "s1", "Login");
    const [painel] = paineis(projeto(state).layout);
    state = workspaceReducer(state, { type: "CREATE_TAB", paneId: painel.id, kind: "agent", title: "App-server", resourceId: "s2" });
    state = abrirEmPainelProprio(state, id, "s3", "Testes");
    const grupos = paineisNaTela(projeto(state), () => undefined);
    expect(grupos.map((g) => g.abas.map((a) => a.sessionId))).toEqual([["s1", "s2"], ["s3"]]);
    expect(deveMostrarNaTela(grupos)).toBe(true);

    const onFocar = vi.fn();
    render(<NaTela paineis={grupos} sessoes={[]} naoVistas={new Set()} onFocar={onFocar} />);
    const juntas = screen.getByRole("list", { name: /Painel 1 · 2 conversas juntas/ });
    expect(within(juntas).getAllByRole("listitem")).toHaveLength(2);
    fireEvent.click(within(juntas).getByRole("button", { name: /Login/ }));
    expect(onFocar).toHaveBeenCalledWith(grupos[0].paneId, grupos[0].abas[0].tabId);
  });

  it("uma conversa só continua acessível na seção", () => {
    expect(deveMostrarNaTela([{ paneId: "p", emFoco: true, abas: [{ tabId: "t", titulo: "A", sessionId: "s", ativa: true }] }])).toBe(true);
  });
});
