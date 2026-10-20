import { aguardarAcao } from "../mobile/conversa/aguardarAcao";
import { request } from "../mobile/api";
vi.mock("../mobile/api", () => ({ request: vi.fn() }));
beforeEach(() => vi.mocked(request).mockReset());
afterEach(() => vi.useRealTimers());

it("202 enfileirado só termina quando o engine informa sent", async () => {
  vi.useFakeTimers();
  vi.mocked(request).mockResolvedValueOnce({ actions: [{ id: "a", state: "executing" }] })
    .mockResolvedValueOnce({ actions: [{ id: "a", state: "sent" }] });
  let terminou = false;
  const promessa = aguardarAcao("c/1", "a").then(() => { terminou = true; });
  await vi.advanceTimersByTimeAsync(100);
  expect(terminou).toBe(false);
  await vi.advanceTimersByTimeAsync(100);
  await promessa;
  expect(terminou).toBe(true);
  expect(request).toHaveBeenCalledWith("/conversas/c%2F1/timeline");
});

it("rejeição da ação é devolvida ao controle", async () => {
  vi.mocked(request).mockResolvedValue({ actions: [{ id: "a", state: "rejected", error: "Campo ocupado" }] });
  await expect(aguardarAcao("c", "a")).rejects.toThrow("Campo ocupado");
});

it("resposta sem id de ação não confirma sucesso", async () => {
  await expect(aguardarAcao("c", undefined)).rejects.toThrow("O engine não confirmou a ação");
  expect(request).not.toHaveBeenCalled();
});
