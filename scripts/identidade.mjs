#!/usr/bin/env node
/**
 * Identidade do produto (`identidade.json` na raiz) → sobreposição do `tauri.conf.json`.
 *
 * O `tauri.conf.json` fica como o autor do upstream deixou (merge sem conflito a cada release
 * dele); nome, bundle id, título da janela e updater do nosso produto entram por
 * `tauri build --config target-tauri/tauri.identidade.conf.json`. Rust e interface leem o mesmo
 * arquivo por conta própria (`crates/omni-protocol/build.rs`, `src/lib/identidade.ts`).
 *
 * Uso:
 *   node scripts/identidade.mjs     gera a sobreposição e imprime o caminho
 */
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export function lerIdentidade() {
  const id = JSON.parse(readFileSync(join(RAIZ, "identidade.json"), "utf8"));
  for (const campo of ["nome", "nomeCurto", "bundleId", "pastaDePerfis", "repositorio"]) {
    if (typeof id[campo] !== "string" || !id[campo]) throw new Error(`identidade.json: "${campo}" vazio`);
  }
  for (const campo of ["portaDoEngine", "portaDeDev", "portaDoCelular"]) {
    if (!Number.isInteger(id[campo]) || id[campo] < 1 || id[campo] > 65535) throw new Error(`identidade.json: "${campo}" não é porta`);
  }
  if (!/^[a-z0-9]+(\.[a-z0-9-]+)+$/i.test(id.bundleId)) throw new Error(`identidade.json: bundleId "${id.bundleId}" inválido`);
  return id;
}

/** Janela igual à do `tauri.conf.json` (ou do `tauri.macos.conf.json` no Mac, que o Tauri aplicaria
 *  por cima), só com o título trocado: a sobreposição substitui o array inteiro. */
function janelas(titulo) {
  const ler = (nome) => JSON.parse(readFileSync(join(RAIZ, "src-tauri", nome), "utf8"));
  const base = process.platform === "darwin" ? ler("tauri.macos.conf.json") : ler("tauri.conf.json");
  return base.app.windows.map((janela) => (janela.label === "main" ? { ...janela, title: titulo } : janela));
}

/** Sobreposição para `tauri build --config`. `ajuste` permite derivar variantes (build local). */
export function sobreposicaoDoTauri(id, ajuste = {}) {
  const nome = ajuste.nome ?? id.nome;
  // Sem chave própria, distribuição manual. Nunca herdar a chave/endpoint do upstream.
  const assinado = Boolean(id.updater?.pubkey && id.updater?.endpoint);
  const updater = ajuste.updater ?? { endpoints: assinado ? [id.updater.endpoint] : [], pubkey: id.updater?.pubkey ?? "" };
  return {
    productName: nome,
    identifier: ajuste.bundleId ?? id.bundleId,
    app: { windows: janelas(nome) },
    bundle: { createUpdaterArtifacts: assinado, ...ajuste.bundle },
    ...(updater ? { plugins: { updater } } : {}),
  };
}

export function gravarSobreposicao(conteudo, arquivo = "tauri.identidade.conf.json") {
  const pasta = join(RAIZ, "target-tauri");
  mkdirSync(pasta, { recursive: true });
  const caminho = join(pasta, arquivo);
  writeFileSync(caminho, JSON.stringify(conteudo, null, 2));
  return caminho;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.log(gravarSobreposicao(sobreposicaoDoTauri(lerIdentidade())));
}
