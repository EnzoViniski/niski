import { describe, expect, it } from "vitest";
import { montarLatestJson } from "../../scripts/make-latest-json.mjs";

/**
 * `latest.json` é o que decide se o app dos outros atualiza. Assinatura trocada ou URL que não bate
 * com o nome do asset no GitHub falham calados (404 / "assinatura inválida"). Fixtures sintéticas:
 * nenhuma assinatura real aqui.
 */
const base = {
  versao: "0.8.2",
  nome: "Medza Agents",
  repositorio: "Medza-Primos-Organizacao/medza-agentes-releases",
  dataDePublicacao: "2026-10-20T12:00:00.000Z",
  lerAssinatura: (sig: string) => `ASSINATURA-DE(${sig})\n`,
};
const MAC = ["Medza.Agents_0.8.2_aarch64.app.tar.gz", "Medza.Agents_0.8.2_aarch64.app.tar.gz.sig", "Medza.Agents_0.8.2_aarch64.dmg"];
const WIN = ["Medza.Agents_0.8.2_x64-setup.exe", "Medza.Agents_0.8.2_x64-setup.exe.sig"];

describe("montarLatestJson", () => {
  it("monta mac arm64 + windows com assinatura inline e URL do repo de releases", () => {
    const { json, usados, tag } = montarLatestJson({ ...base, arquivos: [...MAC, ...WIN], notas: "Novidades." });
    expect(tag).toBe("v0.8.2");
    expect(json.version).toBe("0.8.2");
    expect(json.notes).toBe("Novidades.");
    expect(json.pub_date).toBe("2026-10-20T12:00:00.000Z");
    expect(Object.keys(json.platforms).sort()).toEqual(["darwin-aarch64", "windows-x86_64"]);
    expect(json.platforms["darwin-aarch64"]).toEqual({
      signature: "ASSINATURA-DE(Medza.Agents_0.8.2_aarch64.app.tar.gz.sig)",
      url: "https://github.com/Medza-Primos-Organizacao/medza-agentes-releases/releases/download/v0.8.2/Medza.Agents_0.8.2_aarch64.app.tar.gz",
    });
    expect(json.platforms["windows-x86_64"].url).toMatch(/\/releases\/download\/v0\.8\.2\/Medza\.Agents_0\.8\.2_x64-setup\.exe$/);
    expect(usados["darwin-aarch64"]).toBe("Medza.Agents_0.8.2_aarch64.app.tar.gz");
  });

  it("só mac: não inventa as outras plataformas", () => {
    const { json } = montarLatestJson({ ...base, arquivos: MAC });
    expect(Object.keys(json.platforms)).toEqual(["darwin-aarch64"]);
    expect(json.notes).toBe("Versão 0.8.2.");
  });

  it("arm64 e x64 do mac ficam em chaves separadas", () => {
    const x64 = ["Medza.Agents_0.8.2_x64.app.tar.gz", "Medza.Agents_0.8.2_x64.app.tar.gz.sig"];
    const { json } = montarLatestJson({ ...base, arquivos: [...MAC, ...x64] });
    expect(Object.keys(json.platforms).sort()).toEqual(["darwin-aarch64", "darwin-x86_64"]);
  });

  it("linux usa o AppImage assinado", () => {
    const { json } = montarLatestJson({ ...base, arquivos: ["Medza.Agents_0.8.2_amd64.AppImage", "Medza.Agents_0.8.2_amd64.AppImage.sig"] });
    expect(Object.keys(json.platforms)).toEqual(["linux-x86_64"]);
  });

  it("artefato sem .sig não conta", () => {
    const { json } = montarLatestJson({ ...base, arquivos: ["Medza.Agents_0.8.2_aarch64.app.tar.gz", ...WIN] });
    expect(Object.keys(json.platforms)).toEqual(["windows-x86_64"]);
  });

  it("ignora artefatos de outra versão", () => {
    const antigos = ["Medza.Agents_0.8.1_aarch64.app.tar.gz", "Medza.Agents_0.8.1_aarch64.app.tar.gz.sig"];
    expect(() => montarLatestJson({ ...base, arquivos: antigos })).toThrow(/versão 0\.8\.2/);
  });

  it("nenhum artefato assinado: erro claro com o que existe", () => {
    expect(() => montarLatestJson({ ...base, arquivos: ["Medza.Agents_0.8.2_aarch64.dmg"] }))
      .toThrow(/Não achei artefato ASSINADO[\s\S]*Medza\.Agents_0\.8\.2_aarch64\.dmg/);
    expect(() => montarLatestJson({ ...base, arquivos: [] })).toThrow(/nada/);
  });

  it("codifica o nome do asset na URL (espaço vira ponto antes, o resto é percent-encoded)", () => {
    const nome = "Medza Ágents";
    const arquivo = "Medza.Ágents_0.8.2_aarch64.app.tar.gz";
    const { json } = montarLatestJson({ ...base, nome, arquivos: [arquivo, `${arquivo}.sig`] });
    expect(json.platforms["darwin-aarch64"].url).toBe(
      `https://github.com/Medza-Primos-Organizacao/medza-agentes-releases/releases/download/v0.8.2/${encodeURIComponent(arquivo)}`
    );
    expect(json.platforms["darwin-aarch64"].url).toContain("%C3%81");
  });

  it("urlBase troca a origem (variante de teste local)", () => {
    const { json } = montarLatestJson({ ...base, arquivos: MAC, urlBase: "http://127.0.0.1:8765/" });
    expect(json.platforms["darwin-aarch64"].url).toBe("http://127.0.0.1:8765/Medza.Agents_0.8.2_aarch64.app.tar.gz");
  });
});
