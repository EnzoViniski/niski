// Fork (Editor de vídeo, edição a dois): o editor do host abre a sessão, manda os arquivos e grava
// (com versão) o que chega do convidado. Rust, Tailscale e rede falsos.
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { VideoView } from "../features/video/VideoView";
import { DocumentoADois } from "../features/video/aDois/documento";
import { definirTokenParaTeste } from "../features/video/enderecos";
import { indexInicial } from "../features/video/modelos";
import { discoFalso } from "./videoDiscoFalso";

vi.mock("@hyperframes/player", () => ({}));

let disco: ReturnType<typeof discoFalso>;
let ouvinte: ((e: { payload: unknown }) => void) | null = null;

beforeEach(() => {
  disco = discoFalso();
  definirTokenParaTeste("tok");
  vi.mocked(invoke).mockImplementation(async (cmd, args) => disco.comando(cmd, (args ?? {}) as Record<string, unknown>));
  vi.mocked(listen).mockImplementation(async (nome, f) => { if (nome === "video-sessao") ouvinte = f as typeof ouvinte; return () => { if (nome === "video-sessao") ouvinte = null; }; });
});
afterEach(() => { vi.mocked(invoke).mockReset(); vi.mocked(invoke).mockResolvedValue("{}"); vi.mocked(listen).mockReset(); vi.mocked(listen).mockResolvedValue(() => {}); definirTokenParaTeste(null); ouvinte = null; });

describe("edição a dois no editor", () => {
  it("host abre a sessão, manda os arquivos e grava o que o convidado mudou", async () => {
    const pid = disco.comando("videos_criar", { novo: {
      projeto: { nome: "Promo", formato: "16:9", largura: 1920, altura: 1080, fps: 30, cor: "#fff", ia: { cli: "claude" }, briefing: {}, pastasDeReferencia: [], sessoes: {} },
      brief: "# Brief\n", index: indexInicial({ nome: "Promo", formato: "16:9", duracao: 10 }), instrucoes: "x",
    } }) as string;
    localStorage.setItem("medza.video.ultimo", pid);
    render(<VideoView onClose={() => undefined} continuar />);
    fireEvent.click(await screen.findByRole("button", { name: "A dois" }));
    fireEvent.click(await screen.findByRole("button", { name: "Abrir sessão a dois" }));
    expect(await screen.findByText("medza-video:convite-falso")).toBeInTheDocument();
    await waitFor(() => expect(ouvinte).not.toBeNull());

    // O convidado (outro Y.Doc) recebe tudo o que o host mandou.
    const doConvidado: string[] = [];
    const primo = new DocumentoADois({ enviar: (c) => { doConvidado.push(c); } }, { nome: "Primo", cor: "#0f0" }, false);
    await waitFor(() => expect(disco.sessao.enviadas.length).toBeGreaterThan(0));
    for (const c of disco.sessao.enviadas) primo.receber(c);
    expect(primo.arquivos.get("index.html")).toContain('data-composition-id="main"');
    expect(primo.arquivos.get("BRIEF.md")).toBe("# Brief\n");

    // O primo muda a duração; chega ao host pelo evento do Rust.
    primo.definirArquivo("index.html", primo.arquivos.get("index.html")!.replace('data-duration="10" data-fps', 'data-duration="20" data-fps'));
    await act(async () => { for (const corpo of doConvidado.splice(0)) ouvinte?.({ payload: { tipo: "mensagem", de: "c1", login: "primo@exemplo.com", corpo } }); });
    await waitFor(() => expect(disco.arquivos.get(`${pid}/index.html`)).toContain('data-duration="20" data-fps'));
    const gravacao = disco.chamadas.find((c) => c.cmd === "videos_gravar" && String(c.args.motivo).startsWith("Mudança de"));
    expect(gravacao?.args.motivo).toBe("Mudança de Primo");

    // Pedido de alguém fora da lista aparece para o host permitir.
    await act(async () => { ouvinte?.({ payload: { tipo: "pedido", login: "estranho@exemplo.com" } }); });
    expect(await screen.findByText(/estranho@exemplo.com quer entrar/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Permitir" }));
    await waitFor(() => expect(disco.chamadas.some((c) => c.cmd === "videos_sessao_permitir" && c.args.login === "estranho@exemplo.com")).toBe(true));

    fireEvent.click(screen.getByRole("button", { name: "Encerrar a sessão" }));
    await waitFor(() => expect(disco.sessao.papel).toBeNull());
    primo.sair();
  });
});
