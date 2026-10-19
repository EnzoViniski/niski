import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * `npm run dev` nunca pode encostar no OMNI instalado.
 *
 * O `build-engine.mjs`, ao achar o binário de dev travado, manda `shutdown` para um engine. Ele
 * mirava a porta e o token do app **instalado** — e todo `npm run dev` derrubava o app e as sessões
 * abertas nele. Hoje script e Rust leem a identidade de dev do mesmo `identidade.json`; este teste
 * impede que algum dos dois volte a ter cópia própria e que as portas/pastas colidam.
 */
const script = readFileSync(resolve("scripts/build-engine.mjs"), "utf8");
const protocolo = readFileSync(resolve("crates/omni-protocol/src/lib.rs"), "utf8");
const identidade = JSON.parse(readFileSync(resolve("identidade.json"), "utf8"));

describe("isolamento entre dev e app instalado", () => {
  it("script e Rust tiram a identidade de dev do identidade.json", () => {
    expect(script).toContain("const DEV_ENGINE_PORT = IDENTIDADE.portaDeDev;");
    expect(script).toContain("const DEV_DATA_DIR = `${IDENTIDADE.bundleId}.dev`;");
    expect(protocolo).toContain('pub const DEV_ENGINE_PORT: u16 = porta_valida(env!("OMNI_ID_PORTA_DEV"));');
    expect(protocolo).toContain('pub const DEV_DATA_DIR: &str = concat!(env!("OMNI_ID_BUNDLE"), ".dev");');
  });

  it("dev, instalado e celular nunca compartilham porta", () => {
    const portas = [identidade.portaDoEngine, identidade.portaDeDev, identidade.portaDoCelular];
    expect(new Set(portas).size).toBe(3);
  });

  it("o script não referencia a porta nem a pasta do app instalado fora dos comentários", () => {
    const codigo = script.split("\n").filter((linha) => !linha.trim().startsWith("//")).join("\n");
    expect(codigo).not.toMatch(/47_?321/);
    expect(codigo).not.toContain(identidade.bundleId);
  });
});
