#!/usr/bin/env node
// Exportação revisável, sem .git nem documentos/estado particulares do workspace interno.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, relative, resolve } from "node:path";
import { RAIZ, lerIdentidade } from "./identidade.mjs";

const git = (...args) => execFileSync("git", args, { cwd: RAIZ, encoding: "utf8" });
const destino = resolve(process.argv[2] ?? resolve(RAIZ, "publicacao/medza-agentes"));
if (destino === RAIZ || !relative(destino, RAIZ).startsWith("..")) throw new Error("Escolha uma pasta nova fora da raiz/ancestrais do código.");
if (existsSync(destino) && readdirSync(destino).length) throw new Error("A pasta de destino precisa estar vazia.");
if (git("status", "--porcelain").trim()) throw new Error("Faça commit das alterações antes de exportar a versão atual.");

const privado = (nome) => /^(?:roadmaps\/|docs\/fork\/|docs\/screenshots\/|\.git(?:\/|$)|\.claude\/|\.codex\/|\.agents\/|\.omni(?:-agents)?\/|installers\/|target[^/]*\/|node_modules\/|dist(?:-mobile)?\/)/.test(nome)
  || ["CLAUDE.md", "AGENTS.md", "roadmap.md", "src-tauri/Cargo.lock"].includes(nome)
  || /(?:^|\/)(?:engine\.token|mobile\.json|\.credentials\.json|\.claude\.json|\.Codex\.json|conversations\.json|sessions\.json|profiles\.json|workspace(?:-[^/]*)?\.json|memoria-compartilhada\.json)$/.test(nome)
  || /(?:^|\/)(?:arquivo-conversas|logs)(?:\/|$)/.test(nome)
  || /\.(?:key|pem|p12|pfx|jsonl|log|parcial)$/.test(nome)
  || /(?:^|\/)ggml-[^/]*\.bin$/.test(nome)
  || (/(?:^|\/)\.env(?:\.|$)/.test(nome) && nome !== ".env.example");

const arquivos = git("ls-files", "-z").split("\0").filter(Boolean).filter((nome) => !privado(nome));
const manifesto = [];
const segredo = /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|ghp_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,}|sk-ant-[A-Za-z0-9_-]{30,}/;
// Conferir tudo antes de escrever: um erro não deixa uma exportação parcial parecer pronta.
const preparados = arquivos.map((nome) => {
  if (nome.includes("..") || nome.startsWith("/")) throw new Error("Caminho inválido na lista versionada.");
  const origem = resolve(RAIZ, nome);
  const stat = lstatSync(origem);
  if (!stat.isFile()) throw new Error(`Arquivo não regular: ${nome}`);
  const bytes = readFileSync(origem);
  if (bytes.length > 25 * 1024 * 1024) throw new Error(`Arquivo grande demais para código: ${nome}`);
  if (!bytes.includes(0)) {
    const texto = bytes.toString("utf8");
    if (segredo.test(texto) || texto.includes(homedir())) throw new Error(`Revisão de privacidade necessária em ${nome}`);
  }
  return [nome, bytes, stat.mode & 0o777];
});
const guia = readFileSync(resolve(RAIZ, "docs/AGENTES-PUBLICACAO.md"));
preparados.push(["AGENTS.md", guia], ["CLAUDE.md", guia]);
mkdirSync(destino, { recursive: true });
for (const [nome, bytes, modo = 0o644] of preparados) {
  const caminho = resolve(destino, nome);
  mkdirSync(dirname(caminho), { recursive: true });
  writeFileSync(caminho, bytes, { mode: modo });
  manifesto.push({ arquivo: nome, sha256: createHash("sha256").update(bytes).digest("hex") });
}
writeFileSync(resolve(destino, "DISTRIBUICAO.json"), JSON.stringify({
  versao: readFileSync(resolve(RAIZ, "VERSION"), "utf8").trim(),
  repositorio: lerIdentidade().repositorio,
  commit_de_origem: git("rev-parse", "HEAD").trim(),
  historico_interno_incluido: false,
  arquivos: manifesto,
}, null, 2) + "\n");
console.log(`Exportação limpa: ${manifesto.length} arquivos. Revise a árvore antes de publicar.`);
