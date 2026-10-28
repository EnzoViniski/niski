#!/usr/bin/env node
// Compila o auxiliar de voz do Jarvis (`src-tauri/jarvis-voz/main.swift`) para
// `src-tauri/binaries/medza-voz-<triple>`, ao lado do engine. Só no Mac: ele usa o reconhecimento de
// fala da Apple (macOS 26+). O Tauri copia e assina como os outros binários (`externalBin` entra pela
// sobreposição de `scripts/identidade.mjs`). Em dev o app acha o binário direto em `binaries/`.
//
// Uso: node scripts/build-voz-do-jarvis.mjs [--release] [--target <triple>]
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { resolve } from "node:path";

if (process.platform !== "darwin") {
  console.log("Voz do Jarvis: só no macOS; pulando.");
  process.exit(0);
}

const release = process.argv.includes("--release");
const indice = process.argv.indexOf("--target");
const pedido = indice >= 0 ? process.argv[indice + 1] : process.env.TAURI_ENV_TARGET_TRIPLE;
const host = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (\S+)/m)[1];
const triple = pedido ?? host;
// Inclui os arquivos de política/testes sem concentrar tudo no protocolo de áudio.
const fontes = readdirSync(resolve("src-tauri", "jarvis-voz")).filter((nome) => nome.endsWith(".swift"))
  .map((nome) => resolve("src-tauri", "jarvis-voz", nome));
const pasta = resolve("src-tauri", "binaries");
mkdirSync(pasta, { recursive: true });

const ARQUITETURA = { "aarch64-apple-darwin": "arm64", "x86_64-apple-darwin": "x86_64" };

function compilar(alvo, saida) {
  const arq = ARQUITETURA[alvo];
  if (!arq) throw new Error(`Voz do Jarvis: alvo sem suporte: ${alvo}`);
  // Arquivo novo em vez de sobrescrever: um auxiliar aberto continua com o binário antigo.
  if (existsSync(saida)) rmSync(saida);
  execFileSync("xcrun", ["swiftc", release ? "-O" : "-Onone", "-swift-version", "5",
    "-module-cache-path", resolve("target-tauri", "swift-module-cache"),
    "-target", `${arq}-apple-macos26.0`, ...fontes, "-o", saida], { stdio: "inherit" });
  return saida;
}

const destino = resolve(pasta, `medza-voz-${triple}`);
const atualizado = existsSync(destino) && fontes.every((fonte) => statSync(destino).mtimeMs > statSync(fonte).mtimeMs);
if (triple === "universal-apple-darwin") {
  const partes = Object.keys(ARQUITETURA).map((alvo) => compilar(alvo, resolve(pasta, `medza-voz-${alvo}`)));
  execFileSync("lipo", ["-create", "-output", destino, ...partes], { stdio: "inherit" });
} else if (!atualizado || release) {
  compilar(triple, destino);
}
console.log(`Voz do Jarvis em src-tauri/binaries/medza-voz-${triple}.`);
