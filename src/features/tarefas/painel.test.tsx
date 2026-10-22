import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { aplicarAcoes, desfazerFeitos, lerResposta, montarPedido } from "./assistente";
import { criarTarefa, diaDe } from "./dominio";
import { BotaoDasTarefas, ListaDoDia } from "./Flutuantes";
import { CHAVE_DOS_AJUSTES, reiniciarParaTeste, salvar, transporteEmMemoria, usarTransporteDasTarefas, type TransporteDasTarefas } from "./loja";
import { abrirPainelDeTarefas, reiniciarPainelParaTeste, usePainel } from "./painel";
import { PainelDeTarefas } from "./PainelDeTarefas";
import { Quadro } from "./Visoes";
import type { Pessoa, Tarefa } from "./tipos";

// jsdom não tem PointerEvent: sem ele o React recebe o evento sem botão nem coordenadas.
if (!("PointerEvent" in window)) {
  class PointerEventFalso extends MouseEvent { constructor(tipo: string, init?: MouseEventInit) { super(tipo, init); } }
  (window as unknown as { PointerEvent: typeof MouseEvent }).PointerEvent = PointerEventFalso;
}

const HOJE = diaDe(new Date());
const eu: Pessoa = { id: "enzo", nome: "Enzo", cor: "#000", atualizadaEm: "" };
let pasta: TransporteDasTarefas;

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem(CHAVE_DOS_AJUSTES, JSON.stringify({ eu }));
  pasta = transporteEmMemoria();
  usarTransporteDasTarefas(pasta);
  reiniciarParaTeste();
  reiniciarPainelParaTeste();
});

describe("assistente", () => {
  it("lê o JSON e ignora ações mal formadas", () => {
    const r = lerResposta({ resposta: "Pronto", acoes: [{ tipo: "criar", tarefa: { titulo: "Dentista" } }, { tipo: "criar", tarefa: {} }, { tipo: "voar", id: "x" }, { tipo: "concluir", id: "t1" }] });
    expect(typeof r !== "string" && r.acoes.map((a) => a.tipo)).toEqual(["criar", "concluir"]);
    expect(typeof lerResposta({ acoes: [] })).toBe("string");
  });

  it("aplica criar/editar/concluir/apagar e desfaz tudo", () => {
    const t1 = criarTarefa({ id: "t1", titulo: "Estudar", data: HOJE }, "enzo");
    const t2 = criarTarefa({ id: "t2", titulo: "Boleto" }, "enzo");
    const { gravar, feitos } = aplicarAcoes([
      { tipo: "criar", tarefa: { titulo: "Dentista", data: HOJE, hora: "10:00", categoria: "saude", checklist: [{ texto: "RG" } as never], status: "lixo" as never } },
      { tipo: "editar", id: "t1", campos: { prioridade: "alta", id: "hack" } as never },
      { tipo: "concluir", id: "t2" },
      { tipo: "apagar", id: "nao-existe" },
    ], [t1, t2], "enzo");
    expect(gravar.map((t) => t.titulo)).toEqual(["Dentista", "Estudar", "Boleto"]);
    expect(gravar[0]).toMatchObject({ hora: "10:00", categoria: "saude", status: "a_fazer", criadaPor: "enzo" });
    expect(gravar[0].checklist[0].texto).toBe("RG");
    expect(gravar[1]).toMatchObject({ id: "t1", prioridade: "alta" });
    expect(gravar[2].status).toBe("feito");
    const voltas = desfazerFeitos(feitos, [...gravar]);
    expect(voltas.find((t) => t.titulo === "Dentista")?.apagada).toBe(true);
    expect(voltas.find((t) => t.id === "t1")?.prioridade).toBe("media");
    expect(voltas.find((t) => t.id === "t2")?.status).toBe("a_fazer");
  });

  it("o pedido leva data, pessoas, tarefas e a conversa", () => {
    const texto = montarPedido("e amanhã?", [criarTarefa({ id: "t1", titulo: "Prova", data: HOJE }, "enzo")], [eu], eu, [{ de: "eu", texto: "oi", em: "" }]);
    expect(texto).toContain(HOJE);
    expect(texto).toContain("id=t1");
    expect(texto).toContain("Pessoa: oi");
    expect(texto).toContain("Pessoa: e amanhã?");
  });
});

describe("arrastar no quadro (ponteiro)", () => {
  it("solta o cartão em outra coluna e grava o status novo", async () => {
    const t = criarTarefa({ id: "t1", titulo: "Mover-me", status: "a_fazer" }, "enzo");
    const onMover = vi.fn();
    render(<Quadro tarefas={[t]} hoje={HOJE} pessoas={[eu]} eu={eu} abertaId={null} ocultarFeitasAntigas onAbrir={vi.fn()} onAlternar={vi.fn()}
      onMover={onMover} onNovaEm={vi.fn()} onCriar={vi.fn()} />);
    const cartao = screen.getByText("Mover-me").closest("[data-tarefa]") as HTMLElement;
    const destino = screen.getByRole("region", { name: "Fazendo" });
    const original = document.elementFromPoint;
    document.elementFromPoint = () => destino;
    try {
      fireEvent.pointerDown(cartao, { button: 0, clientX: 10, clientY: 10 });
      act(() => { window.dispatchEvent(new MouseEvent("pointermove", { clientX: 200, clientY: 40 })); });
      expect(destino.className).toContain("is-alvo");
      act(() => { window.dispatchEvent(new MouseEvent("pointerup", { clientX: 200, clientY: 40 })); });
    } finally {
      document.elementFromPoint = original;
    }
    expect(onMover).toHaveBeenCalledWith("t1", expect.objectContaining({ status: "fazendo" }));
  });

  it("clique sem mover abre; ⌥→ leva para a próxima coluna", async () => {
    const t = criarTarefa({ id: "t1", titulo: "Clique", status: "a_fazer" }, "enzo");
    const onAbrir = vi.fn(), onMover = vi.fn();
    render(<Quadro tarefas={[t]} hoje={HOJE} pessoas={[eu]} eu={eu} abertaId={null} ocultarFeitasAntigas onAbrir={onAbrir} onAlternar={vi.fn()}
      onMover={onMover} onNovaEm={vi.fn()} onCriar={vi.fn()} />);
    const cartao = screen.getByText("Clique").closest("[data-tarefa]") as HTMLElement;
    await userEvent.click(cartao);
    expect(onAbrir).toHaveBeenCalledWith("t1");
    fireEvent.keyDown(cartao, { key: "ArrowRight", altKey: true });
    expect(onMover).toHaveBeenCalledWith("t1", expect.objectContaining({ status: "fazendo" }));
  });

  it("+ Adicionar tarefa na coluna cria pela frase", async () => {
    const onCriar = vi.fn();
    render(<Quadro tarefas={[]} hoje={HOJE} pessoas={[eu]} eu={eu} abertaId={null} ocultarFeitasAntigas onAbrir={vi.fn()} onAlternar={vi.fn()}
      onMover={vi.fn()} onNovaEm={vi.fn()} onCriar={onCriar} />);
    const coluna = screen.getByRole("region", { name: "Esperando" });
    await userEvent.click(within(coluna).getByRole("button", { name: /adicionar tarefa/i }));
    await userEvent.keyboard("Resposta do banco amanhã{Enter}");
    expect(onCriar).toHaveBeenCalledWith(expect.objectContaining({ titulo: "Resposta do banco", status: "revisao" }));
  });
});

function Estado() { const p = usePainel(); return <span data-testid="estado">{p.aberto ? "aberto" : "fechado"}/{p.listaFixada ? "fixada" : "solta"}</span>; }

describe("painel e flutuantes", () => {
  it("o botão com a logo abre o painel; Esc e clique fora fecham", async () => {
    render(<><BotaoDasTarefas /><PainelDeTarefas onAbrirAjustes={vi.fn()} /><Estado /></>);
    await userEvent.click(screen.getByRole("button", { name: /^tarefas/i }));
    expect(screen.getByRole("dialog", { name: "Tarefas" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^tarefas/i })).not.toBeInTheDocument(); // o botão some com o painel aberto
    expect(screen.getByRole("complementary", { name: /assistente/i })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(screen.getByTestId("estado")).toHaveTextContent("fechado");
    act(() => abrirPainelDeTarefas());
    fireEvent.mouseDown(document.querySelector(".tarefas-painel-fundo") as HTMLElement);
    expect(screen.getByTestId("estado")).toHaveTextContent("fechado");
  });

  it("lista do dia: fixa pelo 📌, conclui e cria para hoje", async () => {
    await salvar(criarTarefa({ id: "t1", titulo: "Ligar pro médico", data: HOJE }, "enzo"));
    render(<><BotaoDasTarefas /><ListaDoDia /></>);
    await userEvent.click(screen.getByTitle(/lista do dia flutuante/i));
    const lista = screen.getByRole("complementary", { name: "Lista do dia" });
    expect(within(lista).getByText("Ligar pro médico")).toBeInTheDocument();
    await userEvent.click(within(lista).getByRole("button", { name: /concluir "Ligar pro médico"/i }));
    await userEvent.type(within(lista).getByLabelText("Nova tarefa para hoje"), "Comprar pão{Enter}");
    const noDisco = (await pasta.listar(null, "tarefas")).map((x) => JSON.parse(x) as Tarefa);
    expect(noDisco.find((t) => t.id === "t1")?.status).toBe("feito");
    expect(noDisco.find((t) => t.titulo === "Comprar pão")?.data).toBe(HOJE);
  });
});

describe("conversa com o assistente (CLI falsa)", () => {
  it("manda a mensagem, cria a tarefa que a IA pediu e deixa desfazer", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockImplementation(async (comando: string) => comando === "roadmaps_ia"
      ? JSON.stringify({ resposta: "Criei a consulta para amanhã às 10h.", acoes: [{ tipo: "criar", tarefa: { titulo: "Consulta cardiologista", hora: "10:00", categoria: "saude" } }] })
      : "{}");
    act(() => abrirPainelDeTarefas());
    render(<PainelDeTarefas onAbrirAjustes={vi.fn()} />);
    await userEvent.type(screen.getByLabelText("Mensagem para o assistente"), "marca cardiologista amanhã 10h{Enter}");
    expect(await screen.findByText("Criei a consulta para amanhã às 10h.")).toBeInTheDocument();
    expect(screen.getByText("✓ Criada: Consulta cardiologista")).toBeInTheDocument();
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("roadmaps_ia", expect.objectContaining({ pedido: expect.objectContaining({ chave: "tarefas-assistente", web: false }) }));
    const criada = () => (pasta.listar(null, "tarefas")).then((l) => l.map((x) => JSON.parse(x) as Tarefa).find((t) => t.titulo === "Consulta cardiologista"));
    expect((await criada())?.categoria).toBe("saude");
    await userEvent.click(screen.getByRole("button", { name: "Desfazer" }));
    await act(async () => { await Promise.resolve(); });
    expect((await criada())?.apagada).toBe(true);
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue("{}");
  });
});
