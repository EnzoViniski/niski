import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { abrirRecuperacaoDoWorkspace, RecuperacaoDoWorkspace } from "../features/workspace/RecuperacaoDoWorkspace";
import {
  encontrarSessoesOrfas,
  pastaDoProjeto,
  projetosDasConversasRecentes,
} from "../features/workspace/recuperarWorkspace";
import { createProject } from "../features/workspace/workspaceReducer";
import { workspacesReducer } from "../features/workspace/workspacesReducer";
import { guardarBackup, listarBackups } from "../services/backupDoWorkspace";
import { emptyWorkspace, WORKSPACES_STORAGE_KEY } from "../services/workspaceService";
import type { Conversation, TerminalSession } from "../features/terminal/terminalService";
import type { PaneNode, WorkspaceCollection, WorkspaceState } from "../types/workspace";

const AGORA = Date.UTC(2026, 9, 20, 12);
const HORA = 60 * 60 * 1000;

function sessao(id: string, extra: Partial<TerminalSession> = {}): TerminalSession {
  return {
    id,
    project_id: "p-perdido",
    name: `Agente ${id}`,
    cwd: "/dev/perdido",
    shell: "zsh",
    state: "answered",
    pid: 1,
    output_seq: 0,
    rows: 24,
    cols: 80,
    provider: "claude",
    ...extra,
  };
}

function conversa(id: string, em: number, extra: Partial<Conversation> = {}): Conversation {
  return {
    id,
    project_id: "p-perdido",
    cwd: "/dev/perdido",
    title: `Conversa ${id}`,
    title_source: "auto",
    created_at_ms: em,
    segments: [],
    ...extra,
  };
}

function workspaceCom(...projetos: ReturnType<typeof createProject>[]): WorkspaceState {
  return { ...emptyWorkspace(), id: "ws-1", projects: projetos, activeProjectId: projetos[0]?.id ?? null };
}

function colecao(workspace: WorkspaceState): WorkspaceCollection {
  return { version: 1, workspaces: [workspace], activeWorkspaceId: workspace.id };
}

beforeEach(() => {
  localStorage.clear();
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue("{}");
});

describe("sessões órfãs", () => {
  it("acha agentes vivos de projeto que não está em workspace nenhum, agrupados por projeto", () => {
    const aberto = createProject("/dev/aberto", "p-aberto");
    const sessions = [
      sessao("s-1"),
      sessao("s-2", { state: "working" }),
      sessao("s-3", { project_id: "p-aberto", cwd: "/dev/aberto" }), // projeto aberto
      sessao("s-4", { state: "stopped" }), // não está viva
      sessao("s-5", { provider: null }), // terminal puro
      sessao("s-6", { project_id: "p-outro", cwd: "/dev/.medza-worktrees/outro/tarefa" }),
      sessao("s-7", { project_id: "p-id-de-historico", cwd: "/dev/aberto" }), // pasta já aberta
    ];

    const orfas = encontrarSessoesOrfas(sessions, [workspaceCom(aberto)]);
    expect(orfas).toEqual([
      { id: "p-perdido", path: "/dev/perdido", abas: [
        { sessionId: "s-1", title: "Agente s-1", provider: "claude" },
        { sessionId: "s-2", title: "Agente s-2", provider: "claude" },
      ] },
      { id: "p-outro", path: "/dev/outro", abas: [{ sessionId: "s-6", title: "Agente s-6", provider: "claude" }] },
    ]);
  });

  it("Fork: conversa do editor de vídeo (projeto video:<id>) não vira projeto solto", () => {
    const doVideo = sessao("s-v", { project_id: "video:v-1", cwd: "/dados/videos/projetos/v-1" });
    expect(encontrarSessoesOrfas([doVideo], [workspaceCom()])).toEqual([]);
  });

  it("sessão já aberta numa aba de outro projeto não é órfã", () => {
    const aberto = createProject("/dev/aberto", "p-aberto");
    const layout = aberto.layout as PaneNode;
    const comAba = { ...aberto, layout: { ...layout, tabs: [{ id: "t", kind: "agent" as const, title: "x", resourceId: "s-1" }], activeTabId: "t" } };
    expect(encontrarSessoesOrfas([sessao("s-1")], [workspaceCom(comAba)])).toEqual([]);
  });

  it("adotar recria o projeto com o mesmo id e as abas das sessões", () => {
    const estado = colecao(workspaceCom(createProject("/dev/aberto", "p-aberto")));
    const orfas = encontrarSessoesOrfas([sessao("s-1"), sessao("s-2")], estado.workspaces);

    const depois = workspacesReducer(estado, { type: "ADOTAR_PROJETOS", projetos: orfas });
    const ws = depois.workspaces[0];
    expect(ws.projects.map((projeto) => projeto.id)).toEqual(["p-aberto", "p-perdido"]);
    expect(ws.activeProjectId).toBe("p-perdido");
    const pane = ws.projects[1].layout as PaneNode;
    expect(pane.tabs.map((tab) => [tab.kind, tab.resourceId, tab.provider])).toEqual([
      ["agent", "s-1", "claude"],
      ["agent", "s-2", "claude"],
    ]);
    expect(pane.activeTabId).toBe(pane.tabs[0].id);
    // Adotar de novo não duplica abas nem projeto.
    const deNovo = workspacesReducer(depois, { type: "ADOTAR_PROJETOS", projetos: orfas });
    expect(deNovo.workspaces[0].projects).toHaveLength(2);
    expect((deNovo.workspaces[0].projects[1].layout as PaneNode).tabs).toHaveLength(2);
  });
});

describe("recriar a partir das conversas recentes", () => {
  it("lista os projetos das conversas das últimas 48 h fora dos workspaces, com as sessões que o engine conhece", () => {
    const aberto = createProject("/dev/aberto", "p-aberto");
    const conversas = [
      conversa("c-1", AGORA - 2 * HORA, {
        segments: [{ provider: "codex", profile_id: null, external_session_id: null, transcript_path: null, terminal_session_id: "s-1", started_at_ms: AGORA - 2 * HORA, ended_at_ms: null }],
      }),
      conversa("c-2", AGORA - 72 * HORA, {
        // Criada há 3 dias, mas usada há 1 h: entra.
        segments: [{ provider: "claude", profile_id: null, external_session_id: null, transcript_path: null, terminal_session_id: "s-sumida", started_at_ms: AGORA - HORA, ended_at_ms: null }],
      }),
      conversa("c-3", AGORA - 72 * HORA, { project_id: "p-velho", cwd: "/dev/velho" }),
      conversa("c-4", AGORA - HORA, { project_id: "p-aberto", cwd: "/dev/aberto" }),
      conversa("c-5", AGORA - HORA, { project_id: "p-wt", cwd: "/dev/.medza-worktrees/wt/tarefa" }),
    ];
    const sessions = [sessao("s-1", { state: "stopped", provider: "codex" })];

    const recentes = projetosDasConversasRecentes(conversas, sessions, [workspaceCom(aberto)], AGORA);
    expect(recentes).toEqual([
      {
        projeto: { id: "p-perdido", path: "/dev/perdido", abas: [{ sessionId: "s-1", title: "Conversa c-1", provider: "codex" }] },
        conversas: 2,
        ultimaMs: AGORA - HORA,
      },
      { projeto: { id: "p-wt", path: "/dev/wt", abas: [] }, conversas: 1, ultimaMs: AGORA - HORA },
    ]);

    const estado = colecao(workspaceCom(aberto));
    const depois = workspacesReducer(estado, { type: "ADOTAR_PROJETOS", projetos: recentes.map((item) => item.projeto) });
    expect(depois.workspaces[0].projects.map((projeto) => [projeto.id, projeto.path])).toEqual([
      ["p-aberto", "/dev/aberto"],
      ["p-perdido", "/dev/perdido"],
      ["p-wt", "/dev/wt"],
    ]);
    // Sem sessão conhecida, o projeto nasce como um "Adicionar projeto" comum.
    expect((depois.workspaces[0].projects[2].layout as PaneNode).tabs).toHaveLength(1);
  });

  it("pasta de worktree isolada volta para o repositório", () => {
    expect(pastaDoProjeto("/a/.medza-worktrees/repo/nome/sub")).toBe("/a/repo");
    expect(pastaDoProjeto("C:\\a\\.medza-worktrees\\repo\\nome")).toBe("C:\\a\\repo");
    expect(pastaDoProjeto("/a/repo")).toBe("/a/repo");
  });
});

describe("RecuperacaoDoWorkspace (tela)", () => {
  it("mostra o aviso das sessões órfãs e adiciona os projetos", () => {
    const dispatch = vi.fn();
    const workspaces = [workspaceCom()];
    render(<RecuperacaoDoWorkspace workspaces={workspaces} sessions={[sessao("s-1")]} dispatch={dispatch} mostrarAviso />);

    expect(screen.getByRole("status", { name: "Agentes fora do workspace" })).toHaveTextContent("1 agente rodando em um projeto fora do workspace: perdido");
    fireEvent.click(screen.getByRole("button", { name: "Adicionar estes projetos ao workspace" }));
    expect(dispatch).toHaveBeenCalledWith({ type: "ADOTAR_PROJETOS", projetos: [expect.objectContaining({ id: "p-perdido" })] });
    const log = vi.mocked(invoke).mock.calls.find(([comando]) => comando === "registrar_evento_da_ui");
    expect(log?.[1]).toEqual({ evento: "workspace_projetos_adotados", detalhe: "origem=orfas projetos=1 abas=1" });
  });

  it("dispensar esconde o aviso", () => {
    render(<RecuperacaoDoWorkspace workspaces={[workspaceCom()]} sessions={[sessao("s-1")]} dispatch={vi.fn()} mostrarAviso />);
    fireEvent.click(screen.getByRole("button", { name: "Dispensar" }));
    expect(screen.queryByRole("status", { name: "Agentes fora do workspace" })).toBeNull();
  });

  it("lista os backups com data e nº de projetos e restaura um, guardando o atual antes", () => {
    const antigo = { version: 1, activeWorkspaceId: "w", workspaces: [{ ...workspaceCom(createProject("/dev/a", "pa"), createProject("/dev/b", "pb")), id: "w" }] };
    guardarBackup(WORKSPACES_STORAGE_KEY, JSON.stringify(antigo), "conserto", new Date(2026, 9, 19, 9, 30));
    localStorage.setItem(WORKSPACES_STORAGE_KEY, JSON.stringify(colecao(workspaceCom())));
    const dispatch = vi.fn();
    render(<RecuperacaoDoWorkspace workspaces={[workspaceCom()]} sessions={[]} dispatch={dispatch} mostrarAviso={false} />);

    act(() => abrirRecuperacaoDoWorkspace());
    const dialogo = screen.getByRole("dialog", { name: "Recuperar workspace anterior" });
    expect(dialogo).toHaveTextContent("19/10/2026");
    expect(dialogo).toHaveTextContent("2 projetos");
    fireEvent.click(screen.getByRole("button", { name: "Restaurar" }));

    expect(dispatch).toHaveBeenCalledWith({
      type: "RESTAURAR_COLECAO",
      colecao: expect.objectContaining({ activeWorkspaceId: "w", workspaces: [expect.objectContaining({ id: "w" })] }),
    });
    expect(listarBackups().map((item) => item.motivo)).toEqual(["antes-de-restaurar", "conserto"]);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("recria pelos projetos das conversas recentes", async () => {
    vi.mocked(invoke).mockImplementation(async (comando: string) =>
      comando === "list_conversations" ? [conversa("c-1", Date.now() - HORA)] : "{}"
    );
    const dispatch = vi.fn();
    render(<RecuperacaoDoWorkspace workspaces={[workspaceCom()]} sessions={[]} dispatch={dispatch} mostrarAviso={false} />);
    act(() => abrirRecuperacaoDoWorkspace());
    fireEvent.click(screen.getByRole("button", { name: "Procurar conversas recentes" }));

    fireEvent.click(await screen.findByRole("button", { name: "Adicionar este projeto ao workspace" }));
    expect(dispatch).toHaveBeenCalledWith({
      type: "ADOTAR_PROJETOS",
      projetos: [{ id: "p-perdido", path: "/dev/perdido", abas: [] }],
    });
  });
});
