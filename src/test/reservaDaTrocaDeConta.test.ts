import { beforeEach, describe, expect, it, vi } from "vitest";

const engineApi = vi.fn();
vi.mock("../features/terminal/terminalService", () => ({ engineApi: (...args: unknown[]) => engineApi(...args) }));

import { reservarTrocaDeConta, soltarTrocaDeConta } from "../features/terminal/reservaDaTrocaDeConta";

// Fork (harness, A4): o painel só troca a conta com a reserva; 409 = o engine já está trocando.
describe("reserva da troca de conta", () => {
  beforeEach(() => { engineApi.mockReset(); });

  it("ganha a vez com 200 e manda o dono", async () => {
    engineApi.mockResolvedValue({ type: "api", status: 200, body: "{}" });
    expect(await reservarTrocaDeConta("c/1", "painel-x")).toBe(true);
    expect(engineApi).toHaveBeenCalledWith("POST", "/conversas/c%2F1/troca-de-conta/reservar", JSON.stringify({ dono: "painel-x" }));
  });

  it("não troca com 409", async () => {
    engineApi.mockResolvedValue({ type: "api", status: 409, body: "{}" });
    expect(await reservarTrocaDeConta("c1", "painel-x")).toBe(false);
  });

  it("engine antigo (sem a rota) ou fora do ar: troca como sempre", async () => {
    engineApi.mockResolvedValueOnce({ type: "api", status: 404, body: "{}" });
    expect(await reservarTrocaDeConta("c1", "p")).toBe(true);
    engineApi.mockRejectedValueOnce(new Error("engine fora"));
    expect(await reservarTrocaDeConta("c1", "p")).toBe(true);
  });

  it("soltar nunca joga erro", async () => {
    engineApi.mockRejectedValue(new Error("x"));
    await expect(soltarTrocaDeConta("c1", "p")).resolves.toBeUndefined();
  });
});
