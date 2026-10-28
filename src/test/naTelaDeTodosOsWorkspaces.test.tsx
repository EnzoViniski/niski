// Navegar em todas as telas abertas sem recriar sessões nem perder o destino da aba.
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { NaTela } from "../features/projects/NaTela";
import { todasAsTelasAbertas } from "../features/projects/telasAbertas";
import { findPane } from "../features/workspace/workspaceReducer";
import { workspacesReducer } from "../features/workspace/workspacesReducer";
import { emptyWorkspace } from "../services/workspaceService";
import type { LayoutNode, PaneNode, Project, WorkspaceCollection, WorkspaceTab } from "../types/workspace";
import type { TerminalSession } from "../features/terminal/terminalService";

const aba = (id: string, resourceId?: string, kind: WorkspaceTab["kind"] = "agent"): WorkspaceTab => ({
  id, kind, resourceId, title: id, provider: kind === "agent" && resourceId ? "claude" : undefined,
});
const painel = (id: string, tabs: WorkspaceTab[], activeTabId = tabs[0]?.id ?? null): PaneNode => ({ type: "pane", id, tabs, activeTabId });
const projeto = (id: string, layout: LayoutNode, activePaneId: string): Project => ({
  id, name: id, path: `/fake/${id}`, gitRoot: null, branch: null, composeFile: null, lastOpenedAt: "",
  layout, activePaneId, maximizedPaneId: null,
});
function colecao(): WorkspaceCollection {
  const primeiro = projeto("site", painel("primeiro", [aba("duplicada", "compartilhada")]), "primeiro");
  const segundo = projeto("servidor", {
    type: "split", id: "split", direction: "horizontal", ratio: .5,
    first: painel("lado-a", [aba("API", "api"), aba("README.md", "/fake/README.md", "markdown")]),
    second: painel("lado-b", [aba("Novo agente")]),
  }, "lado-a");
  segundo.maximizedPaneId = "lado-a";
  const terceiro = projeto("mobile", painel("atual", [aba("Conversa atual", "compartilhada"), aba("Revisar app", "revisao")]), "atual");
  return { version: 1, activeWorkspaceId: "pessoal", workspaces: [
    { ...emptyWorkspace(), id: "empresa", name: "Empresa", projects: [primeiro, segundo], activeProjectId: primeiro.id },
    { ...emptyWorkspace(), id: "pessoal", name: "Pessoal", projects: [terceiro], activeProjectId: terceiro.id },
  ] };
}

describe("Na tela de todos os workspaces", () => {
  it("preserva ordem, inclui projetos fora da tela e seletores/arquivos, mostrando cada sessão uma vez", () => {
    const state = colecao();
    const grupos = todasAsTelasAbertas(state.workspaces, state.activeWorkspaceId);
    expect(grupos.map((grupo) => `${grupo.workspaceId}/${grupo.projectId}/${grupo.paneId}`)).toEqual([
      "empresa/servidor/lado-a", "empresa/servidor/lado-b", "pessoal/mobile/atual",
    ]);
    expect(grupos.flatMap((grupo) => grupo.abas).filter((item) => item.sessionId === "compartilhada")).toHaveLength(1);
    expect(grupos.flatMap((grupo) => grupo.abas).map((item) => item.titulo)).toEqual([
      "API", "README.md", "Novo agente", "Conversa atual", "Revisar app",
    ]);
    expect(grupos[0].abas.every((item) => !item.ativa)).toBe(true);
    expect(grupos.filter((grupo) => grupo.emFoco)).toEqual([grupos[2]]);
    expect(grupos[2].abas[0].ativa).toBe(true);
    // Deduplicar é só uma projeção: nenhuma aba é apagada do estado salvo.
    expect((state.workspaces[0].projects[0].layout as PaneNode).tabs).toHaveLength(1);
  });

  it("a sessão duplicada conserva a primeira ocorrência quando nenhuma cópia está visível", () => {
    const state = colecao();
    const grupos = todasAsTelasAbertas(state.workspaces, null);
    expect(grupos[0]).toMatchObject({ workspaceId: "empresa", projectId: "site" });
    expect(grupos[0].abas[0].titulo).toBe("duplicada");
    expect(grupos.flatMap((grupo) => grupo.abas).filter((item) => item.sessionId === "compartilhada")).toHaveLength(1);
  });

  it("painel escondido pela maximização não marca aba como visível nem vence a deduplicação", () => {
    const state = colecao();
    const project = state.workspaces[1].projects[0];
    project.layout = { type: "split", id: "maximizado", direction: "horizontal", ratio: .5,
      first: painel("escondido", [aba("Cópia escondida", "compartilhada"), aba("Outra escondida", "outra")]),
      second: painel("visivel", [aba("Nova conversa visível"), aba("Cópia visível", "compartilhada")]),
    };
    project.activePaneId = "visivel";
    project.maximizedPaneId = "visivel";
    const grupos = todasAsTelasAbertas(state.workspaces, state.activeWorkspaceId);
    const escondido = grupos.find((grupo) => grupo.paneId === "escondido")!;
    expect(escondido.emFoco).toBe(false);
    expect(escondido.abas.every((item) => !item.ativa)).toBe(true);
    expect(grupos.flatMap((grupo) => grupo.abas).filter((item) => item.sessionId === "compartilhada").map((item) => item.titulo))
      .toEqual(["Cópia visível"]);
    expect(grupos.find((grupo) => grupo.emFoco)?.paneId).toBe("visivel");
  });

  it("clicar no Novo agente muda workspace, projeto, painel e aba exatos, revelando o painel maximizado", () => {
    const acao = vi.fn();
    function Navegacao() {
      const [state, setState] = useState(colecao);
      const atual = state.workspaces.find((workspace) => workspace.id === state.activeWorkspaceId)!;
      const project = atual.projects.find((item) => item.id === atual.activeProjectId)!;
      const pane = findPane(project.layout, project.activePaneId)!;
      return <>
        <NaTela paineis={todasAsTelasAbertas(state.workspaces, state.activeWorkspaceId)} sessoes={[]} naoVistas={new Set()}
          onFocar={(paneId, tabId, contexto) => {
            const action = { type: "FOCUS_OPEN_TAB" as const, destino: { ...contexto!, paneId, tabId } };
            acao(action);
            setState((anterior) => workspacesReducer(anterior, action));
          }} />
        <output aria-label="Destino selecionado">{[atual.id, project.id, pane.id, pane.activeTabId, project.maximizedPaneId].join("/")}</output>
      </>;
    }
    render(<Navegacao />);
    fireEvent.click(screen.getByRole("button", { name: "Novo agente" }));
    expect(acao).toHaveBeenLastCalledWith({ type: "FOCUS_OPEN_TAB", destino: {
      workspaceId: "empresa", projectId: "servidor", paneId: "lado-b", tabId: "Novo agente",
    } });
    expect(screen.getByLabelText("Destino selecionado")).toHaveTextContent("empresa/servidor/lado-b/Novo agente/lado-b");
    expect(screen.getByRole("button", { name: "Novo agente" })).toHaveAttribute("aria-current", "page");
    fireEvent.click(screen.getByRole("button", { name: "README.md" }));
    expect(screen.getByLabelText("Destino selecionado")).toHaveTextContent("empresa/servidor/lado-a/README.md/lado-a");
    expect(screen.getByRole("button", { name: "README.md" })).toHaveAttribute("aria-current", "page");
  });

  it("o contexto de cada grupo é legível e a marca viva prevalece na sessão que mudou de agente", () => {
    const state = colecao();
    render(<NaTela paineis={todasAsTelasAbertas(state.workspaces, state.activeWorkspaceId)} naoVistas={new Set()} onFocar={vi.fn()}
      sessoes={[{ id: "compartilhada", provider: "codex", name: "Conversa atual", state: "answered" } as TerminalSession]} />);
    const grupo = screen.getByRole("list", { name: "Pessoal · mobile · Painel 1 · 2 conversas juntas" });
    const conversa = within(grupo).getByRole("button", { name: "Conversa atual" });
    expect(conversa.querySelector("svg")).toHaveStyle({ color: "#a78bfa" });
    expect(screen.getByRole("list", { name: "Empresa · servidor · Painel 2" })).toBeInTheDocument();
  });

  it("clique atrasado em uma aba já fechada não muda o contexto", () => {
    const state = colecao();
    const fechado = workspacesReducer(state, { type: "FOCUS_OPEN_TAB", destino: {
      workspaceId: "empresa", projectId: "servidor", paneId: "lado-b", tabId: "sumiu",
    } });
    expect(fechado).toBe(state);
  });
});
