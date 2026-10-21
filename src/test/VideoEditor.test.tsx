// Fork (Editor de vídeo): telas com disco, IA, render e player falsos — criar o vídeo, editar pela
// timeline e pelo chat, desfazer, exportar e pastas de referências.
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { VideoView } from "../features/video/VideoView";
import { lerComposicao } from "../features/video/composicao";
import { definirTokenParaTeste } from "../features/video/enderecos";
import { indexInicial } from "../features/video/modelos";
import { discoFalso } from "./videoDiscoFalso";

vi.mock("@hyperframes/player", () => {
  class PlayerFalso extends HTMLElement {
    static get observedAttributes() { return ["src"]; }
    paused = true; ready = false; currentTime = 0; duration = 0; playbackRate = 1; muted = false;
    attributeChangedCallback() {
      this.ready = true; this.duration = 15;
      queueMicrotask(() => this.dispatchEvent(new CustomEvent("ready", { detail: { duration: 15 } })));
    }
    play() { this.paused = false; this.dispatchEvent(new Event("play")); }
    pause() { this.paused = true; this.dispatchEvent(new Event("pause")); }
    seek(t: number) { this.currentTime = t; this.dispatchEvent(new CustomEvent("timeupdate", { detail: { currentTime: t } })); }
  }
  if (!customElements.get("hyperframes-player")) customElements.define("hyperframes-player", PlayerFalso);
  return {};
});

const mockedInvoke = vi.mocked(invoke);

let disco: ReturnType<typeof discoFalso>;

beforeEach(() => {
  disco = discoFalso();
  definirTokenParaTeste("tok");
  mockedInvoke.mockImplementation(async (cmd, args) => disco.comando(cmd, (args ?? {}) as Record<string, unknown>));
  try { localStorage.clear(); } catch { /* sem armazenamento */ }
});
afterEach(() => { mockedInvoke.mockReset(); mockedInvoke.mockResolvedValue("{}"); definirTokenParaTeste(null); });

async function criarProjeto(nome = "Lançamento") {
  const pid = disco.comando("videos_criar", { novo: {
    projeto: { nome, formato: "9:16", largura: 1080, altura: 1920, fps: 30, cor: "#7c9cff", ia: { cli: "claude" }, briefing: {}, pastasDeReferencia: [], sessoes: {} },
    brief: "# Brief\n", index: indexInicial({ nome, formato: "9:16", duracao: 15 }), instrucoes: "# x",
  } }) as string;
  disco.chamadas.length = 0;
  return pid;
}

describe("Edição de vídeo", () => {
  it("cria um vídeo com briefing e referências e abre o editor", async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(open).mockResolvedValueOnce(["/Users/x/ref.mp4", "/Users/x/logo.png"]);
    render(<VideoView onClose={() => undefined} />);
    fireEvent.click(await screen.findByRole("button", { name: "Criar o primeiro vídeo" }));
    fireEvent.change(screen.getByPlaceholderText("Ex.: Lançamento do app"), { target: { value: "Promo <nova>" } });
    fireEvent.click(screen.getByRole("radio", { name: /9:16/ }));
    fireEvent.change(screen.getByPlaceholderText(/vídeo de 15 s/), { target: { value: "Mostrar as três telas novas" } });
    fireEvent.click(screen.getByRole("button", { name: "Escolher arquivos…" }));
    expect(await screen.findByText("ref.mp4")).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText("https://…"), { target: { value: "https://exemplo.com/referencia" } });
    fireEvent.click(screen.getByRole("button", { name: "Adicionar" }));
    fireEvent.click(screen.getByRole("button", { name: "Criar e abrir o editor" }));

    expect(await screen.findByLabelText("Timeline")).toBeInTheDocument();
    const pid = "v-teste-1";
    const projeto = JSON.parse(disco.arquivos.get(`${pid}/projeto.json`)!);
    expect(projeto).toMatchObject({ nome: "Promo <nova>", formato: "9:16", largura: 1080, altura: 1920 });
    const brief = disco.arquivos.get(`${pid}/BRIEF.md`)!;
    expect(brief).toContain("Mostrar as três telas novas");
    expect(brief).toContain("`assets/ref.mp4`");
    expect(brief).toContain("https://exemplo.com/referencia");
    expect(JSON.parse(disco.arquivos.get(`${pid}/midias.json`)!).links[0].url).toBe("https://exemplo.com/referencia");
    expect(lerComposicao(disco.arquivos.get(`${pid}/index.html`)!).raiz).toMatchObject({ largura: 1080, altura: 1920, duracao: 15 });
    expect(disco.arquivos.get(`${pid}/CLAUDE.md`)).toContain("hyperframes@0.8.183 lint");
  });

  it("edita pela timeline e pelas propriedades, guarda versão e desfaz", async () => {
    const pid = await criarProjeto();
    localStorage.setItem("medza.video.ultimo", pid);
    render(<VideoView onClose={() => undefined} continuar />);
    const clipe = await screen.findByRole("button", { name: /Texto Lançamento/ });
    // jsdom não tem PointerEvent com `button`: seleciona pelo teclado (mesmo caminho do Enter).
    fireEvent.keyDown(clipe, { key: "Enter" });
    const duracao = await screen.findByRole("textbox", { name: "Duração" });
    fireEvent.change(duracao, { target: { value: "4" } });
    fireEvent.blur(duracao);
    await waitFor(() => expect(lerComposicao(disco.arquivos.get(`${pid}/index.html`)!).clipes[0].duracao).toBe(4));
    const gravacao = disco.chamadas.find((c) => c.cmd === "videos_gravar" && c.args.arquivo === "index.html");
    expect(gravacao?.args.motivo).toMatch(/Duração de “Lançamento”/);

    fireEvent.click(screen.getByRole("button", { name: "Desfazer" }));
    await waitFor(() => expect(lerComposicao(disco.arquivos.get(`${pid}/index.html`)!).clipes[0].duracao).toBe(15));
    fireEvent.click(screen.getByRole("button", { name: "Refazer" }));
    await waitFor(() => expect(lerComposicao(disco.arquivos.get(`${pid}/index.html`)!).clipes[0].duracao).toBe(4));
  });

  it("chat: guarda versão antes, manda o contexto, recarrega o preview e continua a sessão", async () => {
    const pid = await criarProjeto();
    disco.definirAgente((_texto, id) => {
      const html = disco.arquivos.get(`${id}/index.html`)!.replace('data-duration="15" data-track-index="0"', 'data-duration="12" data-track-index="0"');
      disco.arquivos.set(`${id}/index.html`, html);
      return { texto: "Cortei os 3 segundos finais.", arquivos: ["index.html"] };
    });
    localStorage.setItem("medza.video.ultimo", pid);
    render(<VideoView onClose={() => undefined} continuar />);
    await screen.findByLabelText("Timeline");
    const player = await waitFor(() => { const p = document.querySelector("hyperframes-player"); expect(p).toBeTruthy(); return p!; });
    const srcAntes = player.getAttribute("src");
    expect(srcAntes).toMatch(/^medzavideo:\/\/localhost\/tok\/p\/v-teste-1\/index\.html\?v=\d+$/);
    expect(player.getAttribute("sandbox-origin")).toBe("opaque");

    fireEvent.change(screen.getByLabelText("Pedido para o agente"), { target: { value: "corta o fim" } });
    fireEvent.click(screen.getByRole("button", { name: "Enviar" }));
    expect(await screen.findByText("Cortei os 3 segundos finais.")).toBeInTheDocument();
    const ordem = disco.chamadas.map((c) => c.cmd);
    expect(ordem.indexOf("videos_criar_versao")).toBeLessThan(ordem.indexOf("videos_agente"));
    const pedido = disco.chamadas.find((c) => c.cmd === "videos_agente")!.args.pedido as { texto: string; cli: string; sessao: string | null };
    expect(pedido).toMatchObject({ cli: "claude", sessao: null });
    expect(pedido.texto).toMatch(/^corta o fim\n\n\[Contexto do editor: agulha em 00:00\.00/);
    await waitFor(() => expect(player.getAttribute("src")).not.toBe(srcAntes));
    await waitFor(() => expect(JSON.parse(disco.arquivos.get(`${pid}/projeto.json`)!).sessoes).toEqual({ "claude:": "sessao-falsa" }));
    expect(disco.arquivos.get(`${pid}/conversa.jsonl`)!.trim().split("\n")).toHaveLength(2);
    expect(screen.getByRole("button", { name: /IA do vídeo: Claude, modelo-falso/ })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Pedido para o agente"), { target: { value: "de novo" } });
    fireEvent.click(screen.getByRole("button", { name: "Enviar" }));
    await waitFor(() => expect(disco.chamadas.filter((c) => c.cmd === "videos_agente")).toHaveLength(2));
    expect((disco.chamadas.filter((c) => c.cmd === "videos_agente")[1].args.pedido as { sessao: string }).sessao).toBe("sessao-falsa");
  });

  it("exporta local com o formato escolhido", async () => {
    const pid = await criarProjeto();
    localStorage.setItem("medza.video.ultimo", pid);
    render(<VideoView onClose={() => undefined} continuar />);
    fireEvent.click(await screen.findByRole("button", { name: "Exportar" }));
    const dialogo = screen.getByRole("dialog", { name: "Exportar vídeo" });
    fireEvent.click(within(dialogo).getByRole("button", { name: "WebM" }));
    fireEvent.click(within(dialogo).getByRole("button", { name: "Exportar" }));
    expect(await within(dialogo).findByText(/Pronto: x\.mp4/)).toBeInTheDocument();
    const pedido = disco.chamadas.find((c) => c.cmd === "videos_exportar")!.args.pedido;
    expect(pedido).toMatchObject({ projeto: pid, formato: "webm", qualidade: "standard", fps: null });
  });

  it("aberto e parado não liga timer nem poll", async () => {
    const pid = await criarProjeto();
    localStorage.setItem("medza.video.ultimo", pid);
    render(<VideoView onClose={() => undefined} continuar />);
    await screen.findByLabelText("Timeline");
    // Depois de abrir (o `findBy` da biblioteca de testes usa setInterval próprio).
    const intervalo = vi.spyOn(window, "setInterval");
    await act(async () => { await new Promise((r) => setTimeout(r, 50)); });
    expect(intervalo).not.toHaveBeenCalled();
    intervalo.mockRestore();
  });

  it("cria pasta de referências pela lista", async () => {
    render(<VideoView onClose={() => undefined} />);
    fireEvent.change(await screen.findByPlaceholderText("Nome da pasta nova"), { target: { value: "Reels da Medza" } });
    fireEvent.click(screen.getByRole("button", { name: "Criar" }));
    expect(await screen.findByRole("button", { name: "Novo vídeo com esta pasta" })).toBeInTheDocument();
    expect(disco.chamadas.find((c) => c.cmd === "videos_ref_criar")?.args.dados).toMatchObject({ nome: "Reels da Medza" });
  });
});
