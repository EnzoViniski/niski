import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AgentLauncher } from "../features/terminal/AgentLauncher";
import { DialogoDaWorktree, worktreeParaPerguntar } from "../features/terminal/DialogoDaWorktree";
import { EtiquetaDaWorktree } from "../features/terminal/EtiquetaDaWorktree";
import { LimiteDeSessoes } from "../features/terminal/LimiteDeSessoes";
import { nomeDaWorktree, worktreeDaPasta } from "../features/terminal/worktreeIsolada";
import type { TerminalSession } from "../features/terminal/terminalService";

// Fork: agente em worktree isolada (`.medza-worktrees`, branch medza/<nome>) e limite de sessões.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const WT = "/c/.medza-worktrees/app/login";

function sessao(id: string, cwd: string, state: TerminalSession["state"] = "answered"): TerminalSession {
  return { id, project_id: "p", name: id, cwd, shell: "zsh", state, pid: 1, output_seq: 0, rows: 24, cols: 80, provider: "claude" };
}

beforeEach(() => vi.mocked(invoke).mockReset());

describe("worktree isolada — caminho e nome", () => {
  it("reconhece a pasta da worktree e a branch pelo caminho", () => {
    expect(worktreeDaPasta(`${WT}/src`)).toEqual({ repo: "app", nome: "login", branch: "medza/login", raiz: WT });
    expect(worktreeDaPasta("/c/app")).toBeNull();
    expect(worktreeDaPasta("/c/.medza-worktrees/app")).toBeNull();
    expect(worktreeDaPasta(null)).toBeNull();
    expect(nomeDaWorktree(" Tela de Ação! ")).toBe("tela-de-acao");
  });

  it("só pergunta da worktree quando nenhuma outra sessão viva usa a mesma", () => {
    const lista = [sessao("a", WT), sessao("b", `${WT}/src`), sessao("c", "/c/app")];
    expect(worktreeParaPerguntar(lista, "a")).toBeNull();
    expect(worktreeParaPerguntar([lista[0], sessao("b", WT, "stopped")], "a")).toBe(WT);
    expect(worktreeParaPerguntar(lista, "c")).toBeNull();
    expect(worktreeParaPerguntar(lista, "x")).toBeNull();
  });

  it("a aba mostra a branch da worktree", () => {
    render(<EtiquetaDaWorktree cwd={`${WT}/src`} />);
    expect(screen.getByLabelText("Worktree isolada na branch medza/login")).toHaveTextContent("medza/login");
  });
});

describe("Novo agente — Isolar em worktree", () => {
  function mockLauncher(criar: () => Promise<unknown>) {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      switch (command) {
        case "agent_cli_statuses":
          return [{ id: "claude", label: "Claude", command: "claude", path: "x", available: true, authenticated: true }];
        case "list_profiles": return [];
        case "git_branches": return [{ name: "main", current: true }];
        case "criar_worktree_isolada": return criar();
        default: return null;
      }
    });
  }

  it("cria a worktree com o nome dado e abre o agente nela", async () => {
    mockLauncher(async () => ({ caminho: WT, pasta_de_trabalho: WT, branch: "medza/login", nome: "login", copiados: 1 }));
    const onLaunch = vi.fn();
    render(<AgentLauncher projectName="app" projectPath="/c/app" onLaunch={onLaunch} />);
    fireEvent.click(await screen.findByLabelText("Isolar em worktree"));
    expect(screen.getByRole("combobox", { name: /Branch/ })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Nome da worktree"), { target: { value: "Login" } });
    expect(screen.getByText(/na branch nova medza\/login a partir de main/)).toBeInTheDocument();
    const abrir = screen.getByRole("button", { name: /Abrir Claude/ });
    await waitFor(() => expect(abrir).not.toBeDisabled());
    fireEvent.click(abrir);
    await waitFor(() => expect(onLaunch).toHaveBeenCalledWith(expect.objectContaining({ cwd: WT })));
    expect(invoke).toHaveBeenCalledWith("criar_worktree_isolada", { projectPath: "/c/app", nome: "Login" });
  });

  it("erro do git aparece e o agente não abre", async () => {
    mockLauncher(async () => { throw new Error("O repositório ainda não tem nenhum commit"); });
    const onLaunch = vi.fn();
    render(<AgentLauncher projectName="app" projectPath="/c/app" onLaunch={onLaunch} />);
    fireEvent.click(await screen.findByLabelText("Isolar em worktree"));
    const abrir = screen.getByRole("button", { name: /Abrir Claude/ });
    await waitFor(() => expect(abrir).not.toBeDisabled());
    fireEvent.click(abrir);
    expect(await screen.findByText(/nenhum commit/)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("criar_worktree_isolada", { projectPath: "/c/app", nome: "claude" });
    expect(onLaunch).not.toHaveBeenCalled();
  });
});

describe("Encerrar agente em worktree", () => {
  it("com mudanças não commitadas avisa e só apaga forçando", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => command === "estado_da_worktree_isolada"
      ? { caminho: WT, branch: "medza/login", mudancas: 2, exemplos: ["src/a.ts", "novo.txt"] } : null);
    const onFechar = vi.fn();
    render(<DialogoDaWorktree caminho={`${WT}/src`} onFechar={onFechar} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("2 mudanças não commitadas");
    expect(screen.getByText("src/a.ts")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Apagar mesmo assim" }));
    await waitFor(() => expect(onFechar).toHaveBeenCalled());
    expect(invoke).toHaveBeenCalledWith("remover_worktree_isolada", { caminho: WT, forcar: true });
  });

  it("limpa: manter não apaga nada; apagar não força", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => command === "estado_da_worktree_isolada"
      ? { caminho: WT, branch: "medza/login", mudancas: 0, exemplos: [] } : null);
    const onFechar = vi.fn();
    const { unmount } = render(<DialogoDaWorktree caminho={WT} onFechar={onFechar} />);
    fireEvent.click(await screen.findByRole("button", { name: "Manter worktree" }));
    expect(onFechar).toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalledWith("remover_worktree_isolada", expect.anything());
    unmount();
    render(<DialogoDaWorktree caminho={WT} onFechar={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: "Apagar worktree" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("remover_worktree_isolada", { caminho: WT, forcar: false }));
  });

  it("pasta que não é mais worktree fecha sem perguntar", async () => {
    vi.mocked(invoke).mockResolvedValue(null);
    const onFechar = vi.fn();
    render(<DialogoDaWorktree caminho={WT} onFechar={onFechar} />);
    await waitFor(() => expect(onFechar).toHaveBeenCalled());
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("Configurações — limite de sessões", () => {
  it("lê, valida a faixa e salva", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
      if (command === "limite_de_sessoes") return { limite: 32, padrao: 32, minimo: 4, maximo: 128 };
      if (command === "definir_limite_de_sessoes") return { limite: (args as { valor: number }).valor, padrao: 32, minimo: 4, maximo: 128 };
      return null;
    });
    render(<LimiteDeSessoes />);
    const campo = await screen.findByLabelText("Limite de sessões ativas neste PC");
    await waitFor(() => expect(campo).toHaveValue(32));
    const salvar = screen.getByRole("button", { name: "Salvar" });
    fireEvent.change(campo, { target: { value: "2" } });
    expect(salvar).toBeDisabled();
    fireEvent.change(campo, { target: { value: "48" } });
    fireEvent.click(salvar);
    expect(await screen.findByText(/Limite salvo: 48/)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("definir_limite_de_sessoes", { valor: 48 });
  });
});
