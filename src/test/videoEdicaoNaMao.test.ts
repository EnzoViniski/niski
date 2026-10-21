// Fork (Editor de vídeo): edição na mão — a posição `n` das tags tem de bater com a do Rust
// (`src-tauri/src/videos/ponte_do_editor.rs`, mesmo texto de amostra) e as edições só mexem no lugar certo.
import { describe, expect, it } from "vitest";
import { definirEstilo, definirTexto, lerComposicao, lerEstilo, lerRef, lerTags, ordemDaTag, refDoClipe, soTexto } from "../features/video/composicao";

const AMOSTRA = "<!doctype html><html><head><title>a<b></title><style>.x>p{}</style></head><body>" +
  "<!-- <div> --><div id=\"root\" data-composition-id=\"main\"><h1 class='t' data-x=\"a>b\">Oi</h1><img src=x.png>" +
  "<script>if (a<b) document.write('<p>')</script><p>fim</p></div></body></html>";

describe("edição na mão no preview", () => {
  it("conta as tags de abertura igual ao Rust", () => {
    const abertas = lerTags(AMOSTRA).filter((t) => t.tipo === "abre").map((t) => t.nome);
    expect(abertas).toEqual(["html", "head", "title", "style", "body", "div", "h1", "img", "script", "p"]);
    expect(ordemDaTag(AMOSTRA, AMOSTRA.indexOf("<h1"))).toBe(6);
  });

  it("lê a referência da ponte e recusa o que não for arquivo do projeto", () => {
    expect(lerRef("index.html:12")).toEqual({ arquivo: "index.html", ordem: 12 });
    expect(lerRef("compositions/cena-1.html:3")).toEqual({ arquivo: "compositions/cena-1.html", ordem: 3 });
    for (const r of ["../x.html:1", "index.html", "projeto.json:1", "compositions/../a.html:1", null]) expect(lerRef(r)).toBeNull();
  });

  it("muda só o style do elemento, preservando o resto da tag e do arquivo", () => {
    const html = '<div id="root" data-composition-id="main"><h1 id="t" class="clip" style="color: red; translate: 1px 2px" data-start="0">Oi</h1></div>';
    const novo = definirEstilo(html, 1, { translate: "40px -12px", "font-size": "80px", color: null });
    expect(novo).toBe('<div id="root" data-composition-id="main"><h1 id="t" class="clip" style="translate: 40px -12px; font-size: 80px;" data-start="0">Oi</h1></div>');
    expect(lerEstilo('a: 1; B : 2 ;;').get("b")).toBe("2");
    const semEstilo = definirEstilo('<p id="x">a</p>', 0, { scale: "1.5" });
    expect(semEstilo).toBe('<p id="x" style="scale: 1.5;">a</p>');
    expect(() => definirEstilo(html, 1, { color: "red; background: url(x)" })).toThrow();
    expect(() => definirEstilo(html, 1, { "x}": "1" })).toThrow();
  });

  it("troca o texto de quem só tem texto, escapando HTML", () => {
    const html = '<div id="root"><h1 id="t">Oi</h1><p>a <b>b</b></p></div>';
    expect(soTexto(html, 1)).toBe(true);
    expect(soTexto(html, 2)).toBe(false);
    expect(definirTexto(html, 1, "Olá <mundo> & cia")).toBe('<div id="root"><h1 id="t">Olá &lt;mundo&gt; &amp; cia</h1><p>a <b>b</b></p></div>');
    expect(() => definirTexto(html, 2, "x")).toThrow(/outros elementos/);
  });

  it("acha a referência do clipe da timeline", () => {
    const html = '<html><body><div id="root" data-composition-id="main" data-width="1080" data-height="1920" data-duration="5"><h1 id="t" class="clip" data-start="0" data-duration="2">Oi</h1></div></body></html>';
    const clipe = lerComposicao(html).clipes[0];
    expect(refDoClipe(html, clipe)).toBe("index.html:3");
  });
});
