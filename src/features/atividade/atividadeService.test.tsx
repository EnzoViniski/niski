import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { atualizarAtividade, marcarAtividadeVista, useAtividade, useAtividadeVista } from "./atividadeService";
import type { ItemDeAtividade } from "./tipos";
const api = vi.hoisted(() => ({ request: vi.fn(), requestSeMudou: vi.fn() }));
vi.mock("../../mobile/api", () => api);
vi.mock("../../mobile/usePoll", () => ({ usePoll: vi.fn() }));
const item: ItemDeAtividade = { id: "evento", conversa: "c", sessao: "s", projeto_id: "p", projeto: "Loja", titulo: "Conversa", plataforma: "codex",
  conta: "Equipe", conta_id: "conta", tipo: "terminou", desde_ms: 1, visto: false, resumo: "Terminou", orquestrador: null };
beforeEach(async () => {
  vi.clearAllMocks(); api.request.mockResolvedValue({ ok: true }); api.requestSeMudou.mockResolvedValue([]);
  await atualizarAtividade(true); vi.clearAllMocks();
});
afterEach(() => { vi.restoreAllMocks(); });
it("compartilha uma requisição em voo e o mesmo snapshot entre contador e tela", async () => {
  let resolver!: (value: ItemDeAtividade[]) => void;
  api.requestSeMudou.mockReturnValueOnce(new Promise(resolve => { resolver = resolve; }));
  const primeira = atualizarAtividade(true), segunda = atualizarAtividade(true);
  expect(primeira).toBe(segunda); expect(api.requestSeMudou).toHaveBeenCalledTimes(1);
  const a = renderHook(() => useAtividade()), b = renderHook(() => useAtividade());
  await act(async () => { resolver([item]); await primeira; });
  expect(a.result.current.itens).toBe(b.result.current.itens); expect(a.result.current.itens).toEqual([item]);
});
it("304 conserva dados e marcar no engine atualiza todos os consumidores", async () => {
  api.requestSeMudou.mockResolvedValueOnce([item]); await atualizarAtividade(true);
  api.requestSeMudou.mockResolvedValueOnce(null); await atualizarAtividade(true);
  const hook = renderHook(() => useAtividade()); expect(hook.result.current.itens).toEqual([item]);
  await act(async () => { await marcarAtividadeVista([item.id]); });
  expect(api.request).toHaveBeenCalledWith("/atividade/vistos", expect.objectContaining({ method: "POST", body: JSON.stringify({ ids: [item.id] }) }));
  expect(hook.result.current.itens[0].visto).toBe(true);
});
it("resposta idêntica conserva a referência mesmo no transporte local sem ETag", async () => {
  api.requestSeMudou.mockResolvedValueOnce([item]); await atualizarAtividade(true);
  const hook = renderHook(() => useAtividade()); const anterior = hook.result.current.itens;
  api.requestSeMudou.mockResolvedValueOnce([{ ...item }]); await act(async () => { await atualizarAtividade(true); });
  expect(hook.result.current.itens).toBe(anterior);
});
it("GET antigo que chega depois de POST não desfaz a leitura", async () => {
  api.requestSeMudou.mockResolvedValueOnce([item]); await atualizarAtividade(true);
  let resolver!: (value: ItemDeAtividade[]) => void;
  api.requestSeMudou.mockReturnValueOnce(new Promise(resolve => { resolver = resolve; }));
  const leitura = atualizarAtividade(true);
  await marcarAtividadeVista([item.id]); resolver([item]); await leitura;
  const hook = renderHook(() => useAtividade()); expect(hook.result.current.itens[0].visto).toBe(true);
});
it("falha do POST não marca localmente e erro da API pode se recuperar", async () => {
  api.requestSeMudou.mockResolvedValueOnce([item]); await atualizarAtividade(true);
  api.request.mockRejectedValueOnce(new Error("disco cheio")); await expect(marcarAtividadeVista([item.id])).rejects.toThrow("disco cheio");
  const hook = renderHook(() => useAtividade()); expect(hook.result.current.itens[0].visto).toBe(false);
  api.requestSeMudou.mockRejectedValueOnce(new Error("offline"));
  await act(async () => { await atualizarAtividade(true).catch(() => undefined); });
  expect(hook.result.current.erro).toBe("offline");
  api.requestSeMudou.mockResolvedValueOnce([item]); await act(async () => { await atualizarAtividade(true); });
  expect(hook.result.current.erro).toBeNull();
});
it("não marca conversa em aba escondida; ao voltar marca só os eventos daquela conversa", async () => {
  const hidden = vi.spyOn(document, "hidden", "get").mockReturnValue(true);
  api.requestSeMudou.mockResolvedValueOnce([item, { ...item, id: "outro", conversa: "outra" }]); await atualizarAtividade(true);
  renderHook(() => useAtividadeVista(["c"], true)); expect(api.request).not.toHaveBeenCalled();
  hidden.mockReturnValue(false); act(() => document.dispatchEvent(new Event("visibilitychange")));
  await waitFor(() => expect(api.request).toHaveBeenCalledWith("/atividade/vistos", expect.objectContaining({ body: JSON.stringify({ ids: ["evento"] }) })));
});
