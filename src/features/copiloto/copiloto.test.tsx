import { beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { BotaoDasTarefas } from "../tarefas/Flutuantes";
import { reiniciarPainelParaTeste } from "../tarefas/painel";
import { CHAVE_DO_COPILOTO, ehConversaDoCopiloto, mudarCopiloto, reiniciarCopilotoParaTeste, useCopiloto } from "./copiloto";
import { JanelaDoCopiloto } from "./JanelaDoCopiloto";
import { encontrarSessoesOrfas } from "../workspace/recuperarWorkspace";
import type { TerminalSession } from "../terminal/terminalService";

beforeEach(() => {
  localStorage.clear();
  reiniciarCopilotoParaTeste();
  reiniciarPainelParaTeste();
});

function Estado() { const c = useCopiloto(); return <span data-testid="c">{c.aberto ? "aberto" : "fechado"}/{c.modo}</span>; }

describe("copiloto", () => {
  it("a logo abre e fecha a janela; ela prende à direita e volta a flutuar", async () => {
    render(<><BotaoDasTarefas /><JanelaDoCopiloto /><Estado /></>);
    await userEvent.click(screen.getByRole("button", { name: "Copiloto" }));
    const janela = await screen.findByRole("complementary", { name: "Copiloto" });
    expect(janela.className).toContain("copiloto-flutuante");
    expect(screen.getByText(/já abre com as ferramentas do app/)).toBeInTheDocument();
    await userEvent.click(screen.getByTitle("Prender à direita"));
    expect(screen.getByTestId("c")).toHaveTextContent("aberto/direita");
    expect(screen.getByRole("complementary", { name: "Copiloto" }).className).toContain("copiloto-direita");
    await userEvent.click(screen.getByRole("button", { name: /fechar o copiloto/i }));
    expect(screen.getByTestId("c")).toHaveTextContent("fechado/direita");
    expect(JSON.parse(localStorage.getItem(CHAVE_DO_COPILOTO) ?? "{}").modo).toBe("direita");
  });

  it("'Nova conversa' só com uma conversa aberta e volta para o lançador", async () => {
    mudarCopiloto({ aberto: true, sessao: "s-1", provider: "claude", pasta: "/tmp" });
    render(<><JanelaDoCopiloto /></>);
    const nova = screen.getByTitle(/nova conversa/i);
    expect(nova).toBeEnabled();
    await userEvent.click(nova);
    expect(JSON.parse(localStorage.getItem(CHAVE_DO_COPILOTO) ?? "{}").sessao).toBe("");
  });

  it("a conversa do copiloto não vira 'agente fora do workspace'", () => {
    const sessao = { id: "s", provider: "claude", state: "working", cwd: "/Users/x", project_id: "copiloto", name: "Copiloto" } as unknown as TerminalSession;
    expect(ehConversaDoCopiloto("copiloto")).toBe(true);
    expect(encontrarSessoesOrfas([sessao], [])).toEqual([]);
    // A mesma sessão em outro projeto seria oferecida para adotar (o teste não passa por acaso).
    expect(encontrarSessoesOrfas([{ ...sessao, project_id: "p" } as TerminalSession], [])).toHaveLength(1);
  });
});
