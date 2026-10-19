// `.app` local para testar no macOS **ao lado** do app instalado, aberto pelo Finder.
//
// O app aberto pelo Finder não recebe variáveis de ambiente, então porta e pasta de dados vão
// gravadas na compilação (`OMNI_PORTA_DO_ENGINE`, `OMNI_PASTA_DE_DADOS`, lidas pelo omni-protocol).
// O bundle id também muda: é ele que separa o armazenamento da webview (localStorage) e o nome da
// pasta de logs. Sem updater: o build local não pode se trocar pelo release do autor.
//
// Uso: node scripts/build-mac-local.mjs   (só a arquitetura da máquina, só o .app)
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

if (process.platform !== "darwin") {
  console.error("build-mac-local: só no macOS.");
  process.exit(1);
}

const identidade = {
  porta: process.env.OMNI_PORTA_DO_ENGINE ?? "47381",
  pasta: process.env.OMNI_PASTA_DE_DADOS ?? "com.omni.agents.local",
  nome: process.env.OMNI_NOME_LOCAL ?? "OMNI AGENTS Local",
};

const sobreposicao = {
  productName: identidade.nome,
  identifier: identidade.pasta,
  app: { windows: [{ label: "main", title: identidade.nome, width: 1000, height: 720, minWidth: 720, minHeight: 560, resizable: true, fullscreen: false, center: true, decorations: false, shadow: true }] },
  bundle: { createUpdaterArtifacts: false },
  // Sem isto o build local ainda consultaria o latest.json do autor e, numa release nova dele,
  // ofereceria trocar este .app pelo build do autor (a assinatura confere). Endereço que nunca
  // responde: a checagem falha calada.
  plugins: { updater: { endpoints: ["https://127.0.0.1:9/sem-atualizacao/latest.json"] } },
};
const pasta = resolve("target-tauri");
mkdirSync(pasta, { recursive: true });
const configLocal = resolve(pasta, "tauri.local.conf.json");
writeFileSync(configLocal, JSON.stringify(sobreposicao, null, 2));

const env = {
  ...process.env,
  OMNI_PORTA_DO_ENGINE: identidade.porta,
  OMNI_PASTA_DE_DADOS: identidade.pasta,
};
const rodar = (cmd, args) => execFileSync(cmd, args, { stdio: "inherit", env });

rodar(process.execPath, ["scripts/sync-version.mjs"]);
rodar(process.execPath, ["scripts/build-engine.mjs", "--release"]);
rodar(process.execPath, [resolve("node_modules/@tauri-apps/cli/tauri.js"), "build", "--bundles", "app", "--config", configLocal]);

console.log(`\nPronto: target/release/bundle/macos/${identidade.nome}.app`);
console.log(`Porta ${identidade.porta}, dados em ~/Library/Application Support/${identidade.pasta}, logs em ~/Library/Logs/${identidade.pasta}/`);
