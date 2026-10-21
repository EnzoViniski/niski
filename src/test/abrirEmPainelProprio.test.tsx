import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { workspaceReducer } from "../features/workspace/workspaceReducer";
import { abrirEmPainelProprio, MAX_PAINEIS, sessoesNaTela } from "../features/workspace/abrirEmPainelProprio";
import { emptyWorkspace } from "../services/workspaceService";
import { PastasDeProjetos } from "../features/projects/PastasDeProjetos";
import type { LayoutNode, PaneNode, WorkspaceState } from "../types/workspace";
import type { TerminalSession } from "../features/terminal/terminalService";

/*
 * Fork (lateral, 21/10/2026): clicar numa conversa abre ela num painel próprio ao lado; o que está na
 * tela fica mais claro na lateral; a pasta abre e fecha com um clique só.
 */

vi.mock("../features/projects/historicoDaPasta", () => ({
  useConversasAnteriores: () => [], anterioresDaPasta: () => [], relerConversasAnteriores: vi.fn(),
}));
vi.mock("../features/orquestracao/orquestracaoService", () => ({ useResumoDaOrquestracao: () => ({ orquestradores: [] }) }));
vi.mock("../features/orquestracao/IconeDaOrquestradora", () => ({ IconeDaOrquestradora: () => null }));

function paineis(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...paineis(node.first), ...paineis(node.second)];
}
function projetoAberto(): WorkspaceState {
  return workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/omni" });
}
const projeto = (state: WorkspaceState) => state.projects[0];

describe("conversa da lateral em painel próprio", () => {
  it("painel ativo vazio: entra ali, sem dividir", () => {
    const depois = abrirEmPainelProprio(projetoAberto(), projetoAberto().projects[0].id, "s1", "Login");
    expect(paineis(projeto(depois).layout)).toHaveLength(1);
    expect(sessoesNaTela(projeto(depois).layout)).toEqual(["s1"]);
  });

  it("com uma conversa na tela, a próxima abre num painel novo à direita, em foco", () => {
    let state = projetoAberto();
    const id = projeto(state).id;
    state = abrirEmPainelProprio(state, id, "s1", "Login");
    state = abrirEmPainelProprio(state, id, "s2", "App-server");
    const todos = paineis(projeto(state).layout);
    expect(todos).toHaveLength(2);
    const raiz = projeto(state).layout;
    expect(raiz.type === "split" && raiz.direction).toBe("horizontal");
    expect(projeto(state).activePaneId).toBe(todos[1].id);
    expect(sessoesNaTela(raiz)).toEqual(["s1", "s2"]);
  });

  it("conversa que já está na tela só ganha o foco (não duplica nem divide)", () => {
    let state = projetoAberto();
    const id = projeto(state).id;
    state = abrirEmPainelProprio(state, id, "s1", "Login");
    state = abrirEmPainelProprio(state, id, "s2", "App-server");
    state = abrirEmPainelProprio(state, id, "s1", "Login");
    expect(paineis(projeto(state).layout)).toHaveLength(2);
    expect(projeto(state).activePaneId).toBe(paineis(projeto(state).layout)[0].id);
  });

  it(`com ${MAX_PAINEIS} painéis, vira aba no painel ativo`, () => {
    let state = projetoAberto();
    const id = projeto(state).id;
    for (let i = 1; i <= MAX_PAINEIS + 1; i++) state = abrirEmPainelProprio(state, id, `s${i}`, `C${i}`);
    expect(paineis(projeto(state).layout)).toHaveLength(MAX_PAINEIS);
    const ativo = paineis(projeto(state).layout).find((pane) => pane.id === projeto(state).activePaneId)!;
    expect(ativo.tabs.map((tab) => tab.resourceId)).toContain(`s${MAX_PAINEIS + 1}`);
  });
});

describe("lateral", () => {
  const sessao = (id: string, name: string, provider: string): TerminalSession => ({
    id, name, provider, project_id: "p", cwd: "/p", shell: "zsh", state: "answered", pid: 1, output_seq: 0, rows: 24, cols: 80,
  });
  function montar(extra: Partial<React.ComponentProps<typeof PastasDeProjetos>> = {}) {
    const onSelectProject = vi.fn();
    render(<PastasDeProjetos projects={[{ id: "p", name: "omni", path: "/p" } as never, { id: "q", name: "site", path: "/q" } as never]}
      activeProjectId="p" activeSessionId="s1" sessoesNaTela={["s1", "s2"]}
      agentSessions={[sessao("s1", "Login", "claude"), sessao("s2", "App-server", "codex"), sessao("s3", "Testes", "claude")]}
      naoVistas={new Set()} onSelectProject={onSelectProject} onCloseProject={vi.fn()} onAbrirConversa={vi.fn()}
      onBifurcar={vi.fn()} onReiniciar={vi.fn()} onFechar={vi.fn()} {...extra} />);
    return { onSelectProject };
  }

  it("a conversa em foco e as outras visíveis na tela ficam marcadas; o resto não", () => {
    localStorage.setItem("omni-pastas-abertas", JSON.stringify(["p"]));
    montar();
    const lista = screen.getByRole("list", { name: "Conversas de omni" });
    const linha = (nome: string) => within(lista).getByText(nome).closest("li")!;
    expect(linha("Login")).toHaveClass("is-ativa");
    expect(linha("App-server")).toHaveClass("is-na-tela");
    expect(linha("Testes")).not.toHaveClass("is-na-tela");
    expect(linha("Testes")).not.toHaveClass("is-ativa");
  });

  it("um clique abre e fecha a pasta; numa pasta de outro projeto também seleciona", async () => {
    localStorage.setItem("omni-pastas-abertas", JSON.stringify(["p"]));
    const { onSelectProject } = montar();
    await userEvent.click(screen.getByRole("button", { name: /^omni/ }));
    expect(screen.queryByRole("list", { name: "Conversas de omni" })).not.toBeInTheDocument();
    expect(onSelectProject).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: /^omni/ }));
    expect(screen.getByRole("list", { name: "Conversas de omni" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /^site/ }));
    expect(onSelectProject).toHaveBeenCalledWith("q");
    expect(screen.getByRole("list", { name: "Conversas de site" })).toBeInTheDocument();
  });
});
