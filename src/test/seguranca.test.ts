import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { normalizarMidias } from "../features/video/useProjeto";

/**
 * Fork (segurança, 0.34.0): travas que só existem em configuração e somem sem ninguém notar.
 *
 * - O escopo de disco do app (`fs:scope` da janela e `assetProtocol`) continua `**` (o explorador de
 *   arquivos abre qualquer projeto), mas nega chaves, tokens e arquivos que dão persistência.
 * - Link de referência do vídeo só http(s): `asset://…` abria um arquivo do disco **como o app**.
 */
const raiz = process.cwd();
const conf = JSON.parse(readFileSync(join(raiz, "src-tauri", "tauri.conf.json"), "utf8"));
const capacidade = JSON.parse(readFileSync(join(raiz, "src-tauri", "capabilities", "default.json"), "utf8"));
const escopoFs = capacidade.permissions.find((p: { identifier?: string }) => p && p.identifier === "fs:scope");

const OBRIGATORIOS = [
  "$HOME/.ssh/**", "$HOME/.tauri/**", "$HOME/.zshrc", "$HOME/Library/LaunchAgents/**", "$HOME/Library/Keychains/**",
  "$HOME/Library/Application Support/*/engine.token", "$HOME/Library/Application Support/*/mobile.json",
  "**/.credentials.json", "**/.claude.json",
];

describe("escopos de disco do app", () => {
  it("a janela nega chaves, tokens e arquivos de inicialização", () => {
    for (const caminho of OBRIGATORIOS) expect(escopoFs.deny, caminho).toContain(caminho);
  });

  it("o protocolo asset nega os mesmos caminhos", () => {
    const escopo = conf.app.security.assetProtocol.scope;
    for (const caminho of OBRIGATORIOS) expect(escopo.deny, caminho).toContain(caminho);
  });

  it("a interface não abre caminho arbitrário pelo opener", () => {
    expect(capacidade.permissions).not.toContain("opener:allow-open-path");
  });
});

describe("links de referência do vídeo", () => {
  it("só http(s) sobrevive ao midias.json", () => {
    const midias = normalizarMidias({
      links: [
        { url: "https://youtube.com/watch?v=1", nota: "" },
        { url: "asset://localhost/%2FUsers%2Fx%2Fa.html", nota: "" },
        { url: "javascript:alert(1)", nota: "" },
        { url: "file:///etc/passwd", nota: "" },
      ],
    } as never);
    expect(midias.links.map((l) => l.url)).toEqual(["https://youtube.com/watch?v=1"]);
  });
});
