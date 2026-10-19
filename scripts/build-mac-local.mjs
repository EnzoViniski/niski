// `.app` local para testar no macOS **ao lado** do app instalado, aberto pelo Finder.
//
// O app aberto pelo Finder não recebe variáveis de ambiente, então porta e pasta de dados vão
// gravadas na compilação (`OMNI_PORTA_DO_ENGINE`, `OMNI_PASTA_DE_DADOS`, lidas pelo omni-protocol).
// O bundle id também muda: é ele que separa o armazenamento da webview (localStorage) e o nome da
// pasta de logs. Sem updater: o build local não pode se trocar pelo release do autor.
//
// Uso: node scripts/build-mac-local.mjs   (só a arquitetura da máquina, só o .app)
import { execFileSync } from "node:child_process";
import { statSync } from "node:fs";
import { resolve } from "node:path";
import { gravarSobreposicao, lerIdentidade, sobreposicaoDoTauri } from "./identidade.mjs";

if (process.platform !== "darwin") {
  console.error("build-mac-local: só no macOS.");
  process.exit(1);
}

// Derivado de `identidade.json`: mesmo produto, com sufixo " Local", bundle id `.local` e porta
// do engine + 60 (ex.: 47321 → 47381), para nunca encostar no app instalado.
const base = lerIdentidade();
const identidade = {
  porta: process.env.OMNI_PORTA_DO_ENGINE ?? String(base.portaDoEngine + 60),
  pasta: process.env.OMNI_PASTA_DE_DADOS ?? `${base.bundleId}.local`,
  nome: process.env.OMNI_NOME_LOCAL ?? `${base.nome} Local`,
};

const configLocal = gravarSobreposicao(sobreposicaoDoTauri(base, {
  nome: identidade.nome,
  bundleId: identidade.pasta,
  bundle: { createUpdaterArtifacts: false },
  // Sem isto o build local ainda consultaria o latest.json do produto e, numa release nova,
  // ofereceria trocar este .app pelo build publicado (a assinatura confere). Endereço que nunca
  // responde: a checagem falha calada.
  updater: { endpoints: ["https://127.0.0.1:9/sem-atualizacao/latest.json"] },
}), "tauri.local.conf.json");

const env = {
  ...process.env,
  OMNI_PORTA_DO_ENGINE: identidade.porta,
  OMNI_PASTA_DE_DADOS: identidade.pasta,
};
const rodar = (cmd, args) => execFileSync(cmd, args, { stdio: "inherit", env });

rodar(process.execPath, ["scripts/sync-version.mjs"]);
rodar(process.execPath, ["scripts/build-engine.mjs", "--release"]);
rodar(process.execPath, [resolve("node_modules/@tauri-apps/cli/tauri.js"), "build", "--bundles", "app", "--config", configLocal]);

// Fork (energia, 2026-10-19): o .app de 13:13 saiu com o engine de **debug** (31 MB, sem otimização,
// ~8% de CPU parado): um `npm run build:engine` rodado no meio do build trocou
// `src-tauri/binaries/` antes do bundle copiar. O release tem ~4 MB; acima do dobro, é debug.
const engineDoBundle = resolve("target/release/bundle/macos", `${identidade.nome}.app`, "Contents/MacOS/omni-engine");
const engineRelease = resolve("target/release/omni-engine");
if (statSync(engineDoBundle).size > 2 * statSync(engineRelease).size) {
  console.error(`\nbuild-mac-local: o engine dentro do .app é o de debug (${engineDoBundle}).`);
  console.error("Algo rodou `npm run build:engine` durante o build. Rode este script de novo sem nada em paralelo.");
  process.exit(1);
}

console.log(`\nPronto: target/release/bundle/macos/${identidade.nome}.app`);
console.log(`Porta ${identidade.porta}, dados em ~/Library/Application Support/${identidade.pasta}, logs em ~/Library/Logs/${identidade.pasta}/`);
