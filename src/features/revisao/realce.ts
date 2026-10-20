// Monaco já pertence ao projeto. Só carrega quando há um trecho visível.
import "../files/monacoSetup";
import { editor } from "monaco-editor";

const linguagens: Record<string, string> = {
  ts: "typescript", tsx: "typescript", js: "javascript", jsx: "javascript", rs: "rust",
  py: "python", json: "json", css: "css", html: "html", md: "markdown", sh: "shell",
  toml: "ini", yaml: "yaml", yml: "yaml", sql: "sql",
};

export interface TokenRealce { texto: string; tipo: string }

/** Tokens viram texto React. As cores usam o tema do app, sem trocar o tema global do editor. */
export async function realcar(caminho: string, linhas: string[]): Promise<TokenRealce[][]> {
  const linguagem = linguagens[caminho.split(".").pop()?.toLowerCase() ?? ""] ?? "plaintext";
  const texto = linhas.join("\n");
  // colorize aguarda o carregamento do tokenizer da linguagem.
  await editor.colorize(texto, linguagem, { tabSize: 4 });
  return editor.tokenize(texto, linguagem).map((tokens, linha) => tokens.map((token, i) => ({
    texto: (linhas[linha] ?? "").slice(token.offset, tokens[i + 1]?.offset),
    tipo: /^(keyword|string|comment|number)/.exec(token.type)?.[1] ?? "texto",
  })));
}
