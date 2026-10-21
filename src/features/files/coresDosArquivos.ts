/**
 * Fork (0.9.4): cor do ícone por tipo de arquivo, como no VS Code com Material Icon Theme.
 *
 * As cores seguem a identidade de cada linguagem/ferramenta (TypeScript azul, React ciano, Rust
 * ferrugem, Python azul...), clareadas para ler bem no tema escuro. No tema claro o CSS
 * (`coresDosArquivos.css`) escurece a mesma cor para manter o contraste. Tipo sem cor conhecida fica
 * com a cor do texto, como antes.
 *
 * O desenho continua vindo de `fileIcons.ts`; aqui é só a tinta.
 */
import type { CSSProperties } from "react";
import "./coresDosArquivos.css";

const AZUL_TS = "#4a9eff";
const AMARELO_JS = "#e8d44d";
const CIANO_REACT = "#61dafb";
const AMBAR_DADOS = "#e2b93d";
const ROSA_ESTILO = "#e06caa";
const LARANJA_HTML = "#ef7b4d";
const AZUL_DOC = "#6cb6e8";
const VERDE_SHELL = "#89d185";
const ROXO_IMAGEM = "#b88cf0";
const VERMELHO_VIDEO = "#f0716c";
const MAGENTA_AUDIO = "#d77ee6";
const MARROM_PACOTE = "#c9a26b";
const CINZA_CONFIG = "#9aa4b2";
const CINZA_TRAVA = "#8b93a1";
const AMARELO_CHAVE = "#f2cc60";

const PASTAS: Record<string, string> = {
  node_modules: "#8bc34a",
  ".git": "#f1653f",
  ".github": "#c3c7cc",
  src: "#5fb878",
  dist: MARROM_PACOTE,
  build: MARROM_PACOTE,
  out: MARROM_PACOTE,
  target: MARROM_PACOTE,
  public: "#4fc1c9",
  assets: ROXO_IMAGEM,
  images: ROXO_IMAGEM,
  img: ROXO_IMAGEM,
  docs: AZUL_DOC,
  doc: AZUL_DOC,
  test: "#a3d65c",
  tests: "#a3d65c",
  __tests__: "#a3d65c",
  spec: "#a3d65c",
  scripts: VERDE_SHELL,
  config: CINZA_CONFIG,
  ".vscode": "#3ea6ff",
  ".idea": "#f15ba6",
  bin: CINZA_CONFIG,
  lib: "#7aa5f7",
  components: CIANO_REACT,
  hooks: "#9b87f5",
  types: AZUL_TS,
  styles: ROSA_ESTILO,
  css: ROSA_ESTILO,
  fonts: "#e8a87c",
  media: VERMELHO_VIDEO,
  videos: VERMELHO_VIDEO,
  audio: MAGENTA_AUDIO,
  cache: CINZA_TRAVA,
  tmp: CINZA_TRAVA,
  temp: CINZA_TRAVA,
  ".cache": CINZA_TRAVA,
  vendor: MARROM_PACOTE,
  packages: MARROM_PACOTE,
  crates: "#e0855a",
  server: "#5fb3a6",
  api: "#5aa9e6",
  db: AMBAR_DADOS,
  database: AMBAR_DADOS,
  migrations: AMBAR_DADOS,
  locale: "#4fc1c9",
  locales: "#4fc1c9",
  i18n: "#4fc1c9",
};
/** Pasta comum: o amarelo-manila de explorador de arquivos. */
const PASTA = "#dcb362";

const NOMES: Record<string, string> = {
  "package.json": "#e05d5d",
  "package-lock.json": "#e05d5d",
  ".npmrc": "#e05d5d",
  "pnpm-lock.yaml": "#f6b73c",
  "pnpm-workspace.yaml": "#f6b73c",
  "yarn.lock": "#3f9fd4",
  "deno.json": "#c3c7cc",
  "deno.jsonc": "#c3c7cc",
  dockerfile: "#3b9ff3",
  "docker-compose.yml": "#3b9ff3",
  "docker-compose.yaml": "#3b9ff3",
  ".dockerignore": "#3b9ff3",
  makefile: "#e8875a",
  "cmakelists.txt": "#6aa8e8",
  ".gitignore": "#f1653f",
  ".gitattributes": "#f1653f",
  ".gitmodules": "#f1653f",
  license: "#e8c547",
  "license.md": "#e8c547",
  "license.txt": "#e8c547",
  ".env": AMARELO_CHAVE,
  ".env.local": AMARELO_CHAVE,
  ".env.example": AMARELO_CHAVE,
  "readme.md": "#4fb7e8",
  "changelog.md": "#8fd16a",
  "contributing.md": "#c792ea",
  "claude.md": "#e5976a",
  "agents.md": "#74c7a5",
  "cargo.toml": "#e0855a",
  "cargo.lock": CINZA_TRAVA,
  "tsconfig.json": AZUL_TS,
  "tsconfig.node.json": AZUL_TS,
  "vite.config.ts": "#a77bf3",
  "vite.config.js": "#a77bf3",
  "tailwind.config.ts": "#38bdf8",
  "tailwind.config.js": "#38bdf8",
  "postcss.config.ts": "#e0574a",
  "vercel.json": "#e6e6e6",
  ".eslintrc": "#8080f2",
  ".eslintrc.json": "#8080f2",
  ".prettierrc": "#56b3b4",
  ".editorconfig": CINZA_CONFIG,
};

const EXTENSOES: Record<string, string> = {
  html: LARANJA_HTML, htm: LARANJA_HTML, xml: "#f0a35c", svg: "#ffb547",
  vue: "#41b883", svelte: "#ff5a2b", astro: "#ff7e33",
  css: "#4c9be8", scss: ROSA_ESTILO, sass: ROSA_ESTILO, less: "#5b8fd6",
  ts: AZUL_TS, mts: AZUL_TS, cts: AZUL_TS,
  tsx: CIANO_REACT, jsx: CIANO_REACT,
  js: AMARELO_JS, mjs: AMARELO_JS, cjs: AMARELO_JS,
  json: AMBAR_DADOS, jsonc: AMBAR_DADOS, json5: AMBAR_DADOS,
  yml: "#e06b74", yaml: "#e06b74",
  toml: "#c49a6c", ini: CINZA_CONFIG, cfg: CINZA_CONFIG, conf: CINZA_CONFIG, properties: CINZA_CONFIG,
  lock: CINZA_TRAVA, env: AMARELO_CHAVE,
  csv: "#3cb878", tsv: "#3cb878",
  sql: "#f29e4c", db: "#f29e4c", sqlite: "#5fa0d8",
  graphql: "#e535ab", gql: "#e535ab", proto: "#6cc4a1",
  rs: "#e0855a", go: "#29beb0", c: "#7aa5d8", h: "#9b8fe0", cpp: "#f06292", cc: "#f06292", hpp: "#c38ee0",
  cs: "#68b04d", java: "#e8a03e", kt: "#a97bff", kts: "#a97bff", swift: "#f5743b", m: "#6aa8e8",
  dart: "#40c4ff", zig: "#f7a41d",
  py: "#4b9ce2", rb: "#e5534b", php: "#8892bf", lua: "#5868d6", pl: "#4f8fbf", r: "#2f8fdf",
  sh: VERDE_SHELL, bash: VERDE_SHELL, zsh: VERDE_SHELL, fish: VERDE_SHELL,
  ps1: "#5391fe", bat: "#c1f12e", cmd: "#c1f12e",
  md: AZUL_DOC, markdown: AZUL_DOC, mdx: "#f9ac00",
  txt: "#b8bfc9", rtf: "#b8bfc9", pdf: "#f05a4f",
  doc: "#4f8de8", docx: "#4f8de8", odt: "#4f8de8",
  xls: "#33b06b", xlsx: "#33b06b", ods: "#33b06b",
  ppt: "#ef7744", pptx: "#ef7744", odp: "#ef7744",
  log: "#a5b0bd",
  png: ROXO_IMAGEM, jpg: ROXO_IMAGEM, jpeg: ROXO_IMAGEM, gif: ROXO_IMAGEM, webp: ROXO_IMAGEM,
  avif: ROXO_IMAGEM, bmp: ROXO_IMAGEM, tiff: ROXO_IMAGEM, ico: "#f7c948",
  psd: "#31a8ff", xcf: "#a39c88", ai: "#ff9a00", fig: "#a259ff",
  raw: ROXO_IMAGEM, cr2: ROXO_IMAGEM, nef: ROXO_IMAGEM,
  mp4: VERMELHO_VIDEO, mkv: VERMELHO_VIDEO, mov: VERMELHO_VIDEO, avi: VERMELHO_VIDEO,
  webm: VERMELHO_VIDEO, wmv: VERMELHO_VIDEO, flv: VERMELHO_VIDEO,
  mp3: MAGENTA_AUDIO, wav: MAGENTA_AUDIO, flac: MAGENTA_AUDIO, ogg: MAGENTA_AUDIO,
  m4a: MAGENTA_AUDIO, aac: MAGENTA_AUDIO, opus: MAGENTA_AUDIO, mid: MAGENTA_AUDIO,
  zip: MARROM_PACOTE, tar: MARROM_PACOTE, gz: MARROM_PACOTE, bz2: MARROM_PACOTE,
  xz: MARROM_PACOTE, "7z": MARROM_PACOTE, rar: MARROM_PACOTE,
  exe: "#6cb6e8", msi: "#6cb6e8", dll: CINZA_CONFIG, so: CINZA_CONFIG, dylib: CINZA_CONFIG,
  app: "#c3c7cc", dmg: "#c3c7cc", deb: "#e95420", rpm: "#e05d5d", appimage: "#6cb6e8",
  apk: "#3ddc84", wasm: "#8a7cf0", bin: CINZA_CONFIG, iso: CINZA_CONFIG,
  ttf: "#e8a87c", otf: "#e8a87c", woff: "#e8a87c", woff2: "#e8a87c",
  eml: "#5aa9e6", ics: "#e05d5d", url: "#5aa9e6", torrent: "#5fb878",
  pem: AMARELO_CHAVE, key: AMARELO_CHAVE, crt: "#5fb878", sig: "#5fb878",
  bak: CINZA_TRAVA, tmp: CINZA_TRAVA,
};

function extensaoDe(nome: string): string {
  const ponto = nome.lastIndexOf(".");
  return ponto <= 0 ? "" : nome.slice(ponto + 1);
}

/** Cor do ícone de um item (mesma precedência de `iconForEntry`: pasta, nome exato, extensão). */
export function corDoArquivo(nome: string, pasta: boolean): string | undefined {
  const limpo = nome.toLowerCase();
  if (pasta) return PASTAS[limpo] ?? PASTA;
  return NOMES[limpo] ?? EXTENSOES[extensaoDe(limpo)];
}

/** `className` + `style` para o elemento que envolve o ícone. Sem cor conhecida: nada muda. */
export function tintaDoArquivo(nome: string, pasta = false): { className: string; style?: CSSProperties } {
  const cor = corDoArquivo(nome, pasta);
  if (!cor) return { className: "" };
  return { className: "icone-colorido", style: { "--cor-arquivo": cor } as CSSProperties };
}

/** A partir de um caminho completo (abas, painel do Git). */
export function tintaDoCaminho(caminho: string, pasta = false) {
  return tintaDoArquivo(caminho.split(/[\\/]/).filter(Boolean).pop() ?? caminho, pasta);
}

/** Só para os testes. */
export const MAPAS_DE_COR = { PASTAS, NOMES, EXTENSOES };
