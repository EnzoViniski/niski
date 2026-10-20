#!/usr/bin/env node
// Build distribuível do Medza. Nenhuma pasta de dados, conta ou conversa entra no bundle.
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { lerIdentidade, gravarSobreposicao, sobreposicaoDoTauri } from "./identidade.mjs";

const args = process.argv.slice(2);
const valor = (flag) => {
  const i = args.indexOf(flag);
  if (i < 0) return undefined;
  if (!args[i + 1] || args[i + 1].startsWith("--")) throw new Error(`Falta valor para ${flag}`);
  return args[i + 1];
};
const target = valor("--target");
const bundles = valor("--bundles");
const id = lerIdentidade();
const config = gravarSobreposicao(sobreposicaoDoTauri(id));
// O build do Vite passa de 2 GB de heap; em máquina com 7-8 GB (runners do GitHub) o limite padrão
// do Node fica abaixo disso e o processo morre por falta de memória.
const opcoesDoNode = process.env.NODE_OPTIONS ?? "";
const env = opcoesDoNode.includes("--max-old-space-size") ? process.env
  : { ...process.env, NODE_OPTIONS: `${opcoesDoNode} --max-old-space-size=4096`.trim() };
const rodar = (cmd, parametros) => execFileSync(cmd, parametros, { stdio: "inherit", env });
rodar(process.execPath, ["scripts/sync-version.mjs"]);
rodar(process.execPath, ["scripts/build-engine.mjs", "--release", ...(target ? ["--target", target] : [])]);
rodar(process.execPath, [resolve("node_modules/@tauri-apps/cli/tauri.js"), "build", "--config", config,
  ...(target ? ["--target", target] : []), ...(bundles ? ["--bundles", bundles] : [])]);
rodar(process.execPath, ["scripts/collect-installers.mjs", ...(target ? ["--target", target] : [])]);
