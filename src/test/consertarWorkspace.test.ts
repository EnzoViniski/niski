import { readFileSync } from "node:fs";
import { join } from "node:path";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { workspacesReducer } from "../features/workspace/workspacesReducer";
import { CHAVE_DO_BACKUP, colecaoDoBruto, guardarBackup, listarBackups } from "../services/backupDoWorkspace";
import { PANE_KINDS } from "../services/consertarWorkspace";
import { loadWorkspace, loadWorkspaces, WORKSPACE_STORAGE_KEY, WORKSPACES_STORAGE_KEY } from "../services/workspaceService";
import type { LayoutNode, PaneNode, SplitNode } from "../types/workspace";

const CAMINHO = "/Users/alguem/segredo/projeto-x";
const TITULO = "Título da conversa privada";

function pane(id: string, tabs: unknown[], activeTabId: unknown = (tabs[0] as { id?: string } | undefined)?.id ?? null) {
  return { type: "pane", id, tabs, activeTabId };
}

function colecaoCom(layout: unknown, extra: Record<string, unknown> = {}) {
  return {
    version: 1,
    activeWorkspaceId: "ws-1",
    workspaces: [
      {
        version: 1,
        id: "ws-1",
        name: "Trabalho",
        activeProjectId: "p-1",
        updatedAt: "2026-10-20T10:00:00.000Z",
        projects: [
          {
            id: "p-1",
            name: "projeto-x",
            path: CAMINHO,
            gitRoot: null,
            branch: null,
            composeFile: null,
            lastOpenedAt: "2026-10-20T10:00:00.000Z",
            layout,
            activePaneId: "pane-1",
            maximizedPaneId: null,
            ...extra,
          },
        ],
      },
    ],
  };
}

function gravar(valor: unknown) {
  localStorage.setItem(WORKSPACES_STORAGE_KEY, JSON.stringify(valor));
}

function chamadasDoLog() {
  return vi.mocked(invoke).mock.calls.filter(([comando]) => comando === "registrar_evento_da_ui");
}

beforeEach(() => {
  localStorage.clear();
  vi.mocked(invoke).mockClear();
});

describe("PANE_KINDS", () => {
  it("cobre todo PaneKind declarado no tipo (falha se um kind novo for esquecido)", () => {
    const fonte = readFileSync(join(__dirname, "../types/workspace.ts"), "utf8");
    const bloco = /export type PaneKind =([\s\S]*?);/.exec(fonte)?.[1] ?? "";
    const declarados = [...bloco.matchAll(/\|\s*"([^"]+)"/g)].map((m) => m[1]);
    expect(declarados.length).toBeGreaterThan(10);
    expect([...PANE_KINDS].sort()).toEqual([...new Set(declarados)].sort());
  });

  it("inclui a aba do fluxo do orquestrador", () => {
    expect(PANE_KINDS.has("orquestracao")).toBe(true);
  });
});

describe("leitura tolerante do workspace", () => {
  it("preserva a aba orquestracao e o workspace inteiro, sem backup", () => {
    const layout = pane("pane-1", [
      { id: "t-1", kind: "agent", title: "Claude", resourceId: "s-1" },
      { id: "t-2", kind: "orquestracao", title: "Fluxo", resourceId: "conv-1" },
    ], "t-2");
    gravar(colecaoCom(layout));

    const lida = loadWorkspaces();
    const projeto = lida.workspaces[0].projects[0];
    expect(projeto.path).toBe(CAMINHO);
    expect((projeto.layout as PaneNode).tabs.map((tab) => tab.kind)).toEqual(["agent", "orquestracao"]);
    expect((projeto.layout as PaneNode).activeTabId).toBe("t-2");
    expect(localStorage.getItem(CHAVE_DO_BACKUP)).toBeNull();
    expect(chamadasDoLog()).toHaveLength(0);
  });

  it("aba com kind de uma versão mais nova não derruba o workspace e fica intacta", () => {
    const futura = { id: "t-9", kind: "quadro-novo", title: "Do futuro", resourceId: "r-9", extra: { a: 1 } };
    gravar(colecaoCom(pane("pane-1", [{ id: "t-1", kind: "terminal", title: "zsh" }, futura])));

    const lida = loadWorkspaces();
    const tabs = (lida.workspaces[0].projects[0].layout as PaneNode).tabs;
    expect(tabs).toHaveLength(2);
    expect(tabs[1]).toEqual(futura);
    // Não é conserto: nada é reescrito nem copiado, mas o log registra o kind.
    expect(localStorage.getItem(CHAVE_DO_BACKUP)).toBeNull();
    const [[, args]] = chamadasDoLog();
    expect(args).toMatchObject({ evento: "workspace_aba_desconhecida" });
    expect((args as { detalhe: string }).detalhe).toContain("kinds=quadro-novo");
  });

  it("grampeia ratio, reaponta ids inválidos e descarta só a aba quebrada", () => {
    const layout: unknown = {
      type: "split",
      id: "split-1",
      direction: "horizontal",
      ratio: 0.95,
      first: pane("pane-1", [{ id: "t-1", kind: "agent", title: "A" }, { kind: "agent", title: "sem id" }], "nao-existe"),
      second: {
        type: "split",
        id: "split-2",
        direction: "diagonal",
        ratio: "meio",
        first: pane("pane-2", [], "t-fantasma"),
        second: { type: "coisa" },
      },
    };
    const original = colecaoCom(layout, { activePaneId: "pane-sumida", maximizedPaneId: "pane-sumida" });
    original.workspaces[0].activeProjectId = "p-sumido";
    gravar(original);

    const lida = loadWorkspaces();
    const projeto = lida.workspaces[0].projects[0];
    const raiz = projeto.layout as SplitNode;
    expect(raiz.ratio).toBe(0.8);
    const primeiro = raiz.first as PaneNode;
    expect(primeiro.tabs.map((tab) => tab.id)).toEqual(["t-1"]);
    expect(primeiro.activeTabId).toBe("t-1");
    // O split interno perdeu um lado ilegível e virou o painel que sobrou.
    const segundo = raiz.second as LayoutNode;
    expect(segundo).toMatchObject({ type: "pane", id: "pane-2", tabs: [], activeTabId: null });
    expect(projeto.activePaneId).toBe("pane-1");
    expect(projeto.maximizedPaneId).toBeNull();
    expect(lida.workspaces[0].activeProjectId).toBe("p-1");
  });

  it("guarda o JSON cru antes de consertar e loga só contagens", () => {
    const original = colecaoCom(pane("pane-1", [{ id: "t-1", kind: "agent", title: TITULO }, { id: 7 }]), { activePaneId: "x" });
    const bruto = JSON.stringify(original);
    localStorage.setItem(WORKSPACES_STORAGE_KEY, bruto);

    loadWorkspaces();

    const backups = listarBackups();
    expect(backups).toHaveLength(1);
    expect(backups[0]).toMatchObject({ chave: WORKSPACES_STORAGE_KEY, motivo: "conserto", bruto, workspaces: 1, projetos: 1 });
    expect(Number.isNaN(Date.parse(backups[0].em))).toBe(false);
    const chamadas = chamadasDoLog();
    expect(chamadas).toHaveLength(1);
    const { evento, detalhe } = chamadas[0][1] as { evento: string; detalhe: string };
    expect(evento).toBe("workspace_consertado");
    expect(detalhe).toContain("abas_descartadas=1");
    expect(detalhe).toContain("ids_reapontados=1");
    expect(detalhe).not.toContain(CAMINHO);
    expect(detalhe).not.toContain("projeto-x");
    expect(detalhe).not.toContain(TITULO);
  });

  it("projeto só cai sem id ou path; workspace só cai se não for objeto", () => {
    const original = colecaoCom(pane("pane-1", []));
    const ws = original.workspaces[0] as { projects: unknown[] };
    ws.projects.push({ id: "p-2" }, { path: "/tmp/x" }, { id: "p-3", path: "/tmp/sem-layout" });
    (original.workspaces as unknown[]).push("lixo", { id: "ws-2", projects: "nao-e-lista" });
    gravar(original);

    const lida = loadWorkspaces();
    expect(lida.workspaces.map((item) => item.id)).toEqual(["ws-1", "ws-2"]);
    expect(lida.workspaces[0].projects.map((item) => item.id)).toEqual(["p-1", "p-3"]);
    const semLayout = lida.workspaces[0].projects[1];
    expect(semLayout.name).toBe("sem-layout");
    expect(semLayout.layout).toMatchObject({ type: "pane", tabs: [], activeTabId: null });
    expect(semLayout.activePaneId).toBe((semLayout.layout as PaneNode).id);
    expect(lida.workspaces[1]).toMatchObject({ name: "Workspace 3", projects: [], activeProjectId: null });
  });

  it("JSON ilegível vai para o backup e cai na chave legada", () => {
    localStorage.setItem(WORKSPACES_STORAGE_KEY, "{quebrado");
    localStorage.setItem(
      WORKSPACE_STORAGE_KEY,
      JSON.stringify({ version: 1, id: "default", name: "Workspace", projects: [], activeProjectId: null, updatedAt: "x" })
    );

    const lida = loadWorkspaces();
    expect(lida.workspaces[0].name).toBe("Workspace 1");
    expect(listarBackups()[0]).toMatchObject({ motivo: "ilegivel", bruto: "{quebrado", projetos: 0 });
    expect(chamadasDoLog()[0][1]).toMatchObject({ evento: "workspace_ilegivel" });
  });

  it("a chave legada também é consertada em vez de descartada", () => {
    localStorage.setItem(
      WORKSPACE_STORAGE_KEY,
      JSON.stringify({
        version: 1,
        id: "w",
        name: "W",
        activeProjectId: "p",
        projects: [{ id: "p", name: "p", path: "/p", layout: pane("a", [{ id: "t", kind: "orquestracao", title: "Fluxo" }]), activePaneId: "a" }],
      })
    );
    expect(loadWorkspace().projects[0].layout).toMatchObject({ tabs: [{ kind: "orquestracao" }] });
  });
});

describe("backup e restauração", () => {
  it("guarda só as 3 últimas e não repete a mais recente", () => {
    for (const n of [1, 2, 2, 3, 4]) guardarBackup(WORKSPACES_STORAGE_KEY, JSON.stringify({ n }), "conserto", new Date(2026, 9, n));
    expect(listarBackups().map((item) => JSON.parse(item.bruto).n)).toEqual([4, 3, 2]);
  });

  it("restaura um backup pelo reducer, já consertado", () => {
    const antigo = colecaoCom(pane("pane-1", [{ id: "t-1", kind: "orquestracao", title: "Fluxo" }]), { activePaneId: "sumida" });
    guardarBackup(WORKSPACES_STORAGE_KEY, JSON.stringify(antigo), "conserto");
    const [backup] = listarBackups();
    const colecao = colecaoDoBruto(backup.bruto);
    expect(colecao).not.toBeNull();

    const vazio = loadWorkspaces();
    const restaurado = workspacesReducer(vazio, { type: "RESTAURAR_COLECAO", colecao: colecao! });
    expect(restaurado.activeWorkspaceId).toBe("ws-1");
    expect(restaurado.workspaces[0].projects[0]).toMatchObject({ id: "p-1", path: CAMINHO, activePaneId: "pane-1" });
  });

  it("backup da chave legada (um workspace só) também restaura", () => {
    const legado = colecaoCom(pane("pane-1", [])).workspaces[0];
    const colecao = colecaoDoBruto(JSON.stringify(legado));
    expect(colecao?.workspaces.map((item) => item.id)).toEqual(["ws-1"]);
    expect(colecao?.activeWorkspaceId).toBe("ws-1");
  });
});
