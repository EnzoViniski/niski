import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FluxoAoVivo, posicoes, rotuloDoNo } from "../features/orquestracao/FluxoAoVivo";
import { IconeDaOrquestradora } from "../features/orquestracao/IconeDaOrquestradora";
import { EVENTO_ORQUESTRACAO } from "../features/orquestracao/abrirFluxo";
import { _zerarResumo, type FluxoAoVivo as Fluxo, type NoAoVivo } from "../features/orquestracao/orquestracaoService";
import { transporteDoEngine } from "../features/conversa/transporteDoEngine";
import { usarTransporte } from "../mobile/api";

// Fluxo ao vivo do orquestrador (`FluxoAoVivo.tsx`) e o atalho da lateral.
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
      no({ id: "sub-1", plataforma: "claude", conta: "Padrão", modelo: "haiku", tarefa: "Revisar o login", estado: "trabalhando", estado_texto: "trabalhando",
        editar: ["src/login.ts"], criar: ["src/login/", "docs/login.md"] }),
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
  it("da esquerda para a direita: orquestrador à esquerda, subagentes em coluna e nova coluna a cada 5", () => {
    const tres = posicoes(3);
    expect(tres.agentes.every((p) => p.x > tres.centro.x)).toBe(true);
    expect(tres.agentes.map((p) => p.x)).toEqual([tres.agentes[0].x, tres.agentes[0].x, tres.agentes[0].x]);
    expect(tres.agentes[1].y).toBeGreaterThan(tres.agentes[0].y);
    expect(tres.centro.y).toBe(tres.agentes[1].y);
    const sete = posicoes(7);
    expect(sete.agentes[5].x).toBeGreaterThan(sete.agentes[4].x);
    expect(sete.agentes[5].y).toBe(sete.agentes[0].y);
    expect(new Set(sete.agentes.map((p) => `${p.x},${p.y}`)).size).toBe(7);
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
    // Harness: o que o subagente pode mexer aparece no nó.
    expect(screen.getByText("edita src/login.ts")).toBeInTheDocument();
    expect(screen.getByText("cria 2 caminhos").parentElement).toHaveAttribute("title", "edita src/login.ts\ncria src/login/\ncria docs/login.md");
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
