import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CartaoDePermissao } from "../mobile/conversa/CartaoDePermissao";
import { formatarRestante } from "../mobile/conversa/pedidoDoGancho";
import { textoDoContexto } from "../mobile/conversa/ChipDeContexto";
import { usarTransporte, type PedidoDoGancho } from "../mobile/api";

// Fork (Onda 2, item 2): cartão de permissão com o pedido segurado pelo gancho do Claude.
let chamadas: { caminho: string; corpo: unknown }[] = [];
let status = 200;
beforeEach(() => {
  chamadas = [];
  status = 200;
  usarTransporte(async (caminho, init) => {
    chamadas.push({ caminho, corpo: init.body ? JSON.parse(String(init.body)) : null });
    return new Response(JSON.stringify(status === 200 ? { state: "sent" } : { error: "x" }), { status });
  });
});
afterEach(() => usarTransporte((path, init) => fetch(path, init)));

const pedido = (parcial: Partial<PedidoDoGancho> = {}): PedidoDoGancho =>
  ({ id: "ped-1", origem: "gancho", ferramenta: "Bash", resumo: "Bash · npm test", expira_em_ms: Date.now() + 275_000, ...parcial });

describe("cartão de permissão", () => {
  it("sem pedido do gancho: os dois botões de sempre (tecla na tela)", () => {
    const responder = vi.fn();
    render(<CartaoDePermissao texto="Do you want to proceed?" busy={false} onResponder={responder} />);
    fireEvent.click(screen.getByRole("button", { name: "Permitir" }));
    fireEvent.click(screen.getByRole("button", { name: "Negar" }));
    expect(responder.mock.calls).toEqual([[true], [false]]);
    expect(screen.queryByText("Responder no terminal")).toBeNull();
  });

  it("pedido do gancho: ferramenta, prazo e Permitir com o id", async () => {
    const respondido = vi.fn();
    render(<CartaoDePermissao busy={false} onResponder={vi.fn()} pedido={pedido({ arriscado: "apaga arquivos em massa" })} conversaId="conv 1" onRespondido={respondido} />);
    expect(screen.getByText(/Aprovação pendente · Bash/)).toBeInTheDocument();
    expect(screen.getByText("Bash · npm test")).toBeInTheDocument();
    expect(screen.getByText(/apaga arquivos em massa/)).toBeInTheDocument();
    expect(screen.getByText(/Vai para o terminal em 4:3\d/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Permitir" }));
    await waitFor(() => expect(respondido).toHaveBeenCalled());
    expect(chamadas).toEqual([{ caminho: "/conversas/conv%201/aprovar", corpo: { permitir: true, pedido: "ped-1" } }]);
  });

  it("negar com o que fazer, e responder no terminal", async () => {
    render(<CartaoDePermissao busy={false} onResponder={vi.fn()} pedido={pedido()} conversaId="c" />);
    fireEvent.click(screen.getByRole("button", { name: "Negar…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Dizer o que fazer" }), "rode só o lint");
    fireEvent.click(screen.getByRole("button", { name: "Negar" }));
    await waitFor(() => expect(chamadas).toHaveLength(1));
    expect(chamadas[0].corpo).toEqual({ permitir: false, pedido: "ped-1", mensagem: "rode só o lint" });
    fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
    fireEvent.click(screen.getByRole("button", { name: "Responder no terminal" }));
    await waitFor(() => expect(chamadas).toHaveLength(2));
    expect(chamadas[1]).toEqual({ caminho: "/conversas/c/pedido/terminal", corpo: { pedido: "ped-1" } });
  });

  it("pedido mudou (412): avisa e relê", async () => {
    status = 412;
    const respondido = vi.fn();
    render(<CartaoDePermissao busy={false} onResponder={vi.fn()} pedido={pedido()} conversaId="c" onRespondido={respondido} />);
    fireEvent.click(screen.getByRole("button", { name: "Permitir" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("O pedido mudou");
    expect(respondido).toHaveBeenCalled();
  });

  it("prazo vencido e contagem formatada", () => {
    render(<CartaoDePermissao busy={false} onResponder={vi.fn()} pedido={pedido({ expira_em_ms: Date.now() - 1 })} conversaId="c" />);
    expect(screen.getByText("Indo para o terminal…")).toBeInTheDocument();
    expect(formatarRestante(275)).toBe("4:35");
    expect(formatarRestante(9)).toBe("0:09");
  });

  it("chip de contexto da statusline", () => {
    expect(textoDoContexto({ estado: "ativos", contexto: { pct: 34.4, custo_usd: 0.82, modelo: "Opus" } })).toBe("Contexto 34% · ~US$ 0,82");
    expect(textoDoContexto({ estado: "ativos", contexto: null })).toBeNull();
    expect(textoDoContexto(null)).toBeNull();
  });
});
