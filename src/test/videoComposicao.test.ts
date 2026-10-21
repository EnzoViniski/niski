// Fork (Editor de vídeo): leitura e edição da composição direto no texto do HTML.
import { describe, expect, it } from "vitest";
import {
  alternarOculto, apagar, aparar, definirDuracaoTotal, dividir, duplicar, duracaoTotal, inserirMidia, inserirTexto,
  lerComposicao, lerTags, moverClipe,
} from "../features/video/composicao";
import { atualizarReferenciasNoBrief, gerarBrief, indexInicial, instrucoesDoAgente, secaoDeReferencias } from "../features/video/modelos";
import type { Midia } from "../features/video/tipos";

const HTML = `<!doctype html>
<html>
  <head>
    <script src="https://cdn.jsdelivr.net/npm/gsap@3.14.2/dist/gsap.min.js"></script>
    <style>.clip { position: absolute; } /* <div data-start="9"> não é clipe */</style>
  </head>
  <body>
    <div id="root" data-composition-id="main" data-start="0" data-width="1080" data-height="1920" data-duration="10">
      <!-- <video data-start="1"></video> comentado -->
      <video id="take" src="assets/take.mp4" data-start="0" data-duration="4" data-media-start="2" data-track-index="0" playsinline data-has-audio="true"></video>
      <section id="titulo-card" class="clip" data-start="4" data-duration="3" data-track-index="1">
        <h1 id="titulo">Olá &amp; bem-vindo</h1>
      </section>
      <img class="clip" src='assets/logo.png' data-start="7" data-track-index="2" />
      <audio id="musica" src="assets/musica.mp3" data-start="0" data-duration="10" data-volume="0.6" data-track-index="3"></audio>
      <div id="depende" class="clip" data-start="titulo-card.end" data-duration="1"></div>
    </div>
    <script>
      const tl = gsap.timeline({ paused: true });
      tl.from("#titulo", { opacity: 0 }, 0); // "<div data-start=\\"5\\">"
      window.__timelines["main"] = tl;
    </script>
  </body>
</html>
`;

describe("lerComposicao", () => {
  it("acha o root e só os clipes de verdade (pula comentário, script e style)", () => {
    const comp = lerComposicao(HTML);
    expect(comp.raiz).toMatchObject({ id: "main", largura: 1080, altura: 1920, duracao: 10 });
    expect(comp.clipes.map((c) => c.chave)).toEqual(["take", "titulo-card", "@2", "musica", "depende"]);
    const [video, titulo, logo, musica, depende] = comp.clipes;
    expect(video).toMatchObject({ tipo: "video", inicio: 0, duracao: 4, midiaInicio: 2, faixa: 0, rotulo: "take.mp4" });
    expect(titulo).toMatchObject({ tipo: "elemento", rotulo: "Olá & bem-vindo", faixa: 1 });
    expect(logo).toMatchObject({ tipo: "imagem", duracao: undefined, src: "assets/logo.png", editavel: true });
    expect(musica).toMatchObject({ tipo: "audio", volume: 0.6 });
    expect(depende.editavel).toBe(false);
    expect(duracaoTotal(comp)).toBe(10);
  });

  it("lê atributos com aspas simples, sem aspas e entidades", () => {
    const tags = lerTags(`<a href='x"y' data-x=3 title="a &quot;b&quot;">`);
    expect(tags[0].atributos.get("href")?.valor).toBe('x"y');
    expect(tags[0].atributos.get("data-x")?.valor).toBe("3");
    expect(tags[0].atributos.get("title")?.valor).toBe('a "b"');
  });
});

describe("edições", () => {
  it("mover só troca os atributos e cresce o root se passar do fim", () => {
    const novo = moverClipe(HTML, "titulo-card", 8.5, 4);
    const c = lerComposicao(novo).clipes.find((x) => x.chave === "titulo-card")!;
    expect(c).toMatchObject({ inicio: 8.5, faixa: 4 });
    expect(lerComposicao(novo).raiz?.duracao).toBe(11.5);
    // O resto do arquivo fica igual.
    expect(novo.replace(/data-start="8\.5" data-duration="3" data-track-index="4"/, "")).toBe(
      HTML.replace(/data-start="4" data-duration="3" data-track-index="1"/, "").replace('data-duration="10"', 'data-duration="11.5"'),
    );
  });

  it("clipe sem id ganha um id na primeira edição", () => {
    const novo = moverClipe(HTML, "@2", 6);
    const logo = lerComposicao(novo).clipes[2];
    expect(logo.id).toBe("logo-png");
    expect(logo.inicio).toBe(6);
  });

  it("não move clipe cujo início depende de outro", () => {
    expect(() => moverClipe(HTML, "depende", 2)).toThrow(/depende de outro/);
  });

  it("aparar o começo de um vídeo avança o corte dentro da mídia", () => {
    const novo = aparar(HTML, "take", "inicio", 1.5);
    expect(lerComposicao(novo).clipes[0]).toMatchObject({ inicio: 1.5, duracao: 2.5, midiaInicio: 3.5 });
    const fim = aparar(HTML, "take", "fim", 3);
    expect(lerComposicao(fim).clipes[0]).toMatchObject({ inicio: 0, duracao: 3, midiaInicio: 2 });
  });

  it("dividir cria a segunda parte com ids novos e a mídia continua de onde parou", () => {
    const novo = dividir(HTML, "take", 1);
    const clipes = lerComposicao(novo).clipes;
    expect(clipes[0]).toMatchObject({ chave: "take", inicio: 0, duracao: 1, midiaInicio: 2 });
    expect(clipes[1]).toMatchObject({ chave: "take-b", inicio: 1, duracao: 3, midiaInicio: 3 });
    const comFilhos = dividir(HTML, "titulo-card", 5);
    expect(comFilhos).toContain('id="titulo-b"');
    expect(lerComposicao(comFilhos).clipes.find((c) => c.chave === "titulo-card-b")).toMatchObject({ inicio: 5, duracao: 2 });
    expect(() => dividir(HTML, "take", 6)).toThrow(/dentro do clipe/);
  });

  it("duplicar põe a cópia logo depois", () => {
    const novo = duplicar(HTML, "titulo-card");
    expect(lerComposicao(novo).clipes.find((c) => c.chave === "titulo-card-copia")).toMatchObject({ inicio: 7, duracao: 3 });
  });

  it("apagar tira o elemento inteiro e ocultar é reversível", () => {
    const sem = apagar(HTML, "titulo-card");
    expect(sem).not.toContain('id="titulo-card"');
    expect(sem).not.toContain("Olá");
    expect(lerComposicao(sem).clipes).toHaveLength(4);
    const oculto = alternarOculto(HTML, "musica");
    expect(lerComposicao(oculto).clipes[3].oculto).toBe(true);
    expect(alternarOculto(oculto, "musica")).toBe(HTML);
  });

  it("inserir mídia e texto entra dentro do root, antes do fechamento", () => {
    const comVideo = inserirMidia(HTML, { arquivo: "assets/broll.mp4", tipo: "video", nome: "B-roll.mp4", duracao: 12 }, 2, 5);
    const comp = lerComposicao(comVideo);
    const broll = comp.clipes.find((c) => c.src === "assets/broll.mp4")!;
    expect(broll).toMatchObject({ id: "b-roll", inicio: 2, duracao: 12, faixa: 5 });
    expect(comp.raiz?.duracao).toBe(14);
    expect(comVideo.indexOf("assets/broll.mp4")).toBeLessThan(comVideo.indexOf("<script>\n      const tl"));
    const comTexto = inserirTexto(HTML, "Compre <já>", 1, 2, 6);
    expect(comTexto).toContain("Compre &lt;já&gt;");
    expect(lerComposicao(comTexto).clipes.some((c) => c.rotulo === "Compre <já>")).toBe(true);
    expect(() => inserirMidia(HTML, { arquivo: "assets/f.woff2", tipo: "fonte", nome: "f.woff2" }, 0, 0)).toThrow();
  });

  it("muda a duração total", () => {
    expect(lerComposicao(definirDuracaoTotal(HTML, 20)).raiz?.duracao).toBe(20);
  });
});

describe("modelos do projeto", () => {
  it("composição inicial é lida pela timeline", () => {
    const html = indexInicial({ nome: "Lançamento <Medza>", formato: "9:16", duracao: 12 });
    const comp = lerComposicao(html);
    expect(comp.raiz).toMatchObject({ largura: 1080, altura: 1920, duracao: 12, id: "main" });
    expect(comp.clipes).toHaveLength(1);
    expect(html).toContain("Lançamento &lt;Medza&gt;");
    expect(html).toContain('window.__timelines["main"]');
  });

  it("instruções proíbem nuvem, render e versão solta", () => {
    const texto = instrucoesDoAgente();
    expect(texto).toContain("npx --yes hyperframes@0.8.183 lint");
    expect(texto).toMatch(/Não rode `render`/);
    expect(texto).toContain("HYPERFRAMES_NO_TELEMETRY=1");
  });

  it("brief separa o que usar do que é só referência e troca só a seção marcada", () => {
    const midia = (nome: string, papel: "usar" | "referencia"): Midia => ({
      id: nome, nome, arquivo: `assets/${nome}`, tipo: "video", papel, hash: "h", tamanho: 1, nota: "", adicionadoEm: 0,
    });
    const brief = gerarBrief("Promo", { descricao: "Vídeo rápido", estilo: "Neon", duracao: 20, formato: "1:1", publico: "", plataforma: "Reels", observacoes: "" },
      [midia("a.mp4", "usar"), midia("b.mp4", "referencia")], [{ url: "https://x.com/v", nota: "ritmo" }], [{ nome: "Reels", descricao: "cortes secos" }]);
    expect(brief).toContain("1:1 (1080×1080)");
    expect(brief).toMatch(/Para usar no vídeo:\n\n- `assets\/a\.mp4`/);
    expect(brief).toMatch(/Só referência de estilo.*\n\n- `assets\/b\.mp4`/);
    expect(brief).toContain("https://x.com/v (ritmo)");
    expect(brief).toContain("**Reels**: cortes secos");
    const editado = brief.replace("Vídeo rápido", "Vídeo rápido, editado à mão");
    const novo = atualizarReferenciasNoBrief(editado, secaoDeReferencias([], [], []));
    expect(novo).toContain("editado à mão");
    expect(novo).toContain("Nenhuma mídia para usar no vídeo ainda.");
    expect(novo).not.toContain("assets/a.mp4");
  });
});
