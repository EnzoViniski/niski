import * as monaco from "monaco-editor";
import { loader } from "@monaco-editor/react";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import JsonWorker from "monaco-editor/language/json/json.worker.js?worker";
import CssWorker from "monaco-editor/language/css/css.worker.js?worker";
import HtmlWorker from "monaco-editor/language/html/html.worker.js?worker";
import TypeScriptWorker from "monaco-editor/language/typescript/ts.worker.js?worker";

// App local-only: nunca buscar o Monaco de CDN (padrão do @monaco-editor/react).
// Aponta pro pacote já bundlado no build do Vite.
loader.config({ monaco });

// Fork: o Vite não resolve um pacote npm em `new URL("monaco-editor/…", import.meta.url)`.
// `?worker` cria bundles locais dos workers, incluindo suas dependências, no build de produção.
// Os exports do Monaco 0.56 já apontam para `esm/vs`: repetir esse prefixo duplicaria o caminho.

self.MonacoEnvironment = {
  getWorker(_workerId: string, label: string) {
    if (label === "json") return new JsonWorker();
    if (["css", "scss", "less"].includes(label)) return new CssWorker();
    if (["html", "handlebars", "razor"].includes(label)) return new HtmlWorker();
    if (["typescript", "javascript"].includes(label)) return new TypeScriptWorker();
    return new EditorWorker();
  },
};
