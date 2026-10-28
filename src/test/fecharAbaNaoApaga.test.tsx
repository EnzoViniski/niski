import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PaneView } from "../features/workspace/PaneView";
import type { PaneNode } from "../types/workspace";

// Fechar aba ou painel de agente não apaga a conversa; terminal puro continua sendo encerrado.
const closeTerminal = vi.fn(() => Promise.resolve());
vi.mock("../features/terminal/terminalService", async (original) => ({
  ...(await original<typeof import("../features/terminal/terminalService")>()),
  closeTerminal: (id: string) => closeTerminal(id),
}));
vi.mock("../features/terminal/TerminalPane", () => ({ TerminalPane: () => null }));
vi.mock("../features/conversa/AbaDeAgente", () => ({ AbaDeAgente: ({ children }: { children: React.ReactNode }) => children }));

function renderizar(kind: "agent" | "terminal") {
  const dispatch = vi.fn();
  const pane: PaneNode = { type: "pane", id: "p1", activeTabId: "t1", tabs: [{ id: "t1", kind, title: "Aba", resourceId: "s1" }] };
  render(<PaneView pane={pane} active onlyPane={false} maximized={false} projectId="proj" projectPath="/p" fileSaveMode="manual"
    agentView="terminal" kanban={{ version: 1, tasks: [], agentByProject: {}, dispatcherEnabled: false, failureTimestamps: [], pausedUntil: null }}
    kanbanDispatch={vi.fn()} dispatch={dispatch} />);
  return dispatch;
}

describe("fechar aba", () => {
  it.each(["Fechar tab Aba", "Fechar painel"])("de agente (%s) só tira da tela, sem encerrar a sessão", async (botao) => {
    closeTerminal.mockClear();
    const dispatch = renderizar("agent");
    fireEvent.click(screen.getByRole("button", { name: botao }));
    await waitFor(() => expect(dispatch).toHaveBeenCalledWith(expect.objectContaining({ type: botao.startsWith("Fechar tab") ? "CLOSE_TAB" : "CLOSE_PANE" })));
    expect(closeTerminal).not.toHaveBeenCalled();
  });

  it("de terminal puro encerra o shell", async () => {
    closeTerminal.mockClear();
    const dispatch = renderizar("terminal");
    fireEvent.click(screen.getByRole("button", { name: "Fechar tab Aba" }));
    await waitFor(() => expect(dispatch).toHaveBeenCalledWith(expect.objectContaining({ type: "CLOSE_TAB" })));
    expect(closeTerminal).toHaveBeenCalledWith("s1");
  });
});
