import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FilaDeContas } from "./FilaDeContas";
import { gravarFila, lerFila, idadeDoDado, moverNaFila, resumoDaCota, type EstadoDaFila, type Politica } from "./roteamentoService";

// Fork (3B): tela da fila de contas com o engine falso (só as duas rotas; o texto da cota é o de verdade).
vi.mock("./roteamentoService", async (original) => ({
  ...(await original<typeof import("./roteamentoService")>()),
  lerFila: vi.fn(),
  gravarFila: vi.fn(),
}));

const AGORA = 1_800_000_000_000;

function estado(politica: Partial<Politica> = {}): EstadoDaFila {
  const final: Politica = { modo: "fila", etapas: ["b", "a"], contas: {}, projetos_confidenciais: [], usar_em_fluxos: false, ...politica };
  return {
    politica: final,
    agora_ms: AGORA,
    contas: [
      { id: "a", nome: "Pessoal", provider: "claude", na_fila: final.etapas.includes("a"), ajustes: final.contas.a ?? { tipo: "assinatura" }, sessoes: 0,
        cota: { conhecida: true, fonte: "claude_statusline", observado_em_ms: AGORA - 120_000, janelas: [{ nome: "5h", percentual: 30, reset_em_ms: AGORA + 3_600_000 }] } },
      { id: "b", nome: "Empresa", provider: "claude", na_fila: final.etapas.includes("b"), ajustes: final.contas.b ?? { tipo: "assinatura" }, sessoes: 2,
        cota: { conhecida: false, fonte: "claude_cache", observado_em_ms: null, janelas: [] } },
      { id: "x", nome: "Codex", provider: "codex", na_fila: final.etapas.includes("x"), ajustes: { tipo: "assinatura" }, sessoes: 0,
        cota: { conhecida: false, fonte: "codex_rollout", observado_em_ms: null, janelas: [] } },
    ],
    escolhas: {
      claude: { perfil_id: "b", nome: "Empresa", explicacao: ["Empresa (b): escolhida — primeira da fila que passou nos filtros (cota desconhecida)"] },
      codex: { perfil_id: "x", nome: "Codex", explicacao: ["a fila não tem contas de codex: valem as contas cadastradas, na ordem"] },
    },
  };
}

beforeEach(() => {
  vi.mocked(lerFila).mockReset().mockResolvedValue(estado());
  vi.mocked(gravarFila).mockReset().mockImplementation(async (politica) => estado(politica));
});

describe("FilaDeContas", () => {
  it("mostra a fila na ordem, a cota com fonte e idade e o porquê de cada plataforma", async () => {
    render(<FilaDeContas />);
    const fila = await screen.findByRole("list", { name: "Contas na fila" });
    const linhas = within(fila).getAllByRole("listitem");
    expect(linhas.map((linha) => within(linha).getByText(/Empresa|Pessoal/).textContent)).toEqual(["Empresa", "Pessoal"]);
    expect(within(linhas[0]).getByText(/cota desconhecida · cache do \/usage do Claude, idade desconhecida · 2 sessões abertas/)).toBeInTheDocument();
    expect(within(linhas[1]).getByText("5h 30% · statusline do Claude, há 2 min")).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "Contas fora da fila" })).getByText("Codex")).toBeInTheDocument();
    expect(screen.getByText(/Sem conta de Codex na fila/)).toBeInTheDocument();
    expect(screen.getByText(/Claude → Empresa/)).toBeInTheDocument();
    expect(screen.getByText(/primeira da fila que passou nos filtros/)).toBeInTheDocument();
  });

  it("reordena, troca o modo e põe conta na fila gravando a política inteira", async () => {
    render(<FilaDeContas />);
    fireEvent.click(await screen.findByRole("button", { name: "Descer Empresa" }));
    await waitFor(() => expect(gravarFila).toHaveBeenLastCalledWith(expect.objectContaining({ etapas: ["a", "b"] })));

    fireEvent.click(screen.getByRole("button", { name: "Automático (menor uso)" }));
    await waitFor(() => expect(gravarFila).toHaveBeenLastCalledWith(expect.objectContaining({ modo: "auto", etapas: ["a", "b"] })));

    fireEvent.click(screen.getByRole("button", { name: "Pôr Codex na fila" }));
    await waitFor(() => expect(gravarFila).toHaveBeenLastCalledWith(expect.objectContaining({ etapas: ["a", "b", "x"] })));

    fireEvent.click(screen.getByRole("button", { name: "Tirar Pessoal da fila" }));
    await waitFor(() => expect(gravarFila).toHaveBeenLastCalledWith(expect.objectContaining({ etapas: ["b", "x"] })));
  });

  it("ajustes da conta: uso extra, orçamento ao sair do campo e confidencial", async () => {
    render(<FilaDeContas />);
    const linha = (await screen.findByRole("button", { name: "Descer Empresa" })).closest("li")!;
    fireEvent.click(within(linha).getByRole("button", { name: "Ajustes" }));
    fireEvent.click(within(linha).getByRole("checkbox", { name: /Aceita uso extra/ }));
    await waitFor(() => expect(gravarFila).toHaveBeenLastCalledWith(expect.objectContaining({ contas: { b: { tipo: "assinatura", uso_extra: true } } })));

    const orcamento = within(linha).getByLabelText("Orçamento do uso extra (US$)");
    fireEvent.change(orcamento, { target: { value: "25,5" } });
    expect(gravarFila).toHaveBeenCalledTimes(1);
    fireEvent.blur(orcamento);
    await waitFor(() => expect(gravarFila).toHaveBeenLastCalledWith(expect.objectContaining({
      contas: { b: expect.objectContaining({ uso_extra: true, orcamento_usd: 25.5 }) } })));

    // Número inválido não vai para o engine.
    fireEvent.change(within(linha).getByLabelText(/Sessões simultâneas/), { target: { value: "-2" } });
    fireEvent.blur(within(linha).getByLabelText(/Sessões simultâneas/));
    expect(gravarFila).toHaveBeenCalledTimes(2);

    fireEvent.click(within(linha).getByRole("checkbox", { name: /Pode receber projeto confidencial/ }));
    await waitFor(() => expect(gravarFila).toHaveBeenLastCalledWith(expect.objectContaining({
      contas: { b: expect.objectContaining({ confidencial_ok: true }) } })));
  });

  it("pastas confidenciais e fluxos; erro do engine aparece e a tela relê", async () => {
    render(<FilaDeContas />);
    const pastas = await screen.findByLabelText(/Projetos confidenciais/);
    fireEvent.change(pastas, { target: { value: " /empresa \n\n/cliente " } });
    fireEvent.blur(pastas);
    await waitFor(() => expect(gravarFila).toHaveBeenLastCalledWith(expect.objectContaining({ projetos_confidenciais: ["/empresa", "/cliente"] })));

    vi.mocked(gravarFila).mockRejectedValueOnce(new Error("orçamento e gasto são valores em US$ a partir de 0"));
    fireEvent.click(screen.getByRole("checkbox", { name: /Usar a fila nos fluxos/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("a partir de 0");
    await waitFor(() => expect(lerFila).toHaveBeenCalledTimes(2));
  });

  it("engine antigo sem a rota", async () => {
    vi.mocked(lerFila).mockRejectedValue(new Error("404"));
    render(<FilaDeContas />);
    expect(await screen.findByRole("alert")).toHaveTextContent(/reinicie o app/);
  });
});

describe("roteamentoService", () => {
  it("cota: janela reiniciada, desconhecida sem fonte e idade em horas/dias", () => {
    expect(resumoDaCota({ conhecida: true, fonte: "codex_rollout", observado_em_ms: AGORA - 3 * 3_600_000,
      janelas: [{ nome: "5h", percentual: 100, reset_em_ms: AGORA - 1 }, { nome: "semanal", percentual: 70.4, reset_em_ms: null }] }, AGORA))
      .toBe("5h reiniciada · semanal 70% · registro do Codex, há 3 h");
    expect(resumoDaCota({ conhecida: false, fonte: null, observado_em_ms: null, janelas: [] }, AGORA)).toBe("cota desconhecida · sem fonte");
    expect(idadeDoDado(AGORA - 3 * 86_400_000, AGORA)).toBe("há 3 dias");
  });

  it("mover na fila não sai dos limites", () => {
    expect(moverNaFila(["a", "b", "c"], "c", 1)).toEqual(["a", "b", "c"]);
    expect(moverNaFila(["a", "b", "c"], "c", -1)).toEqual(["a", "c", "b"]);
    expect(moverNaFila(["a"], "z", 1)).toEqual(["a"]);
  });
});
