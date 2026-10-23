// Fork (Copiloto abre conversa na tela): conversa criada com `na_tela` vira aba uma vez, só se recente.
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { renderHook } from "@testing-library/react";
import { EVENTO_ORQUESTRACAO } from "../orquestracao/abrirFluxo";
import type { NoAoVivo } from "../orquestracao/orquestracaoService";
import { paraAbrirNaTela, PRAZO_PARA_ABRIR_NA_TELA_MS, useAbrirNaTela } from "./abrirNaTela";

const no = (id: string, extra: Partial<NoAoVivo> = {}) =>
  ({ id, titulo: `Codex · ${id}`, criado_em: Date.now(), na_tela: true, ...extra }) as NoAoVivo;

describe("abrir na tela", () => {
  const pedidos: unknown[] = [];
  const ouvir = (e: Event) => pedidos.push((e as CustomEvent).detail);
  afterEach(() => { window.removeEventListener(EVENTO_ORQUESTRACAO, ouvir); pedidos.length = 0; });

  it("só conversas na_tela, recentes e ainda não abertas", () => {
    const agora = Date.now();
    const lista = [no("a"), no("b", { na_tela: false }), no("c", { criado_em: agora - PRAZO_PARA_ABRIR_NA_TELA_MS - 1 }), no("d")];
    expect(paraAbrirNaTela(lista, new Set(["d"]), agora).map((a) => a.id)).toEqual(["a"]);
  });

  it("abre a aba da sessão da conversa uma vez, e espera a sessão aparecer", async () => {
    window.addEventListener(EVENTO_ORQUESTRACAO, ouvir);
    let sessoes: unknown[] = [];
    vi.mocked(invoke).mockClear().mockImplementation(async (cmd) =>
      cmd === "terminal_sessions" ? { type: "sessions", sessions: sessoes } : { type: "ok" });
    const { rerender } = renderHook(({ agentes }) => useAbrirNaTela(agentes), { initialProps: { agentes: [no("conv-1")] } });
    await vi.waitFor(() => expect(vi.mocked(invoke)).toHaveBeenCalledWith("terminal_sessions"));
    expect(pedidos).toEqual([]);

    sessoes = [{ id: "s-9", conversation_id: "conv-1", name: "Codex", state: "idle" }];
    rerender({ agentes: [no("conv-1")] });
    await vi.waitFor(() => expect(pedidos).toEqual([{ tipo: "abrir-conversa", sessionId: "s-9", titulo: "Codex · conv-1" }]));
    rerender({ agentes: [no("conv-1")] });
    await new Promise((r) => setTimeout(r, 20));
    expect(pedidos).toHaveLength(1);
  });
});
