import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Sidebar } from "../components/Sidebar";
import type { AttentionItem } from "../features/terminal/useAttention";
import type { WorkspaceState } from "../types/workspace";
import type { TerminalSession } from "../features/terminal/terminalService";

vi.mock("../components/FileTree", () => ({ FileTree: () => null }));
vi.mock("../components/GitPanel", () => ({ GitPanel: () => null }));
vi.mock("../components/SkillsList", () => ({ SkillsList: () => null }));
vi.mock("../components/KanbanPanel", () => ({ KanbanPanel: () => null }));
vi.mock("../hooks/useNotifications", () => ({ useNotifications: () => [] }));

function workspace(id: string, name: string): WorkspaceState {
  return { version: 1, id, name, projects: [], activeProjectId: null, updatedAt: "" };
}

const item: AttentionItem = {
  sessionId: "s1",
  conversationKey: "conv-1",
  sessionName: "Claude · api",
  reason: "approval_required",
  projectId: "api",
  projectName: "api",
  workspaceId: "ws-a",
  workspaceName: "Cliente X",
};

function renderSidebar(overrides: Partial<React.ComponentProps<typeof Sidebar>> = {}) {
  const props: React.ComponentProps<typeof Sidebar> = {
    workspaces: [workspace("ws-a", "Cliente X"), workspace("ws-b", "Estudos")],
    activeWorkspaceId: "ws-b",
    onSelectWorkspace: vi.fn(),
    onCreateWorkspace: vi.fn(),
    onCloseWorkspace: vi.fn(),
    onRenameWorkspace: vi.fn(),
    attention: [item],
    attentionByWorkspace: { "ws-a": 1 },
    onFocusSession: vi.fn(),
    onDismissAttention: vi.fn(),
    onRunInTerminal: vi.fn(),
    projects: [],
    activeProjectId: null,
    activeProjectPath: null,
    onOpenFile: vi.fn(),
    onFileRenamed: vi.fn(),
    onPathDeleted: vi.fn(),
    terminalSessions: [],
    showSettings: false,
    showHistory: false,
    onOpenHistory: vi.fn(),
    onHome: vi.fn(),
    onAddProject: vi.fn(),
    onSelectProject: vi.fn(),
    onCloseProject: vi.fn(),
    onAttachTerminal: vi.fn(),
    onCloseTerminal: vi.fn(),
    onDuplicateTerminal: vi.fn(),
    onRestartTerminal: vi.fn(),
    onOpenSettings: vi.fn(),
    onOpenGitDiff: vi.fn(),
    onOpenGitGraph: vi.fn(),
    kanban: {
      version: 1,
      tasks: [],
      agentByProject: {},
      dispatcherEnabled: false,
      failureTimestamps: [],
      pausedUntil: null,
    },
    onOpenKanban: vi.fn(),
    ...overrides,
  };
  render(<Sidebar {...props} />);
  return props;
}

describe("Sidebar", () => {
  it("mostra o agente pendente de outro workspace e pula pra ele no clique", async () => {
    const props = renderSidebar();

    const aviso = screen.getByRole("button", { name: /claude · api aprovar cliente x · api/i });
    expect(aviso).toBeInTheDocument();

    await userEvent.click(aviso);
    expect(props.onFocusSession).toHaveBeenCalledWith(item);
  });

  it("dispensa o aviso pelo X sem abrir o agente", async () => {
    const props = renderSidebar();
    await userEvent.click(screen.getByRole("button", { name: /dispensar aviso de/i }));
    expect(props.onDismissAttention).toHaveBeenCalledWith(item);
    expect(props.onFocusSession).not.toHaveBeenCalled();
  });

  it("marca o workspace inativo com a contagem de pendências", () => {
    renderSidebar();
    expect(screen.getByLabelText("1 agente esperando em Cliente X")).toBeInTheDocument();
    expect(screen.getByLabelText("1 agente esperando no total")).toBeInTheDocument();
  });

  it("lista os workspaces sem precisar abrir dropdown", () => {
    renderSidebar({ attention: [], attentionByWorkspace: {} });

    expect(screen.getByRole("button", { name: "Cliente X" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Estudos" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByText("aprovar")).not.toBeInTheDocument();
  });
  /** Conversas com IA ficam em CHATS; shells sem IA (npm run dev, git) em TERMINALS. */
  it("separa as conversas com agente dos terminais", () => {
    const sessao = (id: string, name: string, provider: string | null): TerminalSession => ({
      id, name, provider, project_id: "p", cwd: "C:/p", shell: "bash", state: "answered", pid: 1, output_seq: 0, rows: 24, cols: 80,
    });
    renderSidebar({
      attention: [],
      attentionByWorkspace: {},
      activeProjectId: "p",
      activeProjectPath: "C:/p",
      terminalSessions: [sessao("s1", "Corrigir o login", "claude"), sessao("s2", "npm run dev", null)],
    });

    const chats = screen.getByRole("button", { name: "CHATS" }).parentElement!;
    const terminais = screen.getByRole("button", { name: "TERMINALS" }).parentElement!;
    expect(within(chats).getByText("Corrigir o login")).toBeInTheDocument();
    expect(within(chats).queryByText("npm run dev")).not.toBeInTheDocument();
    expect(within(terminais).getByText("npm run dev")).toBeInTheDocument();
    expect(within(terminais).queryByText("Corrigir o login")).not.toBeInTheDocument();
  });
});
