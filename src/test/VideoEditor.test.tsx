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
import { SessionProvidersContext } from "../features/terminal/sessionProviders";
import type { TerminalSession } from "../features/terminal/terminalService";

// O chat do editor é a conversa normal do app (TerminalPane + AbaDeAgente, testados à parte). Aqui
// entra um falso que mostra a sessão ligada e deixa "abrir" uma conversa nova.
vi.mock("../features/video/ChatDoEditor", () => ({
  ChatDoEditor: ({ chat, pasta, onChat, aviso }: { chat: { sessao: string } | null; pasta: string; onChat: (c: { sessao: string; provider: string } | null) => void; aviso?: string | null }) => (
    <div aria-label="Conversa com o agente (falsa)">
      <span>cwd {pasta}</span><span>{chat ? `sessão ${chat.sessao}` : "sem sessão"}</span>{aviso && <span>{aviso}</span>}
      <button type="button" onClick={() => onChat({ sessao: "s-1", provider: "claude" })}>abrir conversa</button>
    </div>
  ),
}));

function sessaoFalsa(state: TerminalSession["state"]): Record<string, TerminalSession> {
  return { "s-1": { id: "s-1", project_id: "video:v-teste-1", name: "Vídeo", cwd: "/dados/videos/projetos/v-teste-1", state, provider: "claude" } as TerminalSession };
}

vi.mock("@hyperframes/player", () => {
  class PlayerFalso extends HTMLElement {
    static get observedAttributes() { return ["src"]; }
    paused = true; ready = false; currentTime = 0; duration = 0; playbackRate = 1; muted = false;
    // Fork (edição na mão): o iframe da composição, de onde vêm as mensagens da ponte.
    iframeElement = document.createElement("iframe");
    connectedCallback() { if (!this.iframeElement.isConnected) this.appendChild(this.iframeElement); }
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

  it("chat normal: liga a sessão ao vídeo, guarda versão quando o agente começa e recarrega quando ele edita", async () => {
    const pid = await criarProjeto();
    localStorage.setItem("medza.video.ultimo", pid);
    const { rerender } = render(<SessionProvidersContext.Provider value={{}}><VideoView onClose={() => undefined} continuar /></SessionProvidersContext.Provider>);
    await screen.findByLabelText("Timeline");
    expect(await screen.findByText("cwd /dados/videos/projetos/v-teste-1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "abrir conversa" }));
    await waitFor(() => expect(JSON.parse(disco.arquivos.get(`${pid}/projeto.json`)!).chat).toEqual({ sessao: "s-1", provider: "claude" }));
    const player = await waitFor(() => { const p = document.querySelector("hyperframes-player"); expect(p).toBeTruthy(); return p!; });
    const srcAntes = player.getAttribute("src");
    expect(player.getAttribute("sandbox-origin")).toBe("opaque");

    // O agente começa: versão antes, sem travar o editor.
    disco.chamadas.length = 0;
    rerender(<SessionProvidersContext.Provider value={sessaoFalsa("working")}><VideoView onClose={() => undefined} continuar /></SessionProvidersContext.Provider>);
    await waitFor(() => expect(disco.chamadas.some((c) => c.cmd === "videos_criar_versao" && c.args.motivo === "Antes do pedido ao agente")).toBe(true));
    expect(screen.getByRole("button", { name: /Texto Lançamento/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "＋ Texto" })).not.toBeDisabled();

    // O agente edita o arquivo por fora: o editor percebe pela assinatura e recarrega.
    disco.arquivos.set(`${pid}/index.html`, disco.arquivos.get(`${pid}/index.html`)!.replace('data-duration="15" data-track-index="0"', 'data-duration="12" data-track-index="0"'));
    await waitFor(() => expect(player.getAttribute("src")).not.toBe(srcAntes), { timeout: 3000 });
    rerender(<SessionProvidersContext.Provider value={sessaoFalsa("answered")}><VideoView onClose={() => undefined} continuar /></SessionProvidersContext.Provider>);
  });

  it("não grava por cima de uma edição do agente feita enquanto a pessoa editava", async () => {
    const pid = await criarProjeto();
    localStorage.setItem("medza.video.ultimo", pid);
    render(<VideoView onClose={() => undefined} continuar />);
    const clipe = await screen.findByRole("button", { name: /Texto Lançamento/ });
    fireEvent.keyDown(clipe, { key: "Enter" });
    const duracao = await screen.findByRole("textbox", { name: "Duração" });
    const doAgente = disco.arquivos.get(`${pid}/index.html`)!.replace("Lançamento</h1>", "Lançamento do agente</h1>");
    disco.arquivos.set(`${pid}/index.html`, doAgente);
    fireEvent.change(duracao, { target: { value: "4" } });
    fireEvent.blur(duracao);
    expect(await screen.findByText(/O agente mudou o vídeo enquanto você editava/)).toBeInTheDocument();
    expect(disco.arquivos.get(`${pid}/index.html`)).toBe(doAgente);
  });

  it("pasta do computador: escolhida no novo vídeo, ligada ao agente e trocada no editor", async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(open).mockResolvedValueOnce("/Users/x/Medza");
    render(<VideoView onClose={() => undefined} />);
    fireEvent.click(await screen.findByRole("button", { name: "Criar o primeiro vídeo" }));
    fireEvent.change(screen.getByPlaceholderText("Ex.: Lançamento do app"), { target: { value: "Reels" } });
    fireEvent.click(screen.getByRole("button", { name: "Escolher pasta…" }));
    expect(await screen.findByText("/Users/x/Medza")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Criar e abrir o editor" }));
    expect(await screen.findByLabelText("Timeline")).toBeInTheDocument();
    const pid = "v-teste-1";
    expect(JSON.parse(disco.arquivos.get(`${pid}/projeto.json`)!).pastaDoComputador).toBe("/Users/x/Medza");
    expect(disco.chamadas.find((c) => c.cmd === "videos_configurar_pastas")?.args).toMatchObject({ id: pid, pastaDoComputador: "/Users/x/Medza", referencias: [] });
    expect(disco.arquivos.get(`${pid}/BRIEF.md`)).toContain("Pasta do computador deste vídeo: `/Users/x/Medza`");
    expect(screen.getByRole("button", { name: "Medza" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Tirar a pasta do computador deste vídeo" }));
    await waitFor(() => expect(JSON.parse(disco.arquivos.get(`${pid}/projeto.json`)!).pastaDoComputador).toBeUndefined());
    expect(disco.arquivos.get(`${pid}/BRIEF.md`)).not.toContain("/Users/x/Medza");
  });

  it("pastas de referências ligam e desligam por vídeo sem copiar", async () => {
    const pid = await criarProjeto();
    disco.comando("videos_ref_criar", { dados: { nome: "Reels da Medza" } });
    localStorage.setItem("medza.video.ultimo", pid);
    render(<VideoView onClose={() => undefined} continuar />);
    fireEvent.click(await screen.findByRole("tab", { name: "Referências" }));
    const chave = await screen.findByRole("switch", { name: "desligada" });
    fireEvent.click(chave);
    await waitFor(() => expect(JSON.parse(disco.arquivos.get(`${pid}/projeto.json`)!).pastasDeReferencia).toEqual(["r-2"]));
    expect(disco.chamadas.some((c) => c.cmd === "videos_ref_aplicar")).toBe(false);
    expect(disco.chamadas.filter((c) => c.cmd === "videos_configurar_pastas").pop()?.args.referencias).toEqual(["r-2"]);
    expect(disco.arquivos.get(`${pid}/BRIEF.md`)).toContain("Reels da Medza");
    fireEvent.click(await screen.findByRole("switch", { name: "ligada" }));
    await waitFor(() => expect(JSON.parse(disco.arquivos.get(`${pid}/projeto.json`)!).pastasDeReferencia).toEqual([]));
  });

  it("edição na mão: escolher no vídeo abre as propriedades e mexer grava só o style do elemento", async () => {
    const pid = await criarProjeto();
    localStorage.setItem("medza.video.ultimo", pid);
    render(<VideoView onClose={() => undefined} continuar />);
    await screen.findByLabelText("Timeline");
    const player = await waitFor(() => { const p = document.querySelector("hyperframes-player") as HTMLElement & { iframeElement: HTMLIFrameElement }; expect(p?.iframeElement?.contentWindow).toBeTruthy(); return p; });
    const janela = player.iframeElement.contentWindow!;
    const enviadas: unknown[] = [];
    janela.postMessage = ((m: unknown) => { enviadas.push(m); }) as typeof janela.postMessage;
    // O título é a tag de abertura nº 9 do modelo (html, head, meta, meta, title, script, style, body, div, h1).
    const html = disco.arquivos.get(`${pid}/index.html`)!;
    const ordem = [...html.matchAll(/<(?!\/|!)[a-z0-9]+/gi)].findIndex((m) => m[0] === "<h1");
    const el = { ref: `index.html:${ordem}`, id: "titulo", tag: "h1", rect: { x: 10, y: 20, w: 300, h: 80 }, texto: "Lançamento", fonte: 92, peso: "700", cor: "rgb(255, 255, 255)", fundo: "rgba(0, 0, 0, 0)", translate: "", scale: "", rotate: "", visivel: true };
    await act(async () => { window.dispatchEvent(new MessageEvent("message", { source: janela, data: { medza: "ponte", tipo: "selecionar", el } })); });
    // Mensagem que não veio do iframe do player é ignorada.
    await act(async () => { window.dispatchEvent(new MessageEvent("message", { source: window, data: { medza: "ponte", tipo: "selecionar", el: { ...el, texto: "falso" } } })); });

    const x = await screen.findByRole("textbox", { name: "Posição X" });
    expect(screen.getByRole("textbox", { name: "Texto" })).toHaveValue("Lançamento");
    fireEvent.change(x, { target: { value: "120" } });
    fireEvent.blur(x);
    await waitFor(() => expect(disco.arquivos.get(`${pid}/index.html`)).toContain('style="translate: 120px 0px;"'));
    expect(enviadas).toContainEqual({ tipo: "estilo", ref: el.ref, props: { translate: "120px 0px" }, medza: "editor" });
    const gravou = disco.chamadas.find((c) => c.cmd === "videos_gravar" && c.args.arquivo === "index.html");
    expect(gravou?.args.motivo).toMatch(/Movi/);

    const texto = screen.getByRole("textbox", { name: "Texto" });
    fireEvent.change(texto, { target: { value: "Promo <nova>" } });
    fireEvent.blur(texto);
    await waitFor(() => expect(disco.arquivos.get(`${pid}/index.html`)).toContain(">Promo &lt;nova&gt;</h1>"));
    // O resto do arquivo não mudou (só o style e o texto do título).
    const depois = disco.arquivos.get(`${pid}/index.html`)!;
    expect(depois.replace(' style="translate: 120px 0px;"', "").replace("Promo &lt;nova&gt;", "Lançamento")).toBe(html);
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
    // Fork: "Mostrar no Finder" pelo Rust (o opener recusava a pasta do projeto).
    fireEvent.click(within(dialogo).getByRole("button", { name: "Mostrar no Finder" }));
    await waitFor(() => expect(disco.chamadas.find((c) => c.cmd === "videos_abrir_render")?.args).toMatchObject({ id: pid, mostrar: true }));
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
