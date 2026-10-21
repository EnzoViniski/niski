import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { aplicarTema } from "../mobile/tema";
import { lerRota, urlDaRota } from "../mobile/rota";
import { Uso } from "../mobile/telas/Uso";

/*
 * Fork: o celular segue o estilo do PC (Clássico/Minimalista, publicado junto com o tema) e ganhou a
 * tela "Uso das contas" (`#/uso`), com o mesmo painel da web.
 */

afterEach(() => {
  vi.unstubAllGlobals();
  delete document.documentElement.dataset.estilo;
});

describe("estilo no celular", () => {
  it("PC no Minimalista: o celular põe data-estilo=minimalista", () => {
    aplicarTema({ accent: "laranja", theme: "escuro", background: "pixel-grid", glass: "forte", estilo: "minimalista" });
    expect(document.documentElement.dataset.estilo).toBe("minimalista");
  });

  it("PC antigo (sem estilo), valor desconhecido ou sem tema: Clássico", () => {
    aplicarTema({ accent: "laranja", theme: "escuro" });
    expect(document.documentElement.dataset.estilo).toBe("classico");
    aplicarTema({ estilo: "futurista" });
    expect(document.documentElement.dataset.estilo).toBe("classico");
    aplicarTema(null);
    expect(document.documentElement.dataset.estilo).toBe("classico");
  });
});

describe("tela de uso", () => {
  it("rota #/uso vai e volta pelo endereço", () => {
    expect(lerRota("#/uso")).toEqual({ tela: "uso" });
    expect(urlDaRota({ tela: "uso" })).toMatch(/#\/uso$/);
  });

  it("lista as contas com 5H e semana", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify([
      { id: "padrao", provider: "claude", name: "Padrão", usage: { status: "ok", reason: null,
        primary: { used_percent: 42, resets_at: null, reset_label: null }, secondary: { used_percent: 88, resets_at: null, reset_label: null } } },
    ]))));
    render(<Uso />);
    const conta = (await screen.findByText("Padrão")).closest("li")!;
    expect(within(conta).getByText("42%")).toHaveClass("text-success");
    expect(within(conta).getByText("88%")).toHaveClass("text-danger");
    expect(within(conta).getByRole("button", { name: "Atualizar uso de Padrão" })).toBeInTheDocument();
  });

  it("sem contas no PC: aviso em vez de tela vazia", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response("[]")));
    render(<Uso />);
    expect(await screen.findByText("Nenhuma conta de Claude ou Codex no PC ainda.")).toBeInTheDocument();
  });
});
