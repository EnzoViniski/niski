// `.app` local para testar no macOS **ao lado** do app instalado, aberto pelo Finder.
//
// O app aberto pelo Finder não recebe variáveis de ambiente, então porta e pasta de dados vão
// gravadas na compilação (`OMNI_PORTA_DO_ENGINE`, `OMNI_PASTA_DE_DADOS`, lidas pelo omni-protocol).
// O bundle id também muda: é ele que separa o armazenamento da webview (localStorage) e o nome da
// pasta de logs. Sem updater: o build local não pode se trocar pelo release do autor.
//
// Uso: node scripts/build-mac-local.mjs   (só a arquitetura da máquina, só o .app)
//
// Variante de TESTE DO UPDATER (só com MEDZA_TESTE_UPDATER_ENDPOINT definida): bundle id, pasta de dados,
// nome e porta próprios (`<bundleId>.<sufixo>`, porta + 70), updater LIGADO apontando para um endpoint
// local (http://127.0.0.1:<porta>/latest.json), com a pubkey real e artefatos assinados. Serve para
// testar 0.8.1 → 0.8.2 de ponta a ponta sem tocar no app instalado. Variáveis:
//   MEDZA_TESTE_UPDATER_ENDPOINT   ex.: http://127.0.0.1:8765/latest.json  (liga a variante)
//   MEDZA_TESTE_SUFIXO             padrão "teste"
//   TAURI_SIGNING_PRIVATE_KEY (ou _PATH) e TAURI_SIGNING_PRIVATE_KEY_PASSWORD para assinar
// Os artefatos servíveis saem em target-tauri/teste-updater/<versão>/ (tar.gz, .sig e latest.json).
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { gravarSobreposicao, lerIdentidade, RAIZ, sobreposicaoDoTauri } from "./identidade.mjs";
import { montarLatestJson, nomeDeAsset } from "./make-latest-json.mjs";

if (process.platform !== "darwin") {
  console.error("build-mac-local: só no macOS.");
  process.exit(1);
}

// Derivado de `identidade.json`: mesmo produto, com sufixo " Local", bundle id `.local` e porta
// do engine + 60 (ex.: 47321 → 47381), para nunca encostar no app instalado.
const base = lerIdentidade();
const endpointDeTeste = process.env.MEDZA_TESTE_UPDATER_ENDPOINT;
const sufixo = process.env.MEDZA_TESTE_SUFIXO ?? "teste";
if (endpointDeTeste) {
  if (!base.updater?.pubkey) {
    console.error("build-mac-local: variante de teste do updater precisa de updater.pubkey em identidade.json.");
    process.exit(1);
  }
  if (!/^[a-z0-9]+$/i.test(sufixo)) {
    console.error(`build-mac-local: MEDZA_TESTE_SUFIXO "${sufixo}" inválido (só letras e números).`);
    process.exit(1);
  }
  if (new URL(endpointDeTeste).hostname !== "127.0.0.1") {
    console.error("build-mac-local: o endpoint de teste tem que ser local (127.0.0.1).");
    process.exit(1);
  }
}
const identidade = endpointDeTeste
  ? {
      porta: process.env.OMNI_PORTA_DO_ENGINE ?? String(base.portaDoEngine + 70),
      pasta: process.env.OMNI_PASTA_DE_DADOS ?? `${base.bundleId}.${sufixo.toLowerCase()}`,
      nome: process.env.OMNI_NOME_LOCAL ?? `${base.nome} ${sufixo[0].toUpperCase()}${sufixo.slice(1).toLowerCase()}`,
    }
  : {
      porta: process.env.OMNI_PORTA_DO_ENGINE ?? String(base.portaDoEngine + 60),
      pasta: process.env.OMNI_PASTA_DE_DADOS ?? `${base.bundleId}.local`,
      nome: process.env.OMNI_NOME_LOCAL ?? `${base.nome} Local`,
    };

const configLocal = gravarSobreposicao(sobreposicaoDoTauri(base, endpointDeTeste
  ? {
      nome: identidade.nome,
      bundleId: identidade.pasta,
      bundle: { createUpdaterArtifacts: true },
      // O Tauri exige TLS no updater; http só com este sinalizador (plugins.updater, doc oficial v2).
      updater: { endpoints: [endpointDeTeste], pubkey: base.updater.pubkey, dangerousInsecureTransportProtocol: true },
    }
  : {
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
if (endpointDeTeste) {
  // Assinatura: o Tauri aceita caminho ou conteúdo em TAURI_SIGNING_PRIVATE_KEY. Aqui só se repassa
  // CAMINHO; o conteúdo da chave nunca é lido nem impresso por este script.
  const chave = env.TAURI_SIGNING_PRIVATE_KEY ?? env.TAURI_SIGNING_PRIVATE_KEY_PATH ?? join(homedir(), ".tauri", "medza-agents.key");
  if (!env.TAURI_SIGNING_PRIVATE_KEY) {
    if (!existsSync(chave)) {
      console.error("build-mac-local: defina TAURI_SIGNING_PRIVATE_KEY (ou TAURI_SIGNING_PRIVATE_KEY_PATH) para assinar a variante de teste.");
      process.exit(1);
    }
    env.TAURI_SIGNING_PRIVATE_KEY = chave;
  }
  env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= "";
}
const inicio = Date.now();
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

if (endpointDeTeste) {
  // Separa o par assinado desta versão numa pasta pronta para servir como endpoint local.
  const versao = readFileSync(join(RAIZ, "VERSION"), "utf8").trim();
  const bundle = resolve("target/release/bundle/macos");
  const tarGz = join(bundle, `${identidade.nome}.app.tar.gz`);
  if (!existsSync(`${tarGz}.sig`) || statSync(tarGz).mtimeMs < inicio) {
    console.error(`\nbuild-mac-local: o build não gerou ${tarGz}(.sig) novo. Confira a assinatura (createUpdaterArtifacts + chave).`);
    process.exit(1);
  }
  const arquitetura = process.arch === "arm64" ? "aarch64" : "x64";
  const arquivo = `${nomeDeAsset(identidade.nome)}_${versao}_${arquitetura}.app.tar.gz`;
  const saida = join(RAIZ, "target-tauri", "teste-updater", versao);
  mkdirSync(saida, { recursive: true });
  copyFileSync(tarGz, join(saida, arquivo));
  copyFileSync(`${tarGz}.sig`, join(saida, `${arquivo}.sig`));
  const endpoint = new URL(endpointDeTeste);
  const { json } = montarLatestJson({
    arquivos: [arquivo, `${arquivo}.sig`],
    versao,
    nome: identidade.nome,
    repositorio: base.repositorioDeReleases,
    notas: `Versão ${versao} (teste local).`,
    urlBase: new URL(".", endpoint).href,
    lerAssinatura: (sig) => readFileSync(join(saida, sig), "utf8"),
  });
  writeFileSync(join(saida, "latest.json"), JSON.stringify(json, null, 2) + "\n", "utf8");
  console.log(`\nVariante de teste do updater (${versao}): ${saida}`);
  console.log(`Para oferecer esta versão: cd ${saida} && python3 -m http.server ${endpoint.port || 80} --bind 127.0.0.1`);
}

console.log(`\nPronto: target/release/bundle/macos/${identidade.nome}.app`);
console.log(`Porta ${identidade.porta}, dados em ~/Library/Application Support/${identidade.pasta}, logs em ~/Library/Logs/${identidade.pasta}/`);
