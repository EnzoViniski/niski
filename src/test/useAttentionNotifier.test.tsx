import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAttentionNotifier, testarAviso } from "../features/terminal/useAttentionNotifier";
import { enviarAvisos, EVENTO_AVISO_INTERNO } from "../features/atividade/avisos";
import { AVISOS_PADRAO, preferenciasDeAvisos, type ItemDeAtividade } from "../features/atividade/tipos";

const janela = vi.hoisted(() => ({ isFocused: vi.fn(), requestUserAttention: vi.fn() }));
const toast = vi.hoisted(() => ({ sendNotification: vi.fn(), isPermissionGranted: vi.fn(), requestPermission: vi.fn() }));
const feed = vi.hoisted(() => ({ itens: [] as ItemDeAtividade[], carregado: true }));
vi.mock("../features/atividade/atividadeService", () => ({ useAtividade: () => feed }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => janela, UserAttentionType: { Informational: 2 } }));
vi.mock("@tauri-apps/plugin-notification", () => toast);
function item(overrides: Partial<ItemDeAtividade> = {}): ItemDeAtividade {
  return { id: "c:s:1:terminou", conversa: "c", sessao: "s", projeto_id: "p", projeto: "API", titulo: "Revisar login",
    plataforma: "claude", conta_id: "conta", conta: "Trabalho", tipo: "terminou", desde_ms: 1, visto: false,
    resumo: "Terminou o turno", orquestrador: null, ...overrides };
}
function iniciar() {
  const hook = renderHook(({ enabled }) => useAttentionNotifier(enabled, AVISOS_PADRAO), { initialProps: { enabled: true } });
  return { ...hook, receber: (itens: ItemDeAtividade[], enabled = true) => { feed.itens = itens; hook.rerender({ enabled }); } };
}
beforeEach(() => {
  vi.clearAllMocks(); feed.itens = []; feed.carregado = true;
  janela.isFocused.mockResolvedValue(false); janela.requestUserAttention.mockResolvedValue(undefined);
  toast.isPermissionGranted.mockResolvedValue(true); toast.requestPermission.mockResolvedValue("granted");
});
describe("avisos da Atividade", () => {
  it("envia o som do tipo e pisca fora de foco", async () => {
    const hook = iniciar(); hook.receber([item()]);
    await waitFor(() => expect(toast.sendNotification).toHaveBeenCalledWith({ title: "API · Trabalho", body: "Revisar login — Terminou o turno", sound: "Ping" }));
    expect(janela.requestUserAttention).toHaveBeenCalled();
  });
  it("não notifica eventos antigos na primeira carga nem ao repetir o poll", async () => {
    feed.itens = [item()]; const hook = iniciar(); hook.receber([item()]);
    await act(async () => {}); expect(toast.sendNotification).not.toHaveBeenCalled();
  });
  it("espera a primeira carga completa antes de reconhecer eventos novos", async () => {
    feed.carregado = false; const hook = iniciar();
    feed.carregado = true; hook.receber([item()]); await act(async () => {});
    expect(toast.sendNotification).not.toHaveBeenCalled();
    hook.receber([item({ id: "2" })]); await waitFor(() => expect(toast.sendNotification).toHaveBeenCalledTimes(1));
  });
  it("um por conversa até a leitura compartilhada pelo engine, mesmo com troca de sessão", async () => {
    const hook = iniciar(); hook.receber([item()]); await waitFor(() => expect(toast.sendNotification).toHaveBeenCalledTimes(1));
    hook.receber([item({ id: "2", sessao: "outra" }), item()]); await act(async () => {});
    expect(toast.sendNotification).toHaveBeenCalledTimes(1);
    hook.receber([item({ id: "2", visto: true }), item({ visto: true })]);
    hook.receber([item({ id: "3" }), item({ id: "2", visto: true }), item({ visto: true })]);
    await waitFor(() => expect(toast.sendNotification).toHaveBeenCalledTimes(2));
  });
  it("erro, limite, pergunta e subagente notificam com sons próprios", async () => {
    const hook = iniciar();
    for (const [tipo, som] of [["erro", "Basso"], ["limite", "Basso"], ["precisa_de_voce", "Glass"], ["subagente_terminou", "Ping"]] as const) {
      hook.receber([item({ tipo, conversa: tipo, id: tipo })]);
      await waitFor(() => expect(toast.sendNotification).toHaveBeenCalledWith(expect.objectContaining({ sound: som })));
    }
    expect(toast.sendNotification).toHaveBeenCalledTimes(4);
  });
  it("tipos desligados, itens vistos e chave geral desligada ficam quietos", async () => {
    const config = preferenciasDeAvisos({ erro: { quando: "nunca", som: "Basso" } });
    await enviarAvisos([item({ tipo: "erro" })], config);
    const hook = iniciar(); hook.receber([item({ visto: true })]); hook.receber([item({ id: "2" })], false);
    await act(async () => {}); expect(toast.sendNotification).not.toHaveBeenCalled();
  });
  it("suprime fora_de_foco na frente e Sempre mostra aviso interno com som WebAudio", async () => {
    janela.isFocused.mockResolvedValue(true);
    const ouvir = vi.fn(); window.addEventListener(EVENTO_AVISO_INTERNO, ouvir);
    const fechar = vi.fn().mockResolvedValue(undefined);
    const oscilador = { type: "", frequency: { setValueAtTime: vi.fn() }, connect: vi.fn(), start: vi.fn(), stop: vi.fn(), onended: null as null | (() => void) };
    const ganho = { gain: { setValueAtTime: vi.fn(), exponentialRampToValueAtTime: vi.fn() }, connect: vi.fn() };
    vi.stubGlobal("AudioContext", vi.fn(() => ({ resume: vi.fn().mockResolvedValue(undefined), currentTime: 0, destination: {}, createOscillator: () => oscilador, createGain: () => ganho, close: fechar })));
    try {
      await enviarAvisos([item()]); expect(ouvir).not.toHaveBeenCalled();
      await enviarAvisos([item()], { terminou: { quando: "sempre", som: "Ping" } });
      expect(ouvir).toHaveBeenCalledTimes(1); expect(oscilador.start).toHaveBeenCalled();
      oscilador.onended?.(); expect(fechar).toHaveBeenCalled(); expect(toast.sendNotification).not.toHaveBeenCalled();
    } finally { window.removeEventListener(EVENTO_AVISO_INTERNO, ouvir); vi.unstubAllGlobals(); }
  });
  it("resume acima de três depois de filtrar tipos desligados", async () => {
    await enviarAvisos([0,1,2,3].map(n => item({ id: String(n), conversa: String(n) })));
    expect(toast.sendNotification).toHaveBeenCalledTimes(1);
    expect(toast.sendNotification).toHaveBeenCalledWith(expect.objectContaining({ body: "4 agentes com atividade nova", sound: "Ping" }));
  });
  it("Sem som não encaminha um nome de som", async () => {
    await enviarAvisos([item()], { terminou: { quando: "fora_de_foco", som: null } });
    expect(toast.sendNotification.mock.calls[0][0]).not.toHaveProperty("sound");
  });
  it("Windows acompanha o toast com WebAudio, sem enviar nomes de som exclusivos do Mac", async () => {
    const ua = vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Windows NT 10.0");
    const audio = vi.fn(() => ({ resume: vi.fn().mockResolvedValue(undefined), currentTime: 0, destination: {}, close: vi.fn(),
      createOscillator: () => ({ frequency: { setValueAtTime: vi.fn() }, connect: vi.fn(), start: vi.fn(), stop: vi.fn() }),
      createGain: () => ({ gain: { setValueAtTime: vi.fn(), exponentialRampToValueAtTime: vi.fn() }, connect: vi.fn() }) }));
    vi.stubGlobal("AudioContext", audio);
    try {
      await enviarAvisos([item()], { terminou: { quando: "fora_de_foco", som: "Ping" } });
      expect(toast.sendNotification).toHaveBeenCalledTimes(1); expect(toast.sendNotification.mock.calls[0][0]).not.toHaveProperty("sound");
      expect(audio).toHaveBeenCalledTimes(1);
    } finally { ua.mockRestore(); vi.unstubAllGlobals(); }
  });
  it("teste de aviso ignora foco e relata negação e plugin indisponível", async () => {
    janela.isFocused.mockResolvedValue(true); await expect(testarAviso()).resolves.toBeNull();
    expect(toast.sendNotification).toHaveBeenCalledWith(expect.objectContaining({ sound: "Glass" }));
    toast.isPermissionGranted.mockResolvedValue(false); toast.requestPermission.mockResolvedValue("denied");
    await expect(testarAviso()).resolves.toMatch(/negou permissão/i);
    toast.isPermissionGranted.mockRejectedValue(new Error("plugin ausente"));
    await expect(testarAviso()).resolves.toBe("plugin ausente");
  });
});
