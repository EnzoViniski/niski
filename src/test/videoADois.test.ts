// Fork (Editor de vídeo, edição a dois): dois Y.Doc conversando por um transporte falso que faz o
// papel do host (repassa para todos os outros, como o `sessao_http.rs`).
import { afterEach, describe, expect, it } from "vitest";
import { DocumentoADois } from "../features/video/aDois/documento";

function sala() {
  const membros: { id: string; doc?: DocumentoADois }[] = [];
  const filas: (() => void)[] = [];
  const entrar = (id: string, host: boolean, nome: string) => {
    const membro: { id: string; doc?: DocumentoADois } = { id };
    membros.push(membro);
    membro.doc = new DocumentoADois({ enviar: (corpo) => { for (const outro of membros) if (outro !== membro) filas.push(() => outro.doc?.receber(corpo)); } }, { nome, cor: "#fff" }, host);
    return membro.doc;
  };
  /** Entrega tudo o que está na fila (como a rede, só que na hora). */
  const entregar = () => { for (let i = 0; i < 50 && filas.length; i++) filas.splice(0).forEach((f) => f()); };
  const sair = (doc: DocumentoADois) => { doc.sair(); entregar(); const i = membros.findIndex((m) => m.doc === doc); if (i >= 0) membros.splice(i, 1); };
  return { entrar, entregar, sair, membros };
}

let abertos: DocumentoADois[] = [];
afterEach(() => { for (const d of abertos) { try { d.sair(); } catch { /* já saiu */ } } abertos = []; });

describe("edição a dois (Yjs)", () => {
  it("quem entra recebe os arquivos do host e as edições vão nos dois sentidos", () => {
    const s = sala();
    const host = s.entrar("host", true, "Enzo");
    host.definirArquivo("index.html", "<div>v1</div>");
    host.definirArquivo("BRIEF.md", "# Brief");
    const primo = s.entrar("primo", false, "Primo");
    abertos.push(host, primo);
    const noPrimo: [string, string | undefined][] = [];
    primo.ouvirArquivos((rel, c) => noPrimo.push([rel, c]));
    // Antes de sincronizar, o convidado não espalha nada (nem estado vazio).
    expect(primo.sincronizado).toBe(false);
    primo.ola();
    s.entregar();
    expect(primo.sincronizado).toBe(true);
    expect(primo.arquivos.get("index.html")).toBe("<div>v1</div>");
    expect(noPrimo).toContainEqual(["BRIEF.md", "# Brief"]);

    const noHost: [string, string | undefined][] = [];
    host.ouvirArquivos((rel, c) => noHost.push([rel, c]));
    primo.definirArquivo("index.html", "<div>do primo</div>");
    s.entregar();
    expect(host.arquivos.get("index.html")).toBe("<div>do primo</div>");
    expect(noHost).toEqual([["index.html", "<div>do primo</div>"]]);
    // Mudança local não volta como "remota" para quem fez.
    expect(noPrimo.filter(([r]) => r === "index.html")).toHaveLength(1);
  });

  it("os dois gravando o mesmo arquivo ao mesmo tempo terminam iguais (um vence, nada mistura)", () => {
    const s = sala();
    const host = s.entrar("host", true, "Enzo");
    host.definirArquivo("index.html", "base");
    const primo = s.entrar("primo", false, "Primo");
    abertos.push(host, primo);
    primo.ola(); s.entregar();
    host.definirArquivo("index.html", "versão do Enzo");
    primo.definirArquivo("index.html", "versão do primo");
    s.entregar();
    expect(host.arquivos.get("index.html")).toBe(primo.arquivos.get("index.html"));
    expect(["versão do Enzo", "versão do primo"]).toContain(host.arquivos.get("index.html"));
  });

  it("presença mostra o outro, a seleção e a agulha", () => {
    const s = sala();
    const host = s.entrar("host", true, "Enzo");
    const primo = s.entrar("primo", false, "Primo");
    abertos.push(host, primo);
    primo.ola(); s.entregar();
    primo.mudarPresenca({ selecionado: "take", tempo: 3.2 });
    s.entregar();
    expect(host.colegas()).toEqual([expect.objectContaining({ nome: "Primo", selecionado: "take", tempo: 3.2 })]);
    expect(primo.colegas().map((c) => c.nome)).toEqual(["Enzo"]);
  });

  it("um agente por vez: quem pegou segura até soltar ou sair", () => {
    const s = sala();
    const host = s.entrar("host", true, "Enzo");
    const primo = s.entrar("primo", false, "Primo");
    abertos.push(host, primo);
    primo.ola(); s.entregar();
    expect(host.pegarAgente("Enzo")).toBe(true);
    s.entregar();
    expect(primo.agenteCom()).toBe("Enzo");
    expect(primo.pegarAgente("Primo")).toBe(false);
    expect(host.agenteCom()).toBeNull();
    host.soltarAgente(); s.entregar();
    expect(primo.agenteCom()).toBeNull();
    expect(primo.pegarAgente("Primo")).toBe(true);
    s.entregar();
    expect(host.agenteCom()).toBe("Primo");
    // O primo caiu/saiu com a trava: ela deixa de valer.
    s.sair(primo);
    abertos = [host];
    expect(host.agenteCom()).toBeNull();
    expect(host.pegarAgente("Enzo")).toBe(true);
  });

  it("mensagem estranha é ignorada", () => {
    const s = sala();
    const host = s.entrar("host", true, "Enzo");
    abertos.push(host);
    expect(() => { host.receber("não é json"); host.receber(JSON.stringify({ t: "u" })); host.receber(JSON.stringify({ t: "x", u: "AA" })); }).not.toThrow();
  });
});
