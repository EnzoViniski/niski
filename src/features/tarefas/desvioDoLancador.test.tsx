// Fork (revisão do copiloto): o lançador do canto não cobre o campo de mensagem nem a conversa do editor de vídeo.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { desviarLancador, type Caixa } from "./desvioDoLancador";
import { BotaoDasTarefas } from "./Flutuantes";
import { reiniciarPainelParaTeste } from "./painel";
import { reiniciarCopilotoParaTeste } from "../copiloto/copiloto";

const janela = { w: 1512, h: 945 };
const caixa = (left: number, top: number, right: number, bottom: number): Caixa => ({ left, top, right, bottom });

describe("desviarLancador", () => {
  it("sem obstáculo fica onde está", () => {
    expect(desviarLancador({ right: 20, bottom: 64 }, 48, 100, [], janela)).toEqual({ right: 20, bottom: 64 });
  });

  it("campo de mensagem embaixo: sobe para cima dele", () => {
    // Composer de uma aba de agente ocupando a faixa de baixo, até a borda direita.
    const composer = caixa(300, 800, 1500, 930);
    const r = desviarLancador({ right: 20, bottom: 64 }, 48, 100, [composer], janela);
    expect(r).toEqual({ right: 20, bottom: janela.h - 800 + 8 });
  });

  it("coluna alta (conversa do editor de vídeo): vai para a esquerda dela, mesmo com o composer dentro", () => {
    const coluna = caixa(1120, 40, 1512, 945);
    const composerDaColuna = caixa(1130, 820, 1500, 935);
    const r = desviarLancador({ right: 20, bottom: 64 }, 48, 100, [composerDaColuna, coluna], janela);
    expect(r).toEqual({ right: janela.w - 1120 + 8, bottom: 64 });
  });

  it("layout dividido: sobe acima de um e não cai no outro", () => {
    const baixo = caixa(800, 820, 1500, 930);
    const meio = caixa(800, 660, 1500, 780);
    const r = desviarLancador({ right: 20, bottom: 64 }, 48, 100, [baixo, meio], janela);
    const topo = janela.h - r.bottom - 100;
    expect(janela.h - r.bottom).toBeLessThanOrEqual(660 - 8);
    expect(topo).toBeGreaterThanOrEqual(8);
  });

  it("janela pequena (1280×800) e sem lugar acima: fica dentro da janela", () => {
    const enorme = caixa(0, 30, 1280, 790);
    const r = desviarLancador({ right: 20, bottom: 64 }, 48, 100, [enorme], { w: 1280, h: 800 });
    expect(r.right).toBeLessThanOrEqual(1280 - 48 - 8);
    expect(r.bottom).toBeLessThanOrEqual(800 - 100 - 8);
  });

  it("ignora obstáculos escondidos (caixa vazia)", () => {
    expect(desviarLancador({ right: 20, bottom: 64 }, 48, 100, [caixa(0, 0, 0, 0)], janela)).toEqual({ right: 20, bottom: 64 });
  });
});

describe("BotaoDasTarefas", () => {
  beforeEach(() => { localStorage.clear(); reiniciarPainelParaTeste(); reiniciarCopilotoParaTeste(); });
  afterEach(() => { document.querySelectorAll(".composer").forEach((c) => c.remove()); });

  it("sobe para cima do campo de mensagem que está embaixo dele", async () => {
    const composer = document.createElement("form");
    composer.className = "composer";
    document.body.appendChild(composer);
    const H = window.innerHeight, W = window.innerWidth;
    vi.spyOn(composer, "getBoundingClientRect").mockReturnValue({ left: 100, top: H - 140, right: W - 10, bottom: H - 10, width: W - 110, height: 130, x: 100, y: H - 140, toJSON: () => ({}) });
    render(<BotaoDasTarefas />);
    const lancador = screen.getByRole("button", { name: "Copiloto" }).closest(".tarefas-lancador") as HTMLElement;
    await act(async () => { await Promise.resolve(); });
    expect(Number.parseFloat(lancador.style.bottom)).toBe(140 + 8);
    expect(lancador.style.right).toBe("20px");
  });
});
