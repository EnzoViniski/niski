import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { duracaoDoTrabalho, turnosRecolhidos } from "../mobile/conversa/turnos";
import { SeletorDeModelo } from "../mobile/conversa/SeletorDeModelo";
import type { Bloco } from "../mobile/conversa/useTimeline";

/*
 * Fork (21/10/2026): trabalho do pedido recolhido em "Trabalhou por …" e seletor de modelo/esforço no
 * estilo do Codex.
 */

let n = 0;
const fala = (role: string, quando: string): Bloco => ({ tipo: "fala", mensagem: { id: `m${++n}`, role, text: "x", provider: "claude", timestamp: quando } });
const acoes = (quando: string): Bloco => ({ tipo: "acoes", id: `a${++n}`, acoes: [{ id: `t${n}`, role: "assistant", text: "ls", provider: "claude", kind: "tool", tool: "Bash", timestamp: quando }] });

describe("turnos recolhidos", () => {
  it("turno terminado: recolhe tudo entre a sua mensagem e a resposta final, com a duração", () => {
    const blocos = [fala("user", "2026-10-21T10:00:00Z"), fala("assistant", "2026-10-21T10:01:00Z"), acoes("2026-10-21T10:05:00Z"),
      fala("assistant", "2026-10-21T10:20:34Z")];
    const [r] = turnosRecolhidos(blocos, false);
    expect(r).toMatchObject({ inicio: 1, fim: 2, final: 3, duracaoMs: 20 * 60_000 + 34_000 });
    expect(duracaoDoTrabalho(r.duracaoMs!)).toBe("20m 34s");
  });

  it("turno em andamento fica todo à vista; turnos anteriores recolhem", () => {
    const blocos = [fala("user", "2026-10-21T10:00:00Z"), acoes("2026-10-21T10:00:10Z"), fala("assistant", "2026-10-21T10:00:45Z"),
      fala("user", "2026-10-21T10:02:00Z"), acoes("2026-10-21T10:02:10Z"), fala("assistant", "2026-10-21T10:02:30Z")];
    const recolhidos = turnosRecolhidos(blocos, true);
    expect(recolhidos).toHaveLength(1);
    expect(recolhidos[0].final).toBe(2);
  });

  it("resposta direta, sem trabalho no meio, não recolhe nada; sem resposta também não", () => {
    expect(turnosRecolhidos([fala("user", "2026-10-21T10:00:00Z"), fala("assistant", "2026-10-21T10:00:05Z")], false)).toEqual([]);
    expect(turnosRecolhidos([fala("user", "2026-10-21T10:00:00Z"), acoes("2026-10-21T10:00:05Z")], false)).toEqual([]);
  });

  it("duração em formato curto", () => {
    expect(duracaoDoTrabalho(45_000)).toBe("45s");
    expect(duracaoDoTrabalho(3_900_000)).toBe("1h 05m");
  });
});

describe("seletor de modelo e esforço", () => {
  const modelos = [{ id: "opus", label: "Opus 4.8", efforts: ["low", "medium", "high", "max"] }, { id: "sonnet", label: "Sonnet 5.5", efforts: ["low", "high"] }];

  it("mostra modelo e esforço no botão; a barra troca o esforço e o máximo ganha brilho", async () => {
    const onEsforco = vi.fn();
    const { rerender } = render(<SeletorDeModelo modelos={modelos} modelo="opus" esforco="high" esforcos={modelos[0].efforts} onModelo={vi.fn()} onEsforco={onEsforco} />);
    const gatilho = screen.getByRole("button", { name: /Opus 4.8/ });
    expect(gatilho).toHaveTextContent("Alto");
    await userEvent.click(gatilho);
    const barra = screen.getByRole("slider", { name: "Esforço do raciocínio" });
    fireEvent.keyDown(barra, { key: "End" });
    expect(onEsforco).toHaveBeenCalledWith("max");
    fireEvent.keyDown(barra, { key: "ArrowLeft" });
    expect(onEsforco).toHaveBeenLastCalledWith("medium");
    rerender(<SeletorDeModelo modelos={modelos} modelo="opus" esforco="max" esforcos={modelos[0].efforts} onModelo={vi.fn()} onEsforco={onEsforco} />);
    expect(document.querySelector(".barra-esforco")).toHaveClass("is-maximo");
    expect(document.querySelectorAll(".barra-estrela").length).toBeGreaterThan(5);
  });

  it("o nome do modelo abre a lista; escolher volta ao esforço; o ↺ volta ao padrão", async () => {
    const onModelo = vi.fn();
    const onEsforco = vi.fn();
    render(<SeletorDeModelo modelos={modelos} modelo="opus" esforco="high" esforcos={modelos[0].efforts} onModelo={onModelo} onEsforco={onEsforco} />);
    await userEvent.click(screen.getByRole("button", { name: /Opus 4.8/ }));
    // O bloco do esforço + modelo inteiro é clicável.
    await userEvent.click(document.querySelector<HTMLButtonElement>(".seletor-cabecalho")!);
    expect(screen.getByRole("option", { name: /Opus 4.8/ })).toHaveAttribute("aria-selected", "true");
    await userEvent.click(screen.getByRole("option", { name: "Sonnet 5.5" }));
    expect(onModelo).toHaveBeenCalledWith("sonnet");
    expect(await screen.findByRole("slider")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Voltar ao padrão" }));
    expect(onModelo).toHaveBeenLastCalledWith("");
    expect(onEsforco).toHaveBeenLastCalledWith("");
  });

  it("desativado (agente trabalhando) não abre", async () => {
    render(<SeletorDeModelo modelos={modelos} modelo="opus" esforco="" esforcos={modelos[0].efforts} desativado motivo="Espere" onModelo={vi.fn()} onEsforco={vi.fn()} />);
    expect(screen.getByRole("button", { name: /Opus 4.8/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Opus 4.8/ })).toHaveTextContent("Padrão");
  });
});

it("fechar o painel avisa (é quando o ajuste é aplicado)", async () => {
  const onFechar = vi.fn();
  const modelos = [{ id: "opus", label: "Opus 4.8", efforts: ["low", "high"] }];
  render(<SeletorDeModelo modelos={modelos} modelo="opus" esforco="low" esforcos={modelos[0].efforts} onModelo={vi.fn()} onEsforco={vi.fn()} onFechar={onFechar} />);
  await userEvent.click(screen.getByRole("button", { name: /Opus 4.8/ }));
  expect(onFechar).not.toHaveBeenCalled();
  await userEvent.keyboard("{Escape}");
  expect(onFechar).toHaveBeenCalledTimes(1);
});
