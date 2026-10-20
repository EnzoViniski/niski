import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { GanchosDoClaude } from "../features/agentes/GanchosDoClaude";
import { usarTransporte } from "../mobile/api";

// Fork (Onda 2): Configurações → Agentes: eventos do Claude Code, prazo e statusline por conta.
let estado: Record<string, unknown>;
let posts: unknown[] = [];
beforeEach(() => {
  posts = [];
  estado = { ligado: true, espera_s: 300, esperas: [120, 300, 600], contas: [{ id: "claude-p", nome: "Trabalho", statusline: false }] };
  usarTransporte(async (_caminho, init) => {
    if (init.method === "POST") {
      const pedido = JSON.parse(String(init.body));
      posts.push(pedido);
      if ("ligado" in pedido) estado = { ...estado, ligado: pedido.ligado };
      if ("espera_s" in pedido) estado = { ...estado, espera_s: pedido.espera_s };
      if ("statusline" in pedido) estado = { ...estado, contas: [{ id: "claude-p", nome: "Trabalho", statusline: pedido.statusline.ligado }] };
    }
    return new Response(JSON.stringify(estado), { status: 200 });
  });
});
afterEach(() => usarTransporte((path, init) => fetch(path, init)));

describe("ganchos do Claude nas Configurações", () => {
  it("padrão ligado; desligar, trocar o prazo e ligar a statusline de uma conta", async () => {
    render(<GanchosDoClaude />);
    const ligar = await screen.findByRole("checkbox", { name: "Receber eventos do Claude Code" });
    await waitFor(() => expect(ligar).toBeChecked());
    expect(screen.getByRole("radio", { name: "5 min" })).toBeChecked();
    fireEvent.click(screen.getByRole("radio", { name: "10 min" }));
    await waitFor(() => expect(screen.getByRole("radio", { name: "10 min" })).toBeChecked());
    fireEvent.click(screen.getByRole("checkbox", { name: "Trabalho" }));
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "Trabalho" })).toBeChecked());
    fireEvent.click(ligar);
    await waitFor(() => expect(ligar).not.toBeChecked());
    expect(posts).toEqual([{ espera_s: 600 }, { statusline: { conta: "claude-p", ligado: true } }, { ligado: false }]);
    expect(screen.getByRole("radio", { name: "10 min" })).toBeDisabled();
  });

  it("engine antigo: avisa", async () => {
    usarTransporte(async () => new Response("{}", { status: 404 }));
    render(<GanchosDoClaude />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Engine antigo");
  });
});
