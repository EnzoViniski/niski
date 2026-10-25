import { describe, expect, it } from "vitest";
import { capituloComDefeito, dividirCapitulo, lerBiblia, limparTexto, marcadorDepois, passagemAtual, passagens, referencia, versiculosDa, type Capitulo } from "./biblia";
import { arquivoDeTeste } from "./bibliaDeTeste";

const cap = (textos: string[]): Capitulo => ({ n: 1, versiculos: textos.map((texto, i) => ({ n: i + 1, texto })) });

describe("Bíblia do dia: texto e passagens", () => {
  it("lê os 73 livros e recusa arquivo incompleto", () => {
    expect(lerBiblia(arquivoDeTeste()).livros).toHaveLength(73);
    const incompleto = JSON.parse(arquivoDeTeste()) as { novoTestamento: unknown[] };
    incompleto.novoTestamento.pop();
    expect(() => lerBiblia(JSON.stringify(incompleto))).toThrow(/incompleto/);
    expect(() => lerBiblia("{}")).toThrow(/formato/);
  });

  it("tira só as marcas de nota e os hífens invisíveis", () => {
    expect(limparTexto("contraria­dos porque ensinavam,*  e  disse:")).toBe("contrariados porque ensinavam, e disse:");
  });

  it("corta no fim de frase, entre o mínimo e o máximo, sem deixar sobra solta", () => {
    const partes = dividirCapitulo(cap(["a,", "b,", "c.", "d,", "e.", "f,", "g,", "h."]), 2, 4);
    expect(partes).toEqual([[1, 3], [4, 5], [6, 8]]);
    expect(dividirCapitulo(cap(["a.", "b.", "c.", "d.", "e."]), 2, 4)).toEqual([[1, 2], [3, 5]]);
  });

  it("deixa de fora capítulo com versículo grudado ou vazio", () => {
    expect(capituloComDefeito(cap(["Tal é a lei, 30 ou quando o espírito.", "fim."]))).toBe(true);
    expect(capituloComDefeito(cap(["[]", "fim."]))).toBe(true);
    expect(capituloComDefeito(cap(["Eram seus 80 irmãos.", "fim."]))).toBe(false);
    expect(capituloComDefeito({ n: 1, versiculos: [{ n: 1, texto: "a." }, { n: 3, texto: "b." }] })).toBe(true);
  });

  it("salmo curto vem inteiro e a referência segue o jeito da Bíblia", () => {
    const b = lerBiblia(arquivoDeTeste());
    const [primeiro] = passagens(b, "salmos");
    expect([primeiro.de, primeiro.ate]).toEqual([1, 12]);
    expect(referencia(b, primeiro)).toBe("Salmo 1");
    const [ev] = passagens(b, "evangelhos");
    expect(referencia(b, ev)).toBe("São Mateus 1, 1-6");
    expect(versiculosDa(b, ev)).toHaveLength(6);
  });

  it("anda pela trilha sem repetir e recomeça no fim contando a volta", () => {
    const b = lerBiblia(arquivoDeTeste());
    const lista = passagens(b, "evangelhos");
    let marcador;
    const vistas = new Set<string>();
    for (let i = 0; i < lista.length; i++) {
      const atual = passagemAtual(b, "evangelhos", marcador);
      expect(atual?.indice).toBe(i);
      vistas.add(atual!.passagem.id);
      marcador = marcadorDepois(b, atual!.passagem, marcador);
    }
    expect(vistas.size).toBe(lista.length);
    expect(marcador?.ciclo).toBe(1);
    expect(passagemAtual(b, "evangelhos", marcador)?.indice).toBe(0);
  });
});
