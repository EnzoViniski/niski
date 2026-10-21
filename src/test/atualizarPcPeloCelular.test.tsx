import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { atenderPedidoDeAtualizacao, type Dependencias } from "../lib/pedidoDeAtualizacao";
import { arquivoDestaVersao, versaoMudou } from "../mobile/versaoNova";
import { AtualizarPc } from "../mobile/telas/AtualizarPc";

/*
 * Fork: "Atualizar o PC" pelo celular. O celular pede, o engine deixa o pedido, o app do PC faz o
 * mesmo que o "Atualizar agora" da barra, e o celular recarrega quando o engine novo serve a página nova.
 */

function dependencias(parcial: Partial<Dependencias>): Dependencias {
  return {
    consumirPedido: vi.fn(async () => true),
    procurar: vi.fn(async () => undefined),
    haVersaoNova: vi.fn(() => false),
    atualizar: vi.fn(async () => undefined),
    versaoNovaNoDisco: vi.fn(async () => false),
    reabrir: vi.fn(async () => undefined),
    ...parcial,
  };
}

afterEach(() => vi.unstubAllGlobals());

describe("pedido de atualização no PC", () => {
  it("sem pedido: não procura nem reinicia nada", async () => {
    const dep = dependencias({ consumirPedido: vi.fn(async () => false) });
    expect(await atenderPedidoDeAtualizacao(dep)).toBe("sem-pedido");
    expect(dep.procurar).not.toHaveBeenCalled();
    expect(dep.atualizar).not.toHaveBeenCalled();
  });

  it("versão nova no servidor: roda o Atualizar agora", async () => {
    const dep = dependencias({ haVersaoNova: vi.fn(() => true) });
    expect(await atenderPedidoDeAtualizacao(dep)).toBe("atualizando");
    expect(dep.atualizar).toHaveBeenCalledOnce();
    expect(dep.reabrir).not.toHaveBeenCalled();
  });

  it("app já trocado no disco: reabre trocando o engine", async () => {
    const dep = dependencias({ versaoNovaNoDisco: vi.fn(async () => true) });
    expect(await atenderPedidoDeAtualizacao(dep)).toBe("reabrindo");
    expect(dep.reabrir).toHaveBeenCalledOnce();
  });

  it("já está na última versão: não reinicia à toa", async () => {
    const dep = dependencias({});
    expect(await atenderPedidoDeAtualizacao(dep)).toBe("em-dia");
    expect(dep.reabrir).not.toHaveBeenCalled();
  });
});

describe("celular pega a página nova", () => {
  it("reconhece o arquivo do build e ignora o dev", () => {
    expect(arquivoDestaVersao("https://pc.local/assets/index-Bjjjird_.js")).toBe("index-Bjjjird_.js");
    expect(arquivoDestaVersao("http://localhost:5197/src/mobile/versaoNova.ts")).toBeNull();
  });

  it("recarrega só quando o index.html servido aponta para outro arquivo", () => {
    expect(versaoMudou('<script type="module" src="/assets/index-Novo123.js"></script>', "index-Velho12.js")).toBe(true);
    expect(versaoMudou('<script type="module" src="/assets/index-Velho12.js"></script>', "index-Velho12.js")).toBe(false);
    expect(versaoMudou("Página de erro", "index-Velho12.js")).toBe(false);
    expect(versaoMudou("<script></script>", null)).toBe(false);
  });
});

describe("botão Atualizar o PC", () => {
  it("confirma, pede ao PC e avisa", async () => {
    vi.stubGlobal("confirm", vi.fn(() => true));
    const fetch = vi.fn(async () => new Response(JSON.stringify({ ok: true })));
    vi.stubGlobal("fetch", fetch);
    render(<AtualizarPc />);
    await userEvent.click(screen.getByRole("button", { name: "Atualizar o PC" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Pedido enviado");
    expect(fetch.mock.calls.some(([url, init]) => url === "/pc/atualizar" && (init as RequestInit)?.method === "POST")).toBe(true);
  });

  it("cancelar na confirmação não pede nada", async () => {
    vi.stubGlobal("confirm", vi.fn(() => false));
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    render(<AtualizarPc />);
    await userEvent.click(screen.getByRole("button", { name: "Atualizar o PC" }));
    expect(fetch).not.toHaveBeenCalled();
  });
});
