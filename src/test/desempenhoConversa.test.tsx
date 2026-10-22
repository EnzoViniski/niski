// Fork (desempenho, 2026-10-22): o chat do PC passa a receber 304 pelo IPC e a faixa da aba lê só o resumo.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { transporteDoEngine } from "../features/conversa/transporteDoEngine";
import { requestSeMudou, usarTransporte } from "../mobile/api";
import { useResumoDaConversa } from "../features/conversa/resumoDaConversa";

const engine = vi.hoisted(() => ({ engineApi: vi.fn() }));
vi.mock("../features/terminal/terminalService", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../features/terminal/terminalService")>()),
  ...engine,
}));

const CONVERSA = { id: "c1", title: "Login", project_id: "p", provider: "claude", profile_id: null, state: "answered" };

/** Engine falso que responde como o de verdade: ETag do corpo e 304 quando o cliente já o tem. */
function engineComEtag(corpos: Record<string, () => unknown>) {
  engine.engineApi.mockImplementation(async (_m: string, path: string, _b: unknown, extra: { headers?: [string, string][] }) => {
    const rota = Object.keys(corpos).find((r) => path.endsWith(r));
    if (!rota) return { type: "api", status: 404, body: "" };
    const body = JSON.stringify(corpos[rota]());
    const etag = `"${body.length}:${body}"`;
    const anterior = extra?.headers?.find(([nome]) => nome.toLowerCase() === "if-none-match")?.[1];
    return anterior === etag ? { type: "api", status: 304, body: "", etag } : { type: "api", status: 200, body, etag };
  });
}

beforeEach(() => { vi.clearAllMocks(); usarTransporte(transporteDoEngine); });
afterEach(() => usarTransporte((path, init) => fetch(path, init)));

describe("ETag pelo protocolo local do engine", () => {
  it("a segunda leitura sem mudança volta null (304) e sem If-None-Match de outra tela", async () => {
    let titulo = "A";
    engineComEtag({ "/timeline": () => ({ titulo }) });
    expect(await requestSeMudou("/conversas/c1/timeline", undefined, "tela-1")).toEqual({ titulo: "A" });
    expect(await requestSeMudou("/conversas/c1/timeline", undefined, "tela-1")).toBeNull();
    // Outra tela da mesma conversa tem o próprio ETag: recebe o corpo, não um 304 que não é dela.
    expect(await requestSeMudou("/conversas/c1/timeline", undefined, "tela-2")).toEqual({ titulo: "A" });
    titulo = "B";
    expect(await requestSeMudou("/conversas/c1/timeline", undefined, "tela-1")).toEqual({ titulo: "B" });
    const segunda = engine.engineApi.mock.calls[1];
    expect(segunda[3].headers).toContainEqual(["If-None-Match", expect.stringContaining('"')]);
  });
});

describe("useResumoDaConversa", () => {
  it("lê /resumo (não a timeline) e não troca o objeto quando nada mudou", async () => {
    engineComEtag({ "/resumo": () => CONVERSA, "/timeline": () => ({ conversation: CONVERSA }) });
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { result } = renderHook(() => useResumoDaConversa("c1"));
      await waitFor(() => expect(result.current?.title).toBe("Login"));
      const primeiro = result.current;
      await act(async () => { await vi.advanceTimersByTimeAsync(3_100); });
      await act(async () => { await vi.advanceTimersByTimeAsync(3_100); });
      expect(result.current).toBe(primeiro);
      const caminhos = engine.engineApi.mock.calls.map((c) => String(c[1]));
      expect(caminhos.length).toBeGreaterThanOrEqual(3);
      expect(caminhos.every((c) => c.endsWith("/resumo"))).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });

  it("engine antigo, sem a rota: cai na timeline", async () => {
    engineComEtag({ "/timeline": () => ({ conversation: CONVERSA }) });
    const { result } = renderHook(() => useResumoDaConversa("c1"));
    await waitFor(() => expect(result.current?.title).toBe("Login"));
    expect(engine.engineApi.mock.calls.map((c) => String(c[1]))).toEqual(["/conversas/c1/resumo", "/conversas/c1/timeline"]);
  });

  it("sem conversa não pergunta nada", async () => {
    engineComEtag({});
    renderHook(() => useResumoDaConversa(null));
    await act(async () => { await Promise.resolve(); });
    expect(engine.engineApi).not.toHaveBeenCalled();
  });
});
