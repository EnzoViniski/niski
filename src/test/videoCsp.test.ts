// Fork (Editor de vídeo): o preview e as miniaturas vêm do protocolo `medzavideo:` (src-tauri/src/videos/protocolo.rs).
// O CSP não vale em `tauri dev`; só este teste pega a falta da liberação no app instalado.
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const CSP: string = JSON.parse(readFileSync(join(process.cwd(), "src-tauri", "tauri.conf.json"), "utf8")).app.security.csp;

function diretiva(nome: string) {
  const m = CSP.match(new RegExp(`(?:^|;)\\s*${nome}\\s([^;]*)`));
  if (!m) throw new Error(`diretiva ${nome} ausente do CSP`);
  return m[1];
}

describe("CSP do editor de vídeo", () => {
  for (const nome of ["img-src", "media-src", "frame-src"]) {
    it(`${nome} libera o protocolo do projeto nas duas formas (macOS e Windows)`, () => {
      expect(diretiva(nome)).toContain("medzavideo:");
      expect(diretiva(nome)).toContain("http://medzavideo.localhost");
    });
  }

  it("a janela continua sem script de fora (a composição roda isolada no iframe)", () => {
    expect(CSP).not.toMatch(/script-src[^;]*medzavideo/);
    expect(CSP).not.toMatch(/script-src[^;]*https:/);
  });
});
