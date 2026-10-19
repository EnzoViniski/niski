import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "tailwindcss";
import autoprefixer from "autoprefixer";
import identidade from "./identidade.json";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  // `<title>` do index.html vem de `identidade.json` (nome do produto num lugar só).
  plugins: [react(), { name: "titulo-da-identidade", transformIndexHtml: (html) => html.replace("<title>OMNI AGENTS</title>", `<title>${identidade.nome}</title>`) }],
  // "@" → src/. Existe porque o shadcn/React Bits gera imports com esse alias
  // (ver components.json). Espelhado em tsconfig.json > paths.
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  css: {
    postcss: {
      plugins: [tailwindcss, autoprefixer],
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    // Cargo workspace artifacts live in the root target directory.
    // Watching build executables can cause EBUSY on Windows.
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    // safari14 (o macOS 11 mínimo já vem com ele): com safari13 o esbuild reescreve o `r ||= {}` do
    // requestMode do xterm 6 e some com a declaração de `r` → ReferenceError quando a CLI consulta um
    // modo do terminal (DECRQM; o Antigravity faz isso ao abrir) e a resposta nunca chega.
    target:
      process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari14",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
  },
});
