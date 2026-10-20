#!/usr/bin/env node
/**
 * Copia os artefatos do `tauri build` para `installers/` na raiz.
 *
 * Existe porque o caminho real é fundo demais para achar à mão:
 *   target/release/bundle/{nsis,msi}/…
 *
 * Só copia — não apaga o original nem limpa a pasta de destino. Manter a versão
 * anterior por perto é útil para testar uma atualização de N-1 para N.
 */
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");
// `src-tauri` virou membro de um Cargo workspace (crates/omni-engine, omni-protocol) — o target/
// compartilhado do workspace fica na raiz do repo, não mais em src-tauri/target/.
const indiceDoAlvo = process.argv.indexOf("--target");
const alvo = indiceDoAlvo >= 0 ? process.argv[indiceDoAlvo + 1] : undefined;
const BUNDLE = join(RAIZ, "target", ...(alvo ? [alvo] : []), "release", "bundle");
const DESTINO = join(RAIZ, "installers");

/** Instaladores + os artefatos que o updater do Tauri publica no Release. */
const EXTENSOES = [".exe", ".msi", ".zip", ".sig", ".dmg", ".deb", ".rpm", ".appimage", ".tar.gz"];

/**
 * `target/` guarda artefatos de nomes de produto antigos para sempre — o build
 * nunca limpa. Sem este filtro a pasta herdava os 687 MB de "PDF to Markdown"
 * junto com o instalador atual.
 */
const PRODUTO = JSON.parse(
  readFileSync(join(RAIZ, "identidade.json"), "utf8")
).nome;
const VERSAO = readFileSync(join(RAIZ, "VERSION"), "utf8").trim();
const NOMES = [PRODUTO, PRODUTO.replaceAll(" ", "."), PRODUTO.toLowerCase().replaceAll(" ", "-"), PRODUTO.toLowerCase().replaceAll(" ", "_")];

function arquivosDoBundle() {
  if (!existsSync(BUNDLE)) return [];
  return readdirSync(BUNDLE, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .flatMap((d) => {
      const sub = join(BUNDLE, d.name);
      return readdirSync(sub)
        .filter((f) => NOMES.some((nome) => f.startsWith(`${nome}_${VERSAO}_`)))
        .filter((f) => EXTENSOES.some((e) => f.toLowerCase().endsWith(e)))
        .map((f) => join(sub, f));
    });
}

const encontrados = arquivosDoBundle();
// `.app.zip` também funciona sem montar um DMG, inclusive em ambientes sem hdiutil.
const app = join(BUNDLE, "macos", `${PRODUTO}.app`);
let zipDoApp = null;
if (process.platform === "darwin" && existsSync(app)) {
  const versaoDoApp = execFileSync("plutil", ["-extract", "CFBundleShortVersionString", "raw", "-o", "-", join(app, "Contents/Info.plist")], { encoding: "utf8" }).trim();
  if (versaoDoApp === VERSAO) {
    mkdirSync(DESTINO, { recursive: true });
    zipDoApp = join(DESTINO, `${PRODUTO.replaceAll(" ", ".")}_${VERSAO}_${alvo ?? process.arch}.app.zip`);
    execFileSync("ditto", ["-c", "-k", "--sequesterRsrc", "--keepParent", app, zipDoApp]);
    console.log(`[installers] ${zipDoApp}`);
  }
}

if (encontrados.length === 0 && !zipDoApp) {
  // Não é erro: `npm run build:vite` sozinho não gera bundle. Falhar aqui
  // quebraria o script de build por um passo que nem devia ter rodado.
  console.warn(`[installers] Nada de "${PRODUTO}" em ${BUNDLE} — rode "npm run build" antes.`);
  process.exit(0);
}

mkdirSync(DESTINO, { recursive: true });

const mb = (n) => (n / 1024 / 1024).toFixed(1).padStart(6);
console.log(`[installers] -> ${DESTINO}`);
for (const origem of encontrados) {
  // O GitHub troca espaço por ponto no nome de todo asset de Release, sempre — sem
  // controle nenhum da nossa parte. Se copiássemos com o espaço original, o nome local
  // e o nome real do asset no Release divergiriam, e make-latest-json.mjs geraria uma
  // URL que nunca bate (404 em silêncio — armadilha já documentada no CLAUDE.md).
  // Renomeando aqui, o nome local já sai igual ao que o GitHub vai produzir.
  const nome = origem.split(/[\\/]/).pop().replaceAll(" ", ".");
  copyFileSync(origem, join(DESTINO, nome));
  console.log(`  ${mb(statSync(origem).size)} MB  ${nome}`);
}
