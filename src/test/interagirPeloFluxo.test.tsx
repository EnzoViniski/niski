import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FluxoAoVivo } from "../features/orquestracao/FluxoAoVivo";
import { _zerarResumo, type FluxoAoVivo as Fluxo, type NoAoVivo } from "../features/orquestracao/orquestracaoService";
import { transporteDoEngine } from "../features/conversa/transporteDoEngine";
import { usarTransporte } from "../mobile/api";

// Fork: fatia (c) do orquestrador — prompt para qualquer nó, aprovar/negar e parar pelo fluxo.
const engine = vi.hoisted(() => ({ engineApi: vi.fn(), listTerminalSessions: vi.fn() }));
vi.mock("../features/terminal/terminalService", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../features/terminal/terminalService")>()),
  ...engine,
}));

const cap = (parcial: Partial<NonNullable<NoAoVivo["capabilities"]>>): NoAoVivo["capabilities"] =>
  ({ prompt: false, approve: false, revision: '"r0"', approval_text: null, reason: null, ...parcial });
const no = (parcial: Partial<NoAoVivo>): NoAoVivo => ({
  id: "x", titulo: null, plataforma: "claude", conta_id: null, conta: null, modelo: null, esforco: null, tarefa: null,
  criado_em: null, estado: "pronto", estado_texto: "pronto", pendente: null, ultima_mensagem: null, capabilities: cap({}), ...parcial,
});

let fluxo: Fluxo;
/** Capabilities que a timeline devolve na hora do envio, por conversa. */
let naHora: Record<string, NoAoVivo["capabilities"]>;
let statusDoAprovar = 202;

beforeEach(() => {
  vi.clearAllMocks();
  _zerarResumo();
  statusDoAprovar = 202;
  fluxo = {
    orquestrador: no({ id: "orq", titulo: "Planejar", capabilities: cap({ prompt: true }) }),
    ativo: true, pode_orquestrar: true, e_subagente: false, limite: 3,
    agentes: [
      no({ id: "sub-1", tarefa: "Revisar o login", estado: "trabalhando", estado_texto: "trabalhando" }),
      no({ id: "sub-2", plataforma: "codex", tarefa: "Escrever testes", estado: "esperando_voce", estado_texto: "esperando você",
        capabilities: cap({ approve: true, revision: '"r-aprovar"', approval_text: "Do you want to run npm test?" }) }),
    ],
  };
  naHora = { orq: cap({ prompt: true, revision: '"r-orq"' }), "sub-1": cap({ enfileirar: true, revision: '"r-sub1"' }), "sub-2": cap({ approve: true }) };
  usarTransporte(transporteDoEngine);
  engine.engineApi.mockImplementation(async (metodo: string, caminho: string) => {
    const resposta = (status: number, corpo: unknown) => ({ type: "api", status, body: JSON.stringify(corpo) });
    if (caminho === "/orquestracao") return resposta(200, { limite: 3, orquestradores: [] });
    if (caminho === "/orquestracao/orq") return resposta(200, fluxo);
    const timeline = /^\/conversas\/([^/]+)\/timeline$/.exec(caminho);
    if (timeline) return resposta(200, { timeline: { messages: [] }, actions: [], conversation: { id: timeline[1], capabilities: naHora[timeline[1]] } });
    if (caminho.endsWith("/aprovar")) return resposta(statusDoAprovar, statusDoAprovar === 412 ? { error: "O contexto mudou" } : { id: "a1" });
    if (metodo === "POST") return resposta(202, { id: "a2", ok: true });
    return resposta(404, { error: "?" });
  });
  engine.listTerminalSessions.mockResolvedValue([]);
});
afterEach(() => usarTransporte((path, init) => fetch(path, init)));

const posts = (fim: string) => engine.engineApi.mock.calls.filter(([metodo, caminho]) => metodo === "POST" && String(caminho).endsWith(fim));
/** Botão dentro de um nó: o React Flow deixa o nó invisível até medir (o jsdom não mede), então a
 *  busca por papel não o enxerga. */
const botaoDoNo = (texto: string) => screen.getByText(texto).closest("button")!;
const montar = () => render(<div style={{ width: 900, height: 700 }}><FluxoAoVivo orquestrador="orq" paneId="p" /></div>);

describe("interagir pelo fluxo", () => {
  it("aprova o pedido de permissão mostrado no nó, com a versão da tela que estava à vista", async () => {
    montar();
    expect(await screen.findByText("Do you want to run npm test?")).toBeInTheDocument();
    fireEvent.click(botaoDoNo("Permitir"));
    await waitFor(() => expect(posts("/aprovar")).toHaveLength(1));
    const [, caminho, corpo, opcoes] = posts("/aprovar")[0];
    expect(caminho).toBe("/conversas/sub-2/aprovar");
    expect(JSON.parse(String(corpo))).toEqual({ permitir: true });
    expect(Object.fromEntries((opcoes as { headers: [string, string][] }).headers)["If-Match"]).toBe('"r-aprovar"');
    fireEvent.click(botaoDoNo("Negar"));
    await waitFor(() => expect(posts("/aprovar")).toHaveLength(2));
    expect(JSON.parse(String(posts("/aprovar")[1][2]))).toEqual({ permitir: false });
  });

  it("tela mudou desde a leitura (412): não aprova às cegas e avisa", async () => {
    statusDoAprovar = 412;
    montar();
    await screen.findByText("Permitir");
    fireEvent.click(botaoDoNo("Permitir"));
    expect(await screen.findByRole("alert")).toHaveTextContent("A tela do agente mudou");
  });

  it("parar pede confirmação e usa a rota do orquestrador", async () => {
    montar();
    await screen.findAllByText("Revisar o login");
    fireEvent.click(screen.getAllByText("Parar")[0].closest("button")!);
    expect(posts("/parar")).toHaveLength(0);
    fireEvent.click(botaoDoNo("Parar mesmo?"));
    await waitFor(() => expect(posts("/parar")).toHaveLength(1));
    expect(posts("/parar")[0][1]).toBe("/orquestracao/orq/agentes/sub-1/parar");
  });

  it("Para: o orquestrador por padrão; trocar para um subagente trabalhando manda para a fila dele", async () => {
    montar();
    const caixa = await screen.findByRole("textbox", { name: "Mensagem" });
    await userEvent.type(caixa, "junte os resultados{Enter}");
    await waitFor(() => expect(posts("/prompt")).toHaveLength(1));
    let [, caminho, corpo, opcoes] = posts("/prompt")[0];
    expect(caminho).toBe("/conversas/orq/prompt");
    expect(JSON.parse(String(corpo))).toEqual({ texto: "junte os resultados", fila: false });
    expect(Object.fromEntries((opcoes as { headers: [string, string][] }).headers)["If-Match"]).toBe('"r-orq"');
    expect(caixa).toHaveValue("");

    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Para quem" }), "sub-1");
    await userEvent.type(caixa, "use o arquivo novo{Enter}");
    await waitFor(() => expect(posts("/prompt")).toHaveLength(2));
    [, caminho, corpo] = posts("/prompt")[1];
    expect(caminho).toBe("/conversas/sub-1/prompt");
    expect(JSON.parse(String(corpo))).toEqual({ texto: "use o arquivo novo", fila: true });
  });

  it("agente esperando permissão não recebe mensagem: o fluxo explica", async () => {
    montar();
    await userEvent.selectOptions(await screen.findByRole("combobox", { name: "Para quem" }), "sub-2");
    await userEvent.type(screen.getByRole("textbox", { name: "Mensagem" }), "oi{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("Responda o pedido de permissão antes.");
    expect(posts("/prompt")).toHaveLength(0);
  });
});
