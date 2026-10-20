#!/usr/bin/env node
/**
 * Publica uma versão no repositório PÚBLICO de releases (`repositorioDeReleases`), de onde o updater
 * do app baixa. O GITHUB_TOKEN do workflow não escreve em outro repositório; por isso a publicação
 * é feita aqui, com o `gh` já logado na máquina de quem lança a versão.
 *
 * Fluxo: confere VERSION × artefatos → gera latest.json → recusa tag existente → `gh release create`
 * com instaladores, .app.tar.gz, .sig e latest.json.
 *
 * Uso:
 *   node scripts/publicar-atualizacao.mjs                       lista o que subiria (= --dry-run)
 *   node scripts/publicar-atualizacao.mjs --dry-run
 *   node scripts/publicar-atualizacao.mjs --publicar
 *   node scripts/publicar-atualizacao.mjs --publicar --notes "O que mudou, em linguagem de usuário."
 *   node scripts/publicar-atualizacao.mjs --dir pasta            artefatos noutra pasta (padrão: installers/)
 *   node scripts/publicar-atualizacao.mjs --de-ci <run-id>       baixa os artefatos de um run do workflow
 *                                                                do repo privado (`gh run download`)
 */
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { prepararLatestJson } from "./make-latest-json.mjs";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const tem = (flag) => args.includes(flag);
const valorDe = (flag) => {
  const i = args.indexOf(flag);
  if (i < 0) return undefined;
  if (!args[i + 1] || args[i + 1].startsWith("--")) falhar(`Falta valor para ${flag}`);
  return args[i + 1];
};
function falhar(mensagem) {
  console.error(`[publicar] ${mensagem}`);
  process.exit(1);
}

/** O que sobe no Release. `.app.zip` (conveniência local) e arquivos de outras versões ficam de fora. */
const EXTENSOES = [".dmg", ".exe", ".msi", ".nsis.zip", ".appimage", ".appimage.tar.gz", ".deb", ".rpm", ".app.tar.gz", ".sig"];
const sobe = (f) => EXTENSOES.some((e) => f.toLowerCase().endsWith(e));

if (tem("--publicar") && tem("--dry-run")) falhar("Use --publicar OU --dry-run, não os dois.");
const publicar = tem("--publicar");
const idDoRun = valorDe("--de-ci");
const notas = valorDe("--notes");

const identidade = JSON.parse(readFileSync(join(RAIZ, "identidade.json"), "utf8"));
const repositorio = identidade.repositorioDeReleases;
if (!repositorio) falhar('identidade.json sem "repositorioDeReleases".');
const versao = readFileSync(join(RAIZ, "VERSION"), "utf8").trim();
const tag = `v${versao}`;

const gh = (parametros) => execFileSync("gh", parametros, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });

/** Copia `origem` (recursivo) para `destino` achatado; nome repetido é erro, não sobrescrita. */
function achatar(origem, destino) {
  for (const entrada of readdirSync(origem, { withFileTypes: true })) {
    const caminho = join(origem, entrada.name);
    if (entrada.isDirectory()) achatar(caminho, destino);
    else if (existsSync(join(destino, entrada.name))) falhar(`Dois artefatos com o mesmo nome: ${entrada.name}`);
    else copyFileSync(caminho, join(destino, entrada.name));
  }
}

let temporaria = null;
let dir;
if (idDoRun) {
  if (!/^\d+$/.test(idDoRun)) falhar(`--de-ci espera o número do run, recebi "${idDoRun}".`);
  temporaria = mkdtempSync(join(tmpdir(), "medza-ci-"));
  const baixado = join(temporaria, "baixado");
  dir = join(temporaria, "artefatos");
  mkdirSync(dir);
  console.log(`[publicar] baixando artefatos do run ${idDoRun} de ${identidade.repositorio}...`);
  try {
    gh(["run", "download", idDoRun, "--repo", identidade.repositorio, "--dir", baixado]);
  } catch (erro) {
    falhar(`gh run download falhou: ${String(erro.stderr ?? erro.message).trim()}`);
  }
  achatar(baixado, dir);
} else {
  dir = resolve(valorDe("--dir") ?? join(RAIZ, "installers"));
  if (!existsSync(dir)) falhar(`${dir} não existe. Rode "npm run build" ou use --de-ci <run-id>.`);
}

try {
  // 1. VERSION × artefatos (nome do arquivo carrega a versão: <Produto>_<versao>_<arch>…).
  const todos = readdirSync(dir).filter((f) => statSync(join(dir, f)).isFile());
  const versoesNaPasta = new Set(todos.map((f) => f.match(/_(\d+\.\d+\.\d+)_/)?.[1]).filter(Boolean));
  const daVersao = todos.filter((f) => f.includes(`_${versao}_`) && sobe(f));
  if (daVersao.length === 0) {
    falhar(
      `Nenhum artefato da versão ${versao} (arquivo VERSION) em ${dir}.\n` +
        `  Versões encontradas nos nomes: ${[...versoesNaPasta].join(", ") || "nenhuma"}.\n` +
        "  Ajuste VERSION + npm run version:sync e gere o build de novo, ou aponte o --dir/--de-ci certo."
    );
  }
  const conf = JSON.parse(readFileSync(join(RAIZ, "src-tauri", "tauri.conf.json"), "utf8"));
  if (conf.version !== versao) falhar(`VERSION (${versao}) diverge do tauri.conf.json (${conf.version}). Rode npm run version:sync.`);
  const outras = [...versoesNaPasta].filter((v) => v !== versao);
  if (outras.length) console.warn(`[publicar] aviso: ignorando artefatos de outras versões (${outras.join(", ")}).`);

  // 2. latest.json (falha com mensagem clara se não houver nenhum artefato assinado).
  let pronto;
  try {
    pronto = prepararLatestJson({ dir, notas, versao });
  } catch (erro) {
    falhar(erro.message);
  }
  const { json: latest, usados } = pronto;

  // 3. A tag não pode existir (nem como rascunho): republicar com outro conteúdo quebraria quem já baixou.
  let tagLivre = true;
  try {
    gh(["release", "view", tag, "--repo", repositorio]);
    tagLivre = false;
  } catch (erro) {
    const saida = String(erro.stderr ?? erro.message);
    if (!/release not found/i.test(saida)) {
      const msg = `não consegui consultar ${repositorio} (${saida.trim().split("\n")[0]}). O gh está logado?`;
      if (publicar) falhar(msg);
      console.warn(`[publicar] aviso: ${msg}`);
    }
  }
  if (!tagLivre) falhar(`A tag ${tag} já existe em ${repositorio}. Suba a versão no VERSION; não republico em cima.`);

  // 4. O que sobe.
  const arquivos = [...daVersao];
  const caminhoDoJson = publicar ? join(dir, "latest.json") : null;
  console.log(`[publicar] ${publicar ? "PUBLICANDO" : "dry-run (nada será enviado)"}  ${tag} -> ${repositorio}`);
  console.log(`  pasta: ${dir}`);
  for (const f of arquivos.sort()) console.log(`  ${(statSync(join(dir, f)).size / 1024 / 1024).toFixed(1).padStart(7)} MB  ${f}`);
  console.log(`          latest.json  plataformas: ${Object.entries(usados).map(([p, a]) => `${p} <- ${a}`).join("; ")}`);
  console.log(`  notas: ${latest.notes}`);

  if (!publicar) {
    console.log("\nPara publicar de verdade: acrescente --publicar.");
  } else {
    writeFileSync(caminhoDoJson, JSON.stringify(latest, null, 2) + "\n", "utf8");
    const titulo = `${identidade.nome} ${tag}`;
    gh(["release", "create", tag, "--repo", repositorio, "--title", titulo, "--notes", latest.notes, ...arquivos.map((f) => join(dir, f)), caminhoDoJson]);
    console.log(`\n[publicar] Release ${tag} criado em https://github.com/${repositorio}/releases/tag/${tag}`);
    console.log("  Confira o endpoint: " + identidade.updater.endpoint);
  }
} finally {
  // Em dry-run mantém a pasta baixada para reaproveitar com --dir; ao publicar, limpa.
  if (temporaria && publicar) rmSync(temporaria, { recursive: true, force: true });
  else if (temporaria) console.log(`[publicar] artefatos do CI mantidos em ${dir} (use --dir para publicar sem baixar de novo).`);
}
