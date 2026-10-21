// Fork (0.9.4): cor do ícone por tipo de arquivo.
import { describe, expect, it } from "vitest";
import { corDoArquivo, tintaDoArquivo, tintaDoCaminho, MAPAS_DE_COR } from "../features/files/coresDosArquivos";
import { MAPAS_DE_ICONE } from "../features/files/fileIcons";

describe("cores dos arquivos", () => {
  it("segue a mesma precedência dos ícones: pasta, nome exato, extensão", () => {
    expect(corDoArquivo("src", true)).toBe(MAPAS_DE_COR.PASTAS.src);
    expect(corDoArquivo("minha-pasta", true)).toBeTruthy();
    expect(corDoArquivo("package.json", false)).toBe(MAPAS_DE_COR.NOMES["package.json"]);
    expect(corDoArquivo("dados.json", false)).toBe(MAPAS_DE_COR.EXTENSOES.json);
    expect(corDoArquivo("App.TSX", false)).toBe(MAPAS_DE_COR.EXTENSOES.tsx);
  });

  it("dotfile e arquivo sem extensão conhecida ficam sem cor (cor do texto)", () => {
    expect(corDoArquivo(".bashrc", false)).toBeUndefined();
    expect(corDoArquivo("NOTAS", false)).toBeUndefined();
    expect(tintaDoArquivo("sem.extensaoqualquer")).toEqual({ className: "" });
  });

  it("toda cor é hexadecimal válida e todo tipo com ícone próprio comum tem cor", () => {
    for (const mapa of Object.values(MAPAS_DE_COR)) {
      for (const cor of Object.values(mapa)) expect(cor).toMatch(/^#[0-9a-f]{6}$/i);
    }
    const semCor = Object.keys(MAPAS_DE_ICONE.EXTENSOES).filter((ext) => !MAPAS_DE_COR.EXTENSOES[ext]);
    // Os poucos sem cor são de propósito (tipos raros com desenho genérico).
    expect(semCor.length).toBeLessThanOrEqual(12);
  });

  it("tinta pelo caminho completo usa só o nome do arquivo", () => {
    const tinta = tintaDoCaminho("/Users/x/projeto/src/main.rs");
    expect(tinta.className).toBe("icone-colorido");
    expect(tinta.style).toEqual({ "--cor-arquivo": MAPAS_DE_COR.EXTENSOES.rs });
  });
});
