#!/usr/bin/env node
/**
 * Gera o `installers/latest.json` que o updater do app consulta.
 *
 * Roda depois do `npm run build` (o collect-installers já trouxe os artefatos assinados e os
 * `.sig` para `installers/`). Monta `platforms` com todas as plataformas que tiverem artefato
 * assinado da versão atual. Montar esse arquivo à mão a cada release é pedir para errar a
 * assinatura e derrubar a atualização de todo mundo.
 *
 * Uso:
 *   node scripts/make-latest-json.mjs
 *   node scripts/make-latest-json.mjs --notes "Visualizador de PDF e visual novo."
 *   node scripts/make-latest-json.mjs --dir caminho/dos/artefatos
 */
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/**
 * Chaves de plataforma do updater do Tauri v2 (`OS-ARCH`) e, em ordem de preferência, o sufixo do
 * artefato assinado que cada uma consome. `base` = `<Nome.Com.Pontos>_<versao>`.
 *
 * - macOS: `<Produto>.app.tar.gz` (o bundler não põe versão nem arquitetura no nome; o
 *   collect-installers renomeia para `<base>_<aarch64|x64>.app.tar.gz` para os dois não colidirem).
 * - Windows: o `-setup.exe` é assinado direto (`createUpdaterArtifacts: true`); `.nsis.zip` e MSI só
 *   existem em builds antigos / `v1Compatible`. NSIS antes de MSI: é o `windows.installMode` padrão.
 * - Linux: o próprio `.AppImage` é o artefato assinado (`.AppImage.tar.gz` só no `v1Compatible`).
 */
const PLATAFORMAS = [
  ["darwin-aarch64", ["_aarch64.app.tar.gz"]],
  ["darwin-x86_64", ["_x64.app.tar.gz"]],
  ["windows-x86_64", ["_x64-setup.nsis.zip", "_x64-setup.exe", "_x64_en-US.msi"]],
  ["linux-x86_64", ["_amd64.AppImage", "_amd64.AppImage.tar.gz"]],
];

/** Nome local do asset: o GitHub troca espaço por ponto no upload, então já saímos assim. */
export function nomeDeAsset(nome) {
  return nome.replaceAll(" ", ".");
}

/**
 * Monta o conteúdo do `latest.json` a partir da lista de nomes de arquivo de uma pasta.
 * Pura: a assinatura chega por `lerAssinatura(arquivo)` (recebe o nome do `.sig`).
 *
 * @param {object} p
 * @param {string[]} p.arquivos         nomes (não caminhos) presentes na pasta de artefatos
 * @param {string} p.versao             x.y.z
 * @param {string} p.nome               nome do produto (com espaços)
 * @param {string} p.repositorio        repositório PÚBLICO de releases (`dono/repo`)
 * @param {(arquivoSig: string) => string} p.lerAssinatura
 * @param {string} [p.notas]
 * @param {string} [p.urlBase]          troca a base das URLs (teste local); padrão: download da tag no GitHub
 * @param {string} [p.dataDePublicacao] RFC 3339; padrão: agora
 * @returns {{ json: object, usados: Record<string, string>, tag: string }}
 */
export function montarLatestJson({ arquivos, versao, nome, repositorio, lerAssinatura, notas, urlBase, dataDePublicacao }) {
  const tag = `v${versao}`;
  const base = `${nomeDeAsset(nome)}_${versao}`;
  const raizDasUrls = (urlBase ?? `https://github.com/${repositorio}/releases/download/${tag}`).replace(/\/+$/, "");
  const presentes = new Set(arquivos);

  const platforms = {};
  const usados = {};
  for (const [plataforma, sufixos] of PLATAFORMAS) {
    const artefato = sufixos.map((s) => `${base}${s}`).find((a) => presentes.has(a) && presentes.has(`${a}.sig`));
    if (!artefato) continue;
    platforms[plataforma] = {
      // A assinatura vai INLINE no json — o updater não busca o .sig separado.
      signature: lerAssinatura(`${artefato}.sig`).trim(),
      url: `${raizDasUrls}/${encodeURIComponent(artefato)}`,
    };
    usados[plataforma] = artefato;
  }

  if (Object.keys(platforms).length === 0) {
    const daVersao = arquivos.filter((f) => f.includes(`_${versao}_`));
    const procurados = PLATAFORMAS.flatMap(([, sufixos]) => sufixos.map((s) => `${base}${s}`));
    throw new Error(
      `Não achei artefato ASSINADO da versão ${versao} (nenhum par arquivo + .sig).\n` +
        `  Procurei: ${procurados.join(", ")}\n` +
        `  O que existe da ${versao}: ${daVersao.length ? daVersao.join(", ") : "nada"}\n` +
        "  Sem o .sig, o build não assinou. Confira:\n" +
        '  - "createUpdaterArtifacts": true (identidade.json precisa de updater.pubkey + updater.endpoint)\n' +
        "  - TAURI_SIGNING_PRIVATE_KEY e TAURI_SIGNING_PRIVATE_KEY_PASSWORD no ambiente do build"
    );
  }

  return {
    json: {
      version: versao,
      notes: notas ?? `Versão ${versao}.`,
      pub_date: dataDePublicacao ?? new Date().toISOString(),
      platforms,
    },
    usados,
    tag,
  };
}

/** Lê identidade e pasta e devolve o resultado de `montarLatestJson`. Lança Error com mensagem clara. */
export function prepararLatestJson({ dir, notas, versao }) {
  const identidade = JSON.parse(readFileSync(join(RAIZ, "identidade.json"), "utf8"));
  if (!identidade.updater?.pubkey || !identidade.updater?.endpoint) {
    throw new Error("Atualização automática desativada. Distribua os instaladores; não há chave própria configurada.");
  }
  const repositorio = identidade.repositorioDeReleases;
  if (typeof repositorio !== "string" || !repositorio) {
    throw new Error('identidade.json: "repositorioDeReleases" vazio (repositório público onde o updater baixa).');
  }
  if (!existsSync(dir)) throw new Error(`${dir} não existe. Rode "npm run build" antes.`);
  return montarLatestJson({
    arquivos: readdirSync(dir),
    versao: versao ?? readFileSync(join(RAIZ, "VERSION"), "utf8").trim(),
    nome: identidade.nome,
    repositorio,
    notas,
    lerAssinatura: (sig) => readFileSync(join(dir, sig), "utf8"),
  });
}

function valorDe(flag) {
  const i = process.argv.indexOf(flag);
  if (i < 0) return undefined;
  const v = process.argv[i + 1];
  if (!v || v.startsWith("--")) {
    console.error(`[latest.json] Falta valor para ${flag}`);
    process.exit(1);
  }
  return v;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const dir = resolve(valorDe("--dir") ?? join(RAIZ, "installers"));
  try {
    const { json, usados, tag } = prepararLatestJson({ dir, notas: valorDe("--notes") });
    const saida = join(dir, "latest.json");
    writeFileSync(saida, JSON.stringify(json, null, 2) + "\n", "utf8");
    console.log(`[latest.json] ${saida}`);
    console.log(`  versão: ${json.version}   tag esperada: ${tag}`);
    for (const [plataforma, artefato] of Object.entries(usados)) console.log(`  ${plataforma.padEnd(15)} ${artefato}`);
    console.log("\n>> Publique com: node scripts/publicar-atualizacao.mjs --publicar");
  } catch (erro) {
    console.error(`[latest.json] ${erro.message}`);
    process.exit(1);
  }
}
