import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PaneView } from "../features/workspace/PaneView";
import { ProjetosDasAbasContext } from "../features/workspace/projetosDasAbas";
import { SessionProvidersContext } from "../features/terminal/sessionProviders";
import type { TerminalSession } from "../features/terminal/terminalService";
import type { PaneNode } from "../types/workspace";

// Fork: o "+" de um painel que mostra conversa de outro projeto abre agente nesse projeto.
vi.mock("../features/terminal/TerminalPane", () => ({ TerminalPane: () => null }));
vi.mock("../features/conversa/AbaDeAgente", () => ({ AbaDeAgente: ({ children }: { children: React.ReactNode }) => children }));

describe("+ no painel", () => {
  it("cria a aba no projeto da conversa que o painel mostra", () => {
    const dispatch = vi.fn();
    const pane: PaneNode = { type: "pane", id: "p1", activeTabId: "t1", tabs: [{ id: "t1", kind: "agent", title: "Omni", resourceId: "s1" }] };
    const sessao = { id: "s1", project_id: "omni", name: "Omni", cwd: "/omni" } as unknown as TerminalSession;
    render(
      <SessionProvidersContext.Provider value={{ s1: sessao }}>
        <ProjetosDasAbasContext.Provider value={{ projetoAtual: "yorcookie", projetos: { omni: { name: "omni", path: "/omni" }, yorcookie: { name: "yorcookie", path: "/y" } }, abasDaSessao: () => 1 }}>
          <PaneView pane={pane} active onlyPane={false} maximized={false} projectId="yorcookie" projectPath="/y" fileSaveMode="manual"
            agentView="terminal" kanban={{ version: 1, tasks: [], agentByProject: {}, dispatcherEnabled: false, failureTimestamps: [], pausedUntil: null }}
            kanbanDispatch={vi.fn()} dispatch={dispatch} />
        </ProjetosDasAbasContext.Provider>
      </SessionProvidersContext.Provider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Nova tab" }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Novo agente/ }));
    expect(dispatch).toHaveBeenCalledWith(expect.objectContaining({ type: "CREATE_TAB", paneId: "p1", kind: "agent", projectId: "omni" }));
  });
});
