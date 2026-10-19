import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const sessoes = [{ id: "s1", project_id: "p", name: "t", cwd: "/x", shell: "zsh", state: "idle", pid: 1, output_seq: 1, rows: 24, cols: 80 }];
const listar = vi.fn(async () => sessoes.map((s) => ({ ...s })));

vi.mock("../features/terminal/terminalService", () => ({
  ensureEngine: vi.fn(async () => ({ type: "pong" })),
  listTerminalSessions: () => listar(),
}));

import { useTerminalSessions } from "../features/terminal/useTerminalSessions";
import { ensureEngine } from "../features/terminal/terminalService";

describe("useTerminalSessions", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("mantém a mesma lista quando o engine devolve o mesmo conteúdo (sem re-render do App)", async () => {
    const { result } = renderHook(() => useTerminalSessions());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    const primeira = result.current.sessions;
    expect(primeira).toHaveLength(1);
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(listar.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(result.current.sessions).toBe(primeira);

    // Saída nova sozinha não derruba o App (energia); mudança de estado, sim.
    sessoes[0].output_seq = 2;
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(result.current.sessions).toBe(primeira);
    sessoes[0].state = "working";
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(result.current.sessions).not.toBe(primeira);
    expect(result.current.sessions[0].output_seq).toBe(2);
  });

  it("confere o engine só na primeira leitura e depois de uma falha (energia)", async () => {
    const conferir = vi.mocked(ensureEngine);
    conferir.mockClear();
    renderHook(() => useTerminalSessions());
    await act(async () => { await vi.advanceTimersByTimeAsync(3_000); });
    expect(conferir).toHaveBeenCalledTimes(1);
    listar.mockRejectedValueOnce(new Error("offline"));
    await act(async () => { await vi.advanceTimersByTimeAsync(2_000); });
    expect(conferir).toHaveBeenCalledTimes(2);
  });
});
