// Fork (Editor de vídeo): endereços do protocolo `medzavideo://` (ver `src-tauri/src/videos/protocolo.rs`).
// O token muda a cada execução do app; é lido uma vez e guardado.
import { invoke } from "@tauri-apps/api/core";

let token: Promise<string> | null = null;
let tokenPronto: string | null = null;

export function carregarEndereco() {
  token ??= invoke<string>("videos_endereco_base").then((t) => { tokenPronto = t; return t; }).catch((e) => { token = null; throw e; });
  return token;
}

/** O Windows serve esquemas próprios como `http://<esquema>.localhost` (mesma regra do `asset:`). */
const NO_WINDOWS = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);

function base(t: string) {
  return NO_WINDOWS ? `http://medzavideo.localhost/${t}` : `medzavideo://localhost/${t}`;
}

function caminho(rel: string) {
  return rel.split("/").map(encodeURIComponent).join("/");
}

/** Arquivo de um projeto. `null` enquanto o token não carregou (chame `carregarEndereco` antes). */
export function urlDoProjeto(projeto: string, rel = "index.html", versao?: number) {
  if (!tokenPronto) return null;
  return `${base(tokenPronto)}/p/${projeto}/${caminho(rel)}${versao !== undefined ? `?v=${versao}` : ""}`;
}

/** Arquivo de uma pasta de referências (`arquivos/<nome>`). */
export function urlDaPasta(pasta: string, rel: string) {
  if (!tokenPronto) return null;
  return `${base(tokenPronto)}/r/${pasta}/${caminho(rel)}`;
}

/** Só para os testes. */
export function definirTokenParaTeste(t: string | null) { tokenPronto = t; token = t ? Promise.resolve(t) : null; }
