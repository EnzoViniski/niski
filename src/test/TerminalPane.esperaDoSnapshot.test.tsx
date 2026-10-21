// Fork (energia): leitura do terminal por long-poll, com volta ao ritmo antigo se o engine não souber esperar.
import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { TerminalPane } from "../features/terminal/TerminalPane";
import type { WorkspaceTab } from "../types/workspace";

const engine = vi.hoisted(() => ({
  ensureEngine: vi.fn().mockResolvedValue(undefined),
  terminalSnapshot: vi.fn(),
  terminalSnapshotEspera: vi.fn(),
  writeTerminal: vi.fn().mockResolvedValue(undefined),
  resizeTerminal: vi.fn().mockResolvedValue(undefined),
  restartTerminal: vi.fn().mockResolvedValue(undefined),
  stopTerminal: vi.fn().mockResolvedValue(undefined),
  spawnTerminal: vi.fn(),
  attachTerminal: vi.fn().mockResolvedValue(undefined),
  accountUsage: vi.fn(),
  beginConversation: vi.fn(),
  ensureAgentTrust: vi.fn().mockResolvedValue(undefined),
  listAgentClis: vi.fn(),
  listProfiles: vi.fn(),
  planSwitch: vi.fn(),
  engineApi: vi.fn(),
}));

vi.mock("../features/terminal/terminalService", async (original) => ({
  ...(await original<typeof import("../features/terminal/terminalService")>()),
  ...engine,
}));

function snapshot(seq: number, estado?: string) {
  return {
    data: "",
    from_seq: seq,
    next_seq: seq + 1,
    session: { id: "s1", state: "answered", input_locked: false },
    ...(estado ? { estado } : {}),
  };
}

const tab: WorkspaceTab = { id: "t1", kind: "terminal", title: "Terminal", resourceId: "s1" };

function renderPane() {
  return render(<TerminalPane projectId="p1" projectPath="/dev/projeto" paneId="pane-1" tab={tab} onSessionCreated={vi.fn()} />);
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("TerminalPane — leitura com espera", () => {
  it("depois da primeira leitura passa a esperar no engine, devolvendo o estado conhecido", async () => {
    engine.terminalSnapshot.mockResolvedValue(snapshot(0, "aaaa"));
    // Segura as esperas seguintes: o engine só responde quando houver novidade.
    engine.terminalSnapshotEspera.mockResolvedValueOnce(snapshot(1, "bbbb")).mockReturnValue(new Promise(() => undefined));

    renderPane();

    await waitFor(() => expect(engine.terminalSnapshotEspera).toHaveBeenCalledTimes(2));
    expect(engine.terminalSnapshot).toHaveBeenCalledTimes(1);
    expect(engine.terminalSnapshotEspera).toHaveBeenNthCalledWith(1, "s1", 1, "aaaa", 20_000);
    expect(engine.terminalSnapshotEspera).toHaveBeenNthCalledWith(2, "s1", 2, "bbbb", 20_000);
    // Parado não pergunta de novo: só a espera em andamento.
    await new Promise((resolve) => setTimeout(resolve, 150));
    expect(engine.terminalSnapshotEspera).toHaveBeenCalledTimes(2);
  });

  it("com engine antigo (sem estado na resposta) continua no ritmo antigo", async () => {
    engine.terminalSnapshot.mockResolvedValue(snapshot(0));

    renderPane();

    await waitFor(() => expect(engine.terminalSnapshot.mock.calls.length).toBeGreaterThanOrEqual(2));
    expect(engine.terminalSnapshotEspera).not.toHaveBeenCalled();
  });
});
