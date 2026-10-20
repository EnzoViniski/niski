import { beforeEach, expect, it, vi } from "vitest";
import { engineApi } from "../terminal/terminalService";
import { buscarNoHistorico } from "./servico";
vi.mock("../terminal/terminalService", () => ({ engineApi: vi.fn() }));
beforeEach(() => { vi.clearAllMocks(); });
it("consulta via POST com limites sem texto na URL", async () => {
  vi.mocked(engineApi).mockResolvedValue({ type: "api", status: 200, body: '{"achados":[],"parcial":true}' });
  expect(await buscarNoHistorico("texto sintético")).toEqual({ achados: [], parcial: true });
  expect(engineApi).toHaveBeenCalledWith("POST", "/historico/buscar", JSON.stringify({ texto: "texto sintético", limite: 40, tempo_ms: 750 }));
});
it("erro não propaga o corpo nem a consulta", async () => {
  vi.mocked(engineApi).mockResolvedValue({ type: "api", status: 429, body: "conteúdo que não deve vazar" });
  await expect(buscarNoHistorico("texto sintético")).rejects.toThrow("Não foi possível consultar o histórico.");
});
