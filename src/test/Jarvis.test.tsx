import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { JanelaDoJarvis } from "../features/jarvis/JanelaDoJarvis";
import { BotaoDoJarvis } from "../features/jarvis/BotaoDoJarvis";
import { aplicarEvento, estadoInicial, type EventoDoJarvis } from "../features/jarvis/estadoDoJarvis";

type Ouvinte = (e: { payload: unknown }) => void;
let ouvintes: Record<string, Ouvinte> = {};
beforeEach(() => {
  ouvintes = {};
  vi.mocked(invoke).mockReset().mockResolvedValue(null);
  vi.mocked(listen).mockReset().mockImplementation(async (nome, ouvinte) => {
    ouvintes[String(nome)] = ouvinte as Ouvinte;
    return () => undefined;
  });
});
const emitir = (evento: EventoDoJarvis) => act(() => ouvintes.jarvis({ payload: evento }));

describe("estado do Jarvis", () => {
  it("monta a conversa: fala do Enzo, resposta aos pedaços e fim", () => {
    const eventos: EventoDoJarvis[] = [
      { tipo: "voce_disse", texto: "oi" },
      { tipo: "resposta", texto: "Oi " },
      { tipo: "resposta", texto: "Enzo." },
      { tipo: "resposta_fim" },
    ];
    const estado = eventos.reduce(aplicarEvento, estadoInicial);
    expect(estado.falas.map(({ quem, texto, aberta }) => ({ quem, texto, aberta }))).toEqual([
      { quem: "voce", texto: "oi", aberta: undefined },
      { quem: "jarvis", texto: "Oi Enzo.", aberta: false },
    ]);
  });

  it("interrupção marca a resposta como cortada e a próxima resposta é outra fala", () => {
    const estado = ([
      { tipo: "resposta", texto: "Uma coisa longa" },
      { tipo: "interrompido" },
      { tipo: "resposta", texto: "Outra" },
    ] as EventoDoJarvis[]).reduce(aplicarEvento, estadoInicial);
    expect(estado.falas).toHaveLength(2);
    expect(estado.falas[0]).toMatchObject({ cortada: true, aberta: false });
    expect(estado.falas[1]).toMatchObject({ texto: "Outra", aberta: true });
  });

  it("fala nova limpa a transcrição parcial e o erro", () => {
    let estado = aplicarEvento(estadoInicial, { tipo: "parcial", texto: "abre a" });
    estado = aplicarEvento(estado, { tipo: "erro", msg: "x" });
    estado = aplicarEvento(estado, { tipo: "voce_disse", texto: "abre a conversa" });
    expect(estado.parcial).toBe("");
    expect(estado.erro).toBeNull();
  });

  it("guarda só as últimas falas", () => {
    let estado = estadoInicial;
    for (let i = 0; i < 60; i++) estado = aplicarEvento(estado, { tipo: "voce_disse", texto: `fala ${i}` });
    expect(estado.falas).toHaveLength(40);
    expect(estado.falas[39].texto).toBe("fala 59");
  });
});

describe("janela do Jarvis", () => {
  it("liga ao abrir e mostra transcrição ao vivo e resposta", async () => {
    render(<JanelaDoJarvis />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("jarvis_iniciar"));
    await waitFor(() => expect(ouvintes.jarvis).toBeDefined());
    emitir({ tipo: "estado", estado: "ouvindo" });
    expect(screen.getByRole("status")).toHaveTextContent("Ouvindo");
    emitir({ tipo: "parcial", texto: "que horas" });
    expect(screen.getByText("que horas")).toHaveClass("is-parcial");
    emitir({ tipo: "voce_disse", texto: "que horas são" });
    emitir({ tipo: "resposta", texto: "São dez da noite." });
    expect(screen.getByText("que horas são")).toBeInTheDocument();
    expect(screen.getByText("São dez da noite.")).toBeInTheDocument();
  });

  it("Esc e o botão param a fala; microfone pausa e volta", async () => {
    render(<JanelaDoJarvis />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith("jarvis_calar");
    await userEvent.click(screen.getByRole("button", { name: "Pausar o microfone" }));
    expect(invoke).toHaveBeenCalledWith("jarvis_ouvir", { ligado: false });
    await userEvent.click(screen.getByRole("button", { name: "Ligar o microfone" }));
    expect(invoke).toHaveBeenCalledWith("jarvis_ouvir", { ligado: true });
  });

  it("escrever manda o texto e limpa o campo", async () => {
    render(<JanelaDoJarvis />);
    const campo = screen.getByRole("textbox", { name: "Escrever para o Jarvis" });
    await userEvent.type(campo, "resume o dia{Enter}");
    expect(invoke).toHaveBeenCalledWith("jarvis_escrever", { texto: "resume o dia" });
    expect(campo).toHaveValue("");
  });

  it("erro ao ligar aparece com tentar de novo", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === "jarvis_iniciar") throw "O Jarvis precisa do macOS 26 ou mais novo";
      return null;
    });
    render(<JanelaDoJarvis />);
    expect(await screen.findByRole("alert")).toHaveTextContent("macOS 26");
    await userEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    expect(vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "jarvis_iniciar")).toHaveLength(2);
  });

  it("encolhe para bolinha pelo botão", async () => {
    render(<JanelaDoJarvis />);
    await userEvent.click(screen.getByRole("button", { name: "Encolher para bolinha" }));
    expect(invoke).toHaveBeenCalledWith("jarvis_bolinha", { ligada: true });
  });

  it("abre a equipe do Jarvis no app principal", async () => {
    render(<JanelaDoJarvis />);
    await userEvent.click(screen.getByRole("button", { name: "Ver agentes do Jarvis" }));
    expect(invoke).toHaveBeenCalledWith("jarvis_abrir_fluxo");
  });
});

describe("botão do Jarvis na barra", () => {
  it("traduz o pedido da janela flutuante para abrir o fluxo da equipe", async () => {
    const receber = vi.fn();
    window.addEventListener("medza-orquestracao", receber);
    try {
      render(<BotaoDoJarvis />);
      await waitFor(() => expect(ouvintes["jarvis-abrir-fluxo"]).toBeDefined());
      act(() => ouvintes["jarvis-abrir-fluxo"]({ payload: null }));
      expect((receber.mock.calls[0][0] as CustomEvent).detail).toEqual({ tipo: "abrir-fluxo", orquestrador: "jarvis-medza", titulo: "Equipe do Jarvis", paneId: undefined });
    } finally { window.removeEventListener("medza-orquestracao", receber); }
  });

  it("abre e fecha a janela e acende quando ela está aberta", async () => {
    render(<BotaoDoJarvis />);
    const botao = screen.getByRole("button", { name: "Abrir o Jarvis" });
    await userEvent.click(botao);
    expect(invoke).toHaveBeenCalledWith("jarvis_alternar");
    await waitFor(() => expect(ouvintes["jarvis-janela"]).toBeDefined());
    act(() => ouvintes["jarvis-janela"]({ payload: { aberta: true } }));
    expect(screen.getByRole("button", { name: "Fechar o Jarvis" })).toHaveAttribute("aria-pressed", "true");
  });
});
