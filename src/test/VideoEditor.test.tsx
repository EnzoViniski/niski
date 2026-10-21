// Fork (Editor de vídeo): telas com disco, IA, render e player falsos — criar o vídeo, editar pela
// timeline e pelo chat, desfazer, exportar e pastas de referências.
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { VideoView } from "../features/video/VideoView";
import { lerComposicao } from "../features/video/composicao";
import { definirTokenParaTeste } from "../features/video/enderecos";
import { indexInicial } from "../features/video/modelos";

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

/** Disco falso com as regras dos comandos `videos_*` que a tela usa. */
function discoFalso() {
  const arquivos = new Map<string, string>();
  const versoes = new Map<string, { n: number; motivo: string; arquivos: Record<string, string> }[]>();
  const chamadas: { cmd: string; args: Record<string, unknown> }[] = [];
  let ordem: string[] = [];
  let pastas: Record<string, unknown>[] = [];
  let n = 0;
  const ler = (id: string, rel: string) => arquivos.get(`${id}/${rel}`) ?? null;
  const gravar = (id: string, rel: string, c: string) => arquivos.set(`${id}/${rel}`, c);
  const guardar = (id: string, motivo: string) => {
    const lista = versoes.get(id) ?? [];
    const atual = { "index.html": ler(id, "index.html") ?? "", "BRIEF.md": ler(id, "BRIEF.md") ?? "", "midias.json": ler(id, "midias.json") ?? "" };
    const ultima = lista[lista.length - 1];
    if (ultima && JSON.stringify(ultima.arquivos) === JSON.stringify(atual)) return null;
    const v = { n: (ultima?.n ?? 0) + 1, motivo, arquivos: atual };
    versoes.set(id, [...lista, v]);
    return v.n;
  };
  let agente: (texto: string, id: string) => { texto: string; arquivos: string[] } = () => ({ texto: "ok", arquivos: [] });
  return {
    arquivos, chamadas, versoes,
    definirAgente(f: typeof agente) { agente = f; },
    comando(cmd: string, args: Record<string, unknown>) {
      chamadas.push({ cmd, args });
      const id = args.id as string;
      switch (cmd) {
        case "videos_endereco_base": return "tok";
        case "videos_listar": return ordem.map((i) => ({ id: i, projeto: JSON.parse(ler(i, "projeto.json")!) }));
        case "videos_criar": {
          const novo = args.novo as { projeto: Record<string, unknown>; brief: string; index: string; instrucoes: string };
          const pid = `v-teste-${++n}`;
          gravar(pid, "projeto.json", JSON.stringify({ ...novo.projeto, id: pid, versao: 1, hyperframes: "0.8.183", criadoEm: 1, atualizadoEm: 1 }));
          gravar(pid, "BRIEF.md", novo.brief); gravar(pid, "index.html", novo.index); gravar(pid, "CLAUDE.md", novo.instrucoes);
          gravar(pid, "midias.json", JSON.stringify({ versao: 1, itens: [], links: [] }));
          ordem = [pid, ...ordem];
          return pid;
        }
        case "videos_abrir": return `/dados/videos/projetos/${id}`;
        case "videos_ler": return ler(id, args.arquivo as string);
        case "videos_gravar": {
          const rel = args.arquivo as string;
          if (rel.endsWith(".json")) JSON.parse(args.conteudo as string);
          const v = args.motivo ? guardar(id, args.motivo as string) : null;
          gravar(id, rel, args.conteudo as string);
          return v;
        }
        case "videos_anexar_conversa": gravar(id, "conversa.jsonl", `${ler(id, "conversa.jsonl") ?? ""}${args.linha as string}\n`); return null;
        case "videos_criar_versao": return guardar(id, args.motivo as string);
        case "videos_versoes": return [...(versoes.get(id) ?? [])].reverse().map((v) => ({ n: v.n, motivo: v.motivo, quando: 1, arquivos: Object.keys(v.arquivos) }));
        case "videos_restaurar_versao": {
          const v = (versoes.get(id) ?? []).find((x) => x.n === args.n);
          if (!v) throw new Error("Essa versão não existe.");
          const refazer = guardar(id, `Antes de voltar para a versão ${v.n}`);
          for (const [rel, c] of Object.entries(v.arquivos)) gravar(id, rel, c);
          return refazer;
        }
        case "videos_midias_faltando": return [];
        case "videos_importar_midia": {
          const caminho = args.caminho as string;
          const nome = caminho.split("/").pop()!;
          const m = JSON.parse(ler(id, "midias.json")!);
          const item = { id: `h-${nome}`, nome, arquivo: `assets/${nome.toLowerCase()}`, tipo: nome.endsWith(".mp4") ? "video" : "imagem", papel: args.papel, hash: "h", tamanho: 10, nota: "", adicionadoEm: 1 };
          m.itens.push(item);
          gravar(id, "midias.json", JSON.stringify(m));
          return item;
        }
        case "videos_ref_listar": return pastas;
        case "videos_ref_criar": { const p = { versao: 1, id: `r-${++n}`, nome: (args.dados as { nome: string }).nome, descricao: "", cor: "#7c9cff", itens: [], links: [], criadoEm: 1, atualizadoEm: 1 }; pastas = [p, ...pastas]; return p; }
        case "videos_ref_ler": return pastas.find((p) => p.id === id);
        case "videos_ref_aplicar": return [];
        case "videos_agente": {
          const pedido = args.pedido as { projeto: string; texto: string };
          const r = agente(pedido.texto, pedido.projeto);
          return { texto: r.texto, sessao: "sessao-falsa", modelo: "modelo-falso", arquivos: r.arquivos };
        }
        case "videos_agente_rodando": return false;
        case "videos_exportar": return `/dados/videos/projetos/${(args.pedido as { projeto: string }).projeto}/renders/x.mp4`;
        case "list_profiles": return [{ id: "claude-1", provider: "claude", name: "Conta 1", config_dir: "/x", builtin: true, created_at_ms: 0, last_used_at_ms: null, authenticated: true }];
        case "agentes_modelos": return { claude: [{ id: "sonnet", label: "Sonnet" }], codex: [] };
        default: return undefined;
      }
    },
  };
}

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
    const clipe = await screen.findByRole("button", { name: /Bloco Lançamento/ });
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
