import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { ArquivosDoAgente } from "../mobile/ArquivosDoAgente";
import { UltimoLugarDoVideo, ultimosLugares, videosQueTocamAqui } from "../mobile/VideoDoAgente";
import { usarTransporte } from "../mobile/api";

// Fork (vídeo na conversa): o vídeo do agente toca na conversa do celular, por bilhete do engine.
let pedidos: string[] = [];
beforeEach(() => {
  pedidos = [];
  usarTransporte(async (caminho, init) => {
    pedidos.push(`${init.method ?? "GET"} ${caminho}`);
    return new Response(JSON.stringify({ url: "/video/bilhete123", nome: "final.mp4", tipo: "video/mp4", tamanho: 10 }), { status: 200 });
  });
});
afterEach(() => usarTransporte((path, init) => fetch(path, init)));

describe("vídeo do agente na conversa", () => {
  it("toca no player com o endereço do bilhete; o resto continua cartão", async () => {
    render(<ArquivosDoAgente conversa="c 1" n={4} caminhos={["/p/relatorio.pdf", "/p/renders/r1/final.mp4"]} />);
    const player = await screen.findByLabelText("Vídeo final.mp4");
    expect(player.tagName).toBe("VIDEO");
    expect(player).toHaveAttribute("src", "/video/bilhete123");
    expect(pedidos).toEqual(["POST /conversas/c%201/videos/4/1"]);
    expect(screen.getByText("relatorio.pdf")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Salvar ou compartilhar final.mp4" })).toBeInTheDocument();
  });

  it("o mesmo vídeo em duas mensagens toca só na última", async () => {
    const lugares = ultimosLugares([{ n: 2, arquivos: ["/p/v.mp4"] }, { n: 7, arquivos: ["/p/v.mp4", "/p/x.pdf"] }]);
    expect([...lugares]).toEqual([["/p/v.mp4", 7]]);
    render(<UltimoLugarDoVideo.Provider value={lugares}><ArquivosDoAgente conversa="c" n={2} caminhos={["/p/v.mp4"]} /></UltimoLugarDoVideo.Provider>);
    expect(screen.getByText("toca mais abaixo")).toBeInTheDocument();
    expect(pedidos).toEqual([]);
  });

  it("trabalho recolhido mostra o vídeo que não foi citado depois", () => {
    const blocos = [{ tipo: "acoes", acoes: [{ id: "c:3", arquivos: ["/p/a.mp4", "/p/b.mp4"] }, { id: "c:4" }] }];
    const lugares = new Map([["/p/a.mp4", 3], ["/p/b.mp4", 9]]);
    expect(videosQueTocamAqui(blocos, lugares)).toEqual([{ caminho: "/p/a.mp4", n: 3, k: 0 }]);
  });

  it("vídeo que ainda não existe avisa e deixa tentar de novo", async () => {
    usarTransporte(async () => new Response(JSON.stringify({ error: "Vídeo não encontrado" }), { status: 404 }));
    render(<ArquivosDoAgente conversa="c" n={1} caminhos={["/p/ainda.mp4"]} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("ainda não está pronto");
    await waitFor(() => expect(screen.getByRole("button", { name: "Tentar de novo" })).toBeInTheDocument());
  });
});
