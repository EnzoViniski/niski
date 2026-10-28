import { describe, expect, it, vi } from "vitest";
import { linkHandlerDoTerminal, provedorDeUrls, urlsDaLinha } from "../features/navegador/linksDoTerminal";
import { EVENTO_ABRIR_LINK, marcarDesenhada, prenderPagina, rotulosDesenhados, rotulosPresos } from "../features/navegador/registroDoNavegador";
import type { Terminal } from "@xterm/xterm";

// Links do terminal abrem numa aba do app (o padrão do xterm não fazia nada no Tauri).
describe("links do terminal", () => {
  it("acha endereços soltos sem a pontuação da frase", () => {
    expect(urlsDaLinha("  Studio: http://localhost:3002/projects/video-1. Abra.")).toEqual([{ inicio: 10, url: "http://localhost:3002/projects/video-1" }]);
    expect(urlsDaLinha("veja (https://pt.wikipedia.org/wiki/Foo_(bar)) e https://x.dev/a,")).toEqual([
      { inicio: 6, url: "https://pt.wikipedia.org/wiki/Foo_(bar)" }, { inicio: 49, url: "https://x.dev/a" },
    ]);
    expect(urlsDaLinha("│ https://a.b/c │")[0].url).toBe("https://a.b/c");
    expect(urlsDaLinha("file:///etc/passwd e ftp://x")).toEqual([]);
  });

  it("OSC 8 abre com um clique; endereço solto só com ⌘ + clique", () => {
    const pedidos: string[] = [];
    const ouvir = (e: Event) => pedidos.push((e as CustomEvent<{ url: string }>).detail.url);
    window.addEventListener(EVENTO_ABRIR_LINK, ouvir);
    const host = document.createElement("div");
    const handler = linkHandlerDoTerminal(() => host);
    expect(handler.allowNonHttpProtocols).toBe(false);
    handler.activate(new MouseEvent("click"), "https://hyperframes.dev/x", { start: { x: 1, y: 1 }, end: { x: 2, y: 1 } });
    handler.activate(new MouseEvent("click"), "javascript:alert(1)", { start: { x: 1, y: 1 }, end: { x: 2, y: 1 } });

    const terminal = { buffer: { active: { getLine: () => ({ translateToString: () => "Preview em http://localhost:3002/" }) } } } as unknown as Terminal;
    const devolver = vi.fn();
    provedorDeUrls(terminal, () => host).provideLinks(3, devolver);
    const [link] = devolver.mock.calls[0][0];
    expect(link.range).toEqual({ start: { x: 12, y: 3 }, end: { x: 33, y: 3 } });
    link.activate(new MouseEvent("click"), link.text);
    link.activate(new MouseEvent("click", { metaKey: true }), link.text);
    link.hover?.(new MouseEvent("mouseover"), link.text);
    expect(host.title).toContain("⌘");
    window.removeEventListener(EVENTO_ABRIR_LINK, ouvir);
    expect(pedidos).toEqual(["https://hyperframes.dev/x", "http://localhost:3002/"]);
  });

  it("registro: desenhada e presa saem quando o dono desmonta", () => {
    const a = marcarDesenhada("nav-a"); const b = marcarDesenhada("nav-a"); const p = prenderPagina("nav-r");
    expect(rotulosDesenhados()).toContain("nav-a");
    a(); expect(rotulosDesenhados()).toContain("nav-a");
    b(); expect(rotulosDesenhados()).not.toContain("nav-a");
    expect(rotulosPresos()).toEqual(["nav-r"]); p(); expect(rotulosPresos()).toEqual([]);
  });
});

describe("conferência das páginas de navegador", () => {
  it("manda as abas que existem e as desenhadas; pedido do terminal vira ABRIR_LINK", async () => {
    vi.useFakeTimers();
    const { renderHook } = await import("@testing-library/react");
    const { invoke } = await import("@tauri-apps/api/core");
    const { useLinksEmAba } = await import("../features/navegador/useLinksEmAba");
    const { abrirLinkNoApp } = await import("../features/navegador/registroDoNavegador");
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    vi.mocked(invoke).mockClear();
    const workspaces = [{ projects: [{ layout: { type: "pane", id: "p1", tabs: [{ id: "tab-web", kind: "web" }, { id: "t2", kind: "terminal" }] } }] }];
    const dispatch = vi.fn();
    const desmarcar = marcarDesenhada("nav-tab-web");
    const { unmount } = renderHook(() => useLinksEmAba(workspaces as never, dispatch, () => {}));
    await vi.advanceTimersByTimeAsync(1600);
    expect(invoke).toHaveBeenCalledWith("navegador_conferir", { vivos: ["nav-tab-web"], montados: ["nav-tab-web"] });
    abrirLinkNoApp("http://localhost:3002/");
    expect(dispatch).toHaveBeenCalledWith(expect.objectContaining({ type: "ABRIR_LINK", url: "http://localhost:3002/" }));
    unmount(); desmarcar();
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.useRealTimers();
  });
});
