import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FaixaDoOrquestrador } from "../features/orquestracao/FaixaDoOrquestrador";
import { EVENTO_ORQUESTRACAO } from "../features/orquestracao/abrirFluxo";
import { _zerarResumo } from "../features/orquestracao/orquestracaoService";
import { transporteDoEngine } from "../features/conversa/transporteDoEngine";
import { usarTransporte } from "../mobile/api";
import { workspaceReducer } from "../features/workspace/workspaceReducer";
import { emptyWorkspace } from "../services/workspaceService";
import type { LayoutNode, PaneNode, WorkspaceState } from "../types/workspace";

// Fork: botões do orquestrador na faixa da conversa e a aba do fluxo (`features/orquestracao`).
const engine = vi.hoisted(() => ({ engineApi: vi.fn() }));
vi.mock("../features/terminal/terminalService", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../features/terminal/terminalService")>()),
  ...engine,
}));

let resumo = { limite: 3, orquestradores: [] as unknown[] };
let respostaDoAtivar: { status: number; body: unknown } = { status: 200, body: { ativo: true, reaberta: true } };

beforeEach(() => {
  vi.clearAllMocks();
  _zerarResumo();
  resumo = { limite: 3, orquestradores: [] };
  respostaDoAtivar = { status: 200, body: { ativo: true, reaberta: true } };
  usarTransporte(transporteDoEngine);
  engine.engineApi.mockImplementation(async (_metodo: string, caminho: string) => {
    if (caminho.endsWith("/ativar")) return { type: "api", status: respostaDoAtivar.status, body: JSON.stringify(respostaDoAtivar.body) };
    return { type: "api", status: 200, body: JSON.stringify(caminho === "/orquestracao" ? resumo : {}) };
  });
});
afterEach(() => usarTransporte((path, init) => fetch(path, init)));

const chamadas = (fim: string) => engine.engineApi.mock.calls.filter(([, caminho]) => String(caminho).endsWith(fim));

describe("faixa do orquestrador", () => {
  it("Tornar orquestradora pede confirmação e liga pela rota do engine", async () => {
    render(<FaixaDoOrquestrador conversaId="conv-1" provider="claude" titulo="Login" />);
    await userEvent.click(await screen.findByRole("button", { name: /Tornar orquestradora/ }));
    expect(chamadas("/ativar")).toHaveLength(0);
    await userEvent.click(screen.getByRole("button", { name: /Reabrir com ferramentas\?/ }));
    await waitFor(() => expect(chamadas("/ativar")).toHaveLength(1));
    const [metodo, caminho, corpo] = chamadas("/ativar")[0];
    expect([metodo, caminho, JSON.parse(String(corpo))]).toEqual(["POST", "/orquestracao/conv-1/ativar", { ativo: true }]);
  });

  it("Antigravity e Cursor não viram orquestradora (ainda não suportado)", async () => {
    const { rerender } = render(<FaixaDoOrquestrador conversaId="conv-1" provider="antigravity" />);
    await waitFor(() => expect(chamadas("/orquestracao").length).toBeGreaterThan(0));
    expect(screen.queryByRole("button", { name: /Tornar orquestradora/ })).not.toBeInTheDocument();
    rerender(<FaixaDoOrquestrador conversaId="conv-1" provider="cursor" />);
    expect(screen.queryByRole("button", { name: /Tornar orquestradora/ })).not.toBeInTheDocument();
  });

  it("orquestradora mostra Ver fluxo (N), abre o fluxo e pode deixar de orquestrar", async () => {
    resumo = { limite: 3, orquestradores: [{ orquestrador: "conv-1", ativo: true, agentes: 2, subagentes: ["a", "b"] }] };
    const pedidos: unknown[] = [];
    const ouvir = (e: Event) => pedidos.push((e as CustomEvent).detail);
    window.addEventListener(EVENTO_ORQUESTRACAO, ouvir);
    render(<FaixaDoOrquestrador conversaId="conv-1" provider="claude" titulo="Login" />);
    await userEvent.click(await screen.findByRole("button", { name: "Ver fluxo (2)" }));
    expect(pedidos).toEqual([{ tipo: "abrir-fluxo", orquestrador: "conv-1", titulo: "Fluxo · Login", paneId: undefined }]);
    window.removeEventListener(EVENTO_ORQUESTRACAO, ouvir);

    await userEvent.click(screen.getByRole("button", { name: "Deixar de orquestrar" }));
    await userEvent.click(screen.getByRole("button", { name: "Reabrir sem ferramentas?" }));
    await waitFor(() => expect(chamadas("/ativar")).toHaveLength(1));
    expect(JSON.parse(String(chamadas("/ativar")[0][2]))).toEqual({ ativo: false });
  });

  it("subagente leva ao fluxo da orquestradora e não oferece orquestrar (sem recursão)", async () => {
    resumo = { limite: 3, orquestradores: [{ orquestrador: "mae", ativo: true, agentes: 1, subagentes: ["conv-1"] }] };
    render(<FaixaDoOrquestrador conversaId="conv-1" provider="claude" />);
    expect(await screen.findByRole("button", { name: /Subagente/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Tornar orquestradora/ })).not.toBeInTheDocument();
  });

  it("recusa do engine (agente trabalhando) aparece na faixa", async () => {
    respostaDoAtivar = { status: 409, body: { error: "Espere o agente terminar o turno" } };
    render(<FaixaDoOrquestrador conversaId="conv-1" provider="codex" />);
    await userEvent.click(await screen.findByRole("button", { name: /Tornar orquestradora/ }));
    await userEvent.click(screen.getByRole("button", { name: /Reabrir com ferramentas\?/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Espere o agente terminar o turno");
  });
});

function panes(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...panes(node.first), ...panes(node.second)];
}
const projeto = (s: WorkspaceState) => s.projects[0];

describe("aba do fluxo (ABRIR_FLUXO)", () => {
  it("abre à direita da conversa e, se já aberta, só ganha o foco", () => {
    let state = workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/dev/a" });
    const conversa = projeto(state).activePaneId;
    state = workspaceReducer(state, { type: "ABRIR_FLUXO", orquestrador: "conv-1", title: "Fluxo", paneId: conversa });
    const raiz = projeto(state).layout;
    if (raiz.type !== "split" || raiz.second.type !== "pane") throw new Error("esperava o painel dividido");
    expect(raiz.direction).toBe("horizontal");
    expect(raiz.first.id).toBe(conversa);
    expect(raiz.second.tabs).toEqual([expect.objectContaining({ kind: "orquestracao", resourceId: "conv-1" })]);

    state = workspaceReducer(state, { type: "FOCUS_PANE", paneId: conversa });
    state = workspaceReducer(state, { type: "ABRIR_FLUXO", orquestrador: "conv-1", title: "Fluxo", paneId: conversa });
    const comFluxo = panes(projeto(state).layout).filter((p) => p.tabs.some((t) => t.kind === "orquestracao"));
    expect(comFluxo).toHaveLength(1);
    expect(projeto(state).activePaneId).toBe(comFluxo[0].id);
  });
});
