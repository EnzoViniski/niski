// Fork: regressões reais do + da pasta, marca após troca e fechamento de workspace.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { WorkspaceList } from "../components/WorkspaceList";
import { AgentLauncher } from "../features/terminal/AgentLauncher";
import { workspacesReducer } from "../features/workspace/workspacesReducer";
import { workspaceReducer } from "../features/workspace/workspaceReducer";
import { emptyWorkspace } from "../services/workspaceService";
import { ajustesDoAgente, lembrarAgente, lembrarAjustes, lembrarConta } from "../lib/escolhasSalvas";
import type { PaneNode, WorkspaceCollection } from "../types/workspace";

beforeEach(() => { localStorage.clear(); vi.mocked(invoke).mockReset().mockResolvedValue(null); });

describe("confirmação do workspace", () => {
  function montar() {
    const onClose = vi.fn();
    const workspace = { ...emptyWorkspace(), id: "trabalho", name: "Trabalho" };
    render(<WorkspaceList workspaces={[workspace]} activeWorkspaceId={workspace.id} attentionByWorkspace={{}}
      onClose={onClose} onCreate={vi.fn()} onRename={vi.fn()} onSelect={vi.fn()} />);
    const fechar = screen.getByRole("button", { name: "Fechar workspace Trabalho" });
    return { onClose, fechar };
  }

  it("clicar no X aguarda confirmação; Cancelar mantém o workspace e devolve o foco", async () => {
    const { onClose, fechar } = montar();
    fechar.focus();
    fireEvent.click(fechar);
    const dialogo = screen.getByRole("alertdialog", { name: "Fechar workspace Trabalho?" });
    expect(onClose).not.toHaveBeenCalled();
    expect(within(dialogo).getByRole("button", { name: "Cancelar" })).toHaveFocus();
    fireEvent.click(within(dialogo).getByRole("button", { name: "Cancelar" }));
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(fechar).toHaveFocus();
  });

  it("teclado: Escape cancela, Tab fica no diálogo e Enter confirma uma única vez", async () => {
    const usuario = userEvent.setup();
    const { onClose, fechar } = montar();
    await act(async () => { await usuario.click(fechar); });
    await act(async () => { await usuario.keyboard("{Escape}"); });
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await act(async () => { await usuario.keyboard("{Enter}"); }); // foco voltou ao X
    await act(async () => { await usuario.tab(); });
    const dialogo = screen.getByRole("alertdialog");
    expect(within(dialogo).getByRole("button", { name: "Fechar" })).toHaveFocus();
    await act(async () => { await usuario.tab(); });
    expect(within(dialogo).getByRole("button", { name: "Cancelar" })).toHaveFocus();
    await act(async () => { await usuario.tab({ shift: true }); await usuario.keyboard("{Enter}"); });
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("trabalho");
  });
});

describe("identidade persistida da conversa", () => {
  it("atualiza marca/nome em projetos e workspaces fora da tela sem recriar nada no poll seguinte", () => {
    let workspace = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/fake/app" });
    const project = workspace.projects[0];
    const pane = project.layout as PaneNode;
    const tab = { ...pane.tabs[0], kind: "agent" as const, provider: "claude", resourceId: "sessao", title: "Antes" };
    workspace = { ...workspace, projects: [{ ...project, layout: { ...pane, tabs: [tab] } }] };
    const state: WorkspaceCollection = { version: 1, activeWorkspaceId: "outra", workspaces: [
      { ...workspace, id: "outra" }, { ...workspace, id: "fora-da-tela", activeProjectId: null },
    ] };
    const action = { type: "SYNC_SESSION_IDENTITIES" as const, sessions: [{ id: "sessao", provider: "codex", name: "Depois" }] };
    const atualizado = workspacesReducer(state, action);
    for (const item of atualizado.workspaces) {
      const salvo = (item.projects[0].layout as PaneNode).tabs[0];
      expect(salvo).toMatchObject({ provider: "codex", title: "Depois", resourceId: "sessao" });
      expect(item.projects[0].activePaneId).toBe(project.activePaneId);
    }
    expect(workspacesReducer(atualizado, action)).toBe(atualizado);
    expect((state.workspaces[0].projects[0].layout as PaneNode).tabs[0].provider).toBe("claude");
  });
});

describe("Novo agente com modelo, conta e esforço", () => {
  it("lembra a última marca, mas só abre depois de escolher modelo/conta e confirmar", async () => {
    lembrarAgente("codex"); lembrarConta("codex", "pessoal");
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "agent_cli_statuses") return ["claude", "codex"].map((id) => ({ id, label: id === "claude" ? "Claude" : "Codex",
        command: id, path: `/fake/${id}`, available: true, authenticated: true }));
      if (command === "list_profiles") return ["pessoal", "trabalho"].map((id) => ({ id, provider: "codex", name: id,
        config_dir: `/fake/${id}`, builtin: false, authenticated: true }));
      if (command === "agentes_modelos") return { codex: [{ id: "modelo-falso", label: "Modelo de teste", esforcos: ["low", "high"] }] };
      return [];
    });
    const onLaunch = vi.fn();
    render(<AgentLauncher projectName="app" projectPath="/fake/app" onLaunch={onLaunch} iniciarDireto />);
    await waitFor(() => expect(screen.getByRole("radio", { name: /Codex/ })).toHaveAttribute("aria-checked", "true"));
    expect(onLaunch).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("combobox", { name: "Modelo" }));
    fireEvent.click(await screen.findByRole("option", { name: "Modelo de teste" }));
    fireEvent.click(screen.getByRole("combobox", { name: "Esforço" }));
    fireEvent.click(screen.getByRole("option", { name: "Alto" }));
    fireEvent.click(screen.getByRole("combobox", { name: "Conta" }));
    fireEvent.click(screen.getByRole("option", { name: /trabalho/ }));
    fireEvent.click(screen.getByRole("button", { name: "Abrir Codex" }));
    expect(onLaunch).toHaveBeenCalledWith(expect.objectContaining({
      agent: expect.objectContaining({ id: "codex" }), profile: expect.objectContaining({ id: "trabalho" }),
      ajustes: { model: "modelo-falso", effort: "high" },
    }));
    expect(ajustesDoAgente("codex")).toEqual({ model: "modelo-falso", effort: "high" });
  });

  it("o Codex local não recebe o modelo cloud lembrado", async () => {
    lembrarAjustes("codex", { model: "cloud-falso", effort: "high" });
    vi.mocked(invoke).mockImplementation(async (command) => command === "agent_cli_statuses"
      ? [{ id: "codex", label: "Codex", command: "codex", path: "/fake/codex", available: true, authenticated: true }] : []);
    const onLaunch = vi.fn();
    render(<AgentLauncher projectName="app" projectPath="/fake/app" onLaunch={onLaunch} />);
    fireEvent.click(await screen.findByRole("combobox", { name: "Usar modelo" }));
    fireEvent.click(screen.getByRole("option", { name: "Local com Ollama" }));
    fireEvent.change(screen.getByLabelText("Nome do modelo instalado"), { target: { value: "modelo-local" } });
    fireEvent.click(screen.getByRole("button", { name: "Abrir Codex" }));
    expect(onLaunch).toHaveBeenCalledWith(expect.objectContaining({ ajustes: { model: null, effort: null },
      agent: expect.objectContaining({ command: "codex --oss --local-provider ollama --model modelo-local" }) }));
    expect(ajustesDoAgente("codex")?.model).toBe("cloud-falso");
  });
});
