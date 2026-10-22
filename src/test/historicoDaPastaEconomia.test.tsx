// Fork (desempenho, 2026-10-22): voltar à janela várias vezes não revarre os transcripts a cada volta,
// e lista igual não redesenha a lateral.
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { FOCO_RELE_DEPOIS_DE_MS, redefinirConversasAnteriores, useConversasAnteriores } from "../features/projects/historicoDaPasta";

const ENTRADA = { provider: "claude", profile_id: "p", profile_name: "P", session_id: "s1", title: "T", first_prompt: "oi",
  cwd: "/proj", started_at_ms: 1, updated_at_ms: 1, path: "/x.jsonl" };

function leiturasDoHistorico() {
  return vi.mocked(invoke).mock.calls.filter(([comando]) => comando === "agent_history").length;
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (comando: string, args?: unknown) =>
    comando === "agent_history" ? ((args as { provider: string }).provider === "claude" ? [ENTRADA] : []) : []);
  redefinirConversasAnteriores();
});

describe("conversas anteriores da lateral", () => {
  it("foco logo depois de ler não relê; depois da janela relê; lista igual não redesenha", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      let desenhos = 0;
      const { result, unmount } = renderHook(() => { desenhos += 1; return useConversasAnteriores(); });
      await act(async () => { await vi.advanceTimersByTimeAsync(10); });
      expect(result.current.entradas).toHaveLength(1);
      const lidas = leiturasDoHistorico();
      const desenhosDepoisDaLeitura = desenhos;

      await act(async () => { window.dispatchEvent(new Event("focus")); await vi.advanceTimersByTimeAsync(10); });
      expect(leiturasDoHistorico()).toBe(lidas);

      await act(async () => { await vi.advanceTimersByTimeAsync(FOCO_RELE_DEPOIS_DE_MS); });
      await act(async () => { window.dispatchEvent(new Event("focus")); await vi.advanceTimersByTimeAsync(10); });
      expect(leiturasDoHistorico()).toBeGreaterThan(lidas);
      expect(desenhos).toBe(desenhosDepoisDaLeitura);
      unmount();
    } finally {
      vi.useRealTimers();
    }
  });
});
