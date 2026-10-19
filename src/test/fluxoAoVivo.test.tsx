import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FluxoAoVivo, posicoes, rotuloDoNo } from "../features/orquestracao/FluxoAoVivo";
import { IconeDaOrquestradora } from "../features/orquestracao/IconeDaOrquestradora";
import { EVENTO_ORQUESTRACAO } from "../features/orquestracao/abrirFluxo";
import { _zerarResumo, type FluxoAoVivo as Fluxo, type NoAoVivo } from "../features/orquestracao/orquestracaoService";
import { transporteDoEngine } from "../features/conversa/transporteDoEngine";
import { usarTransporte } from "../mobile/api";

// Fork: fluxo ao vivo do orquestrador (`FluxoAoVivo.tsx`) e o atalho da lateral.
const engine = vi.hoisted(() => ({ engineApi: vi.fn(), listTerminalSessions: vi.fn() }));
vi.mock("../features/terminal/terminalService", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../features/terminal/terminalService")>()),
  ...engine,
}));

const no = (parcial: Partial<NoAoVivo>): NoAoVivo => ({
  id: "x", titulo: null, plataforma: "claude", conta_id: null, conta: null, modelo: null, esforco: null, tarefa: null,
  criado_em: null, estado: "pronto", estado_texto: "pronto", pendente: null, ultima_mensagem: null, capabilities: undefined, ...parcial,
});

let fluxo: Fluxo;
let resumo: unknown;

beforeEach(() => {
  vi.clearAllMocks();
  _zerarResumo();
  fluxo = {
    orquestrador: no({ id: "orq", titulo: "Planejar a migração", modelo: "haiku", estado: "pronto", estado_texto: "pronto" }),
    ativo: true, pode_orquestrar: true, e_subagente: false, limite: 3,
    agentes: [
      no({ id: "sub-1", plataforma: "claude", conta: "Padrão", modelo: "haiku", tarefa: "Revisar o login", estado: "trabalhando", estado_texto: "trabalhando" }),
      no({ id: "sub-2", plataforma: "codex", conta: "Trabalho", modelo: "gpt-mini", esforco: "low", tarefa: "Escrever testes",
        estado: "esperando_voce", estado_texto: "esperando você", ultima_mensagem: "Posso rodar npm test?" }),
    ],
  };
  resumo = { limite: 3, orquestradores: [] };
  usarTransporte(transporteDoEngine);
  engine.engineApi.mockImplementation(async (_metodo: string, caminho: string) => {
    const corpo = caminho === "/orquestracao" ? resumo : caminho.startsWith("/orquestracao/orq") ? fluxo : { limite: 2 };
    return { type: "api", status: 200, body: JSON.stringify(corpo) };
  });
  engine.listTerminalSessions.mockResolvedValue([{ id: "terminal-9", conversation_id: "sub-2", name: "Codex · testes", state: "answered" }]);
});
afterEach(() => usarTransporte((path, init) => fetch(path, init)));

describe("posições do fluxo", () => {
  it("até 4 subagentes um de cada lado; mais que isso em círculo, sem sobrepor", () => {
    const dois = posicoes(2);
    expect(dois[0].x).toBeGreaterThan(0);
    expect(dois[1].x).toBeLessThan(0);
    expect(dois.map((p) => p.lado)).toEqual(["right", "left"]);
    const seis = posicoes(6);
    expect(new Set(seis.map((p) => `${p.x},${p.y}`)).size).toBe(6);
  });

  it("rótulo do nó junta conta, modelo e esforço", () => {
    expect(rotuloDoNo(no({ conta: "Trabalho", modelo: "gpt-mini", esforco: "low" }))).toBe("Trabalho · gpt-mini · low");
    expect(rotuloDoNo(no({}))).toBe("conta e modelo padrão");
  });
});

/** O texto dentro de um nó do canvas (o composer "Para:" repete as tarefas na lista). */
async function noComTexto(texto: string) {
  // O composer aparece antes: o React Flow monta os nós num tique seguinte.
  return waitFor(() => {
    const no = screen.getAllByText(texto).find((el) => el.closest(".orq-no"));
    if (!no) throw new Error(`nó com "${texto}" ainda não apareceu`);
    return no;
  });
}

describe("FluxoAoVivo", () => {
  it("mostra o orquestrador no centro e cada subagente com plataforma, conta, modelo, estado, tarefa e última mensagem", async () => {
    render(<div style={{ width: 800, height: 600 }}><FluxoAoVivo orquestrador="orq" paneId="pane-1" /></div>);
    expect(await noComTexto("Revisar o login")).toBeInTheDocument();
    expect(screen.getAllByText("Orquestrador").some((el) => el.closest(".orq-no.is-centro"))).toBe(true);
    expect(screen.getAllByText("Planejar a migração").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Codex").length).toBeGreaterThan(0);
    expect(screen.getByText("Trabalho · gpt-mini · low")).toBeInTheDocument();
    expect(screen.getByText("esperando você")).toBeInTheDocument();
    expect(screen.getByText("Posso rodar npm test?")).toBeInTheDocument();
    expect(screen.getByText("orquestrando")).toBeInTheDocument();
    expect(screen.getByText(/2 vivo\(s\)/)).toBeInTheDocument();
  });

  it("clique no nó abre a conversa dele ao lado do fluxo", async () => {
    const pedidos: unknown[] = [];
    const ouvir = (e: Event) => pedidos.push((e as CustomEvent).detail);
    window.addEventListener(EVENTO_ORQUESTRACAO, ouvir);
    render(<div style={{ width: 800, height: 600 }}><FluxoAoVivo orquestrador="orq" paneId="pane-1" /></div>);
    fireEvent.click(await noComTexto("Escrever testes"));
    await waitFor(() => expect(pedidos).toEqual([{ tipo: "abrir-conversa", sessionId: "terminal-9", titulo: "Codex · testes", paneId: "pane-1" }]));
    window.removeEventListener(EVENTO_ORQUESTRACAO, ouvir);
  });

  it("sem subagentes, explica como pedir; o limite troca pela rota do engine", async () => {
    fluxo = { ...fluxo, agentes: [] };
    render(<div style={{ width: 800, height: 600 }}><FluxoAoVivo orquestrador="orq" /></div>);
    expect(await screen.findByText(/Nenhum subagente ainda/)).toBeInTheDocument();
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Limite de subagentes" }), "2");
    await waitFor(() => expect(engine.engineApi.mock.calls.some(([m, c, b]) => m === "POST" && c === "/orquestracao/limite" && JSON.parse(String(b)).limite === 2)).toBe(true));
  });
});

describe("atalho na lateral", () => {
  it("só aparece na orquestradora, com o número de subagentes", async () => {
    resumo = { limite: 3, orquestradores: [{ orquestrador: "orq", ativo: true, agentes: 2, subagentes: ["a", "b"] }] };
    const { rerender } = render(<IconeDaOrquestradora conversa="orq" titulo="Planejar" />);
    expect(await screen.findByRole("button", { name: "Ver fluxo de Planejar" })).toHaveTextContent("2");
    rerender(<IconeDaOrquestradora conversa="outra" titulo="Outra" />);
    expect(screen.queryByRole("button", { name: /Ver fluxo/ })).not.toBeInTheDocument();
  });
});
