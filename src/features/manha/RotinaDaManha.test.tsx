import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RotinaDaManha } from "./RotinaDaManha";
import { estadoDaManha, recarregarDaManha, salvarAjustesDaManha } from "./rotina";
import { reiniciarParaTeste, transporteEmMemoria, usarTransporteDasTarefas } from "../tarefas/loja";

beforeEach(() => {
  localStorage.clear();
  recarregarDaManha();
  usarTransporteDasTarefas(transporteEmMemoria());
  reiniciarParaTeste();
});

describe("Aquecimento da manhã", () => {
  it("responde todas, mostra o resumo e grava o dia no histórico", async () => {
    salvarAjustesDaManha({ quantidade: 2, temas: ["aritmetica"] });
    const fechar = vi.fn();
    render(<RotinaDaManha onFechar={fechar} onAbrirTarefas={() => {}} />);
    for (let i = 0; i < 2; i++) {
      await screen.findByText(`${i + 1} de 2`);
      await userEvent.click(screen.getAllByRole("button", { pressed: false }).find((b) => b.className.includes("manha-opcao")) as HTMLElement);
      expect(screen.getByRole("status")).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: i === 1 ? "Ver resultado" : /próxima/i }));
    }
    expect(await screen.findByText(/dia seguido|dias seguidos/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Começar o dia" }));
    expect(fechar).toHaveBeenCalled();
    expect(estadoDaManha().historico).toHaveLength(1);
    expect(estadoDaManha().sessao?.fim).not.toBe("");
  });

  it("teclado: A–D responde", async () => {
    salvarAjustesDaManha({ quantidade: 1, temas: ["logica"] });
    render(<RotinaDaManha onFechar={() => {}} onAbrirTarefas={() => {}} />);
    await screen.findByText("1 de 1");
    await userEvent.keyboard("b");
    expect(estadoDaManha().sessao?.respostas[0]).toBe(1);
  });

  it("'Agora não' adia e fecha", async () => {
    const fechar = vi.fn();
    render(<RotinaDaManha onFechar={fechar} onAbrirTarefas={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: "Agora não" }));
    expect(fechar).toHaveBeenCalled();
    expect(estadoDaManha().sessao?.adiadaAte).not.toBe("");
  });
});
