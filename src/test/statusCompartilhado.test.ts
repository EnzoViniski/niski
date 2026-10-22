// Fork (desempenho, 2026-10-22): um `git status` por pasta para rodapé, painel e abas de agente.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { esquecerStatusCompartilhado, gitStatusCompartilhado, JANELA_DO_STATUS_MS } from "../features/git/statusCompartilhado";

const STATUS = { branch: "main", entries: [] };
beforeEach(() => esquecerStatusCompartilhado());

describe("gitStatusCompartilhado", () => {
  it("leituras de fundo na mesma janela dividem um pedido só", async () => {
    const ler = vi.fn().mockResolvedValue(STATUS);
    const pedidos = [0, 10, 500, 1_999].map((agora) => gitStatusCompartilhado("/repo", { ler, agora }));
    expect(await Promise.all(pedidos)).toEqual([STATUS, STATUS, STATUS, STATUS]);
    expect(ler).toHaveBeenCalledTimes(1);
    await gitStatusCompartilhado("/repo", { ler, agora: JANELA_DO_STATUS_MS });
    expect(ler).toHaveBeenCalledTimes(2);
  });

  it("pastas diferentes não se misturam", async () => {
    const ler = vi.fn().mockResolvedValue(STATUS);
    await gitStatusCompartilhado("/a", { ler, agora: 0 });
    await gitStatusCompartilhado("/b", { ler, agora: 0 });
    expect(ler.mock.calls.map((c) => c[0])).toEqual(["/a", "/b"]);
  });

  it("depois de uma ação, `fresco` lê na hora e o resultado vale para os outros", async () => {
    const ler = vi.fn().mockResolvedValueOnce(STATUS).mockResolvedValueOnce({ branch: "nova", entries: [] });
    await gitStatusCompartilhado("/repo", { ler, agora: 0 });
    expect((await gitStatusCompartilhado("/repo", { ler, agora: 100, fresco: true }))?.branch).toBe("nova");
    expect((await gitStatusCompartilhado("/repo", { ler, agora: 200 }))?.branch).toBe("nova");
    expect(ler).toHaveBeenCalledTimes(2);
  });

  it("falha não fica guardada", async () => {
    const ler = vi.fn().mockRejectedValueOnce(new Error("sem acesso")).mockResolvedValueOnce(STATUS);
    await expect(gitStatusCompartilhado("/repo", { ler, agora: 0 })).rejects.toThrow("sem acesso");
    await Promise.resolve();
    expect(await gitStatusCompartilhado("/repo", { ler, agora: 10 })).toEqual(STATUS);
  });
});
