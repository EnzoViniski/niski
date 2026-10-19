import { createContext, useContext, useState, type ReactNode } from "react";

/**
 * Fork: caminho que o agente escreve entre crases (`~/omni/PROMPT.md`, `src/App.tsx:12`) vira link na
 * Conversa do PC: clicar abre a pasta no Finder ou o arquivo no app padrão (comando
 * `abrir_caminho_da_conversa`, que nunca executa script nem `.app` — esses só aparecem selecionados).
 * No celular continua texto: lá não há como abrir arquivo do PC.
 */
const NO_PC = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Pasta onde a conversa roda: caminho relativo é relativo a ela. */
export const PastaDaConversaContext = createContext<string | null>(null);

/** Parece caminho de arquivo/pasta: começa com `/`, `~/`, `./` ou tem uma `/` e nenhum espaço. */
export function pareceCaminho(texto: string): boolean {
  const t = texto.trim();
  if (!t || t.length > 400 || /\s/.test(t) || /^[a-z][\w+.-]*:\/\//i.test(t)) return false;
  if (/^(~\/|\/|\.{1,2}\/)/.test(t)) return t.length > 1;
  // Relativo: precisa de uma barra entre nomes (`src/App.tsx`, `OMNI-AGENTS/docs`), sem ser comando.
  return /^[\w@.-]+(\/[\w@.+-]+)+\/?(:\d+(:\d+)?)?$/.test(t);
}

async function abrir(caminho: string, cwd: string | null) {
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("abrir_caminho_da_conversa", { caminho, cwd });
}

/** `code` do markdown: caminho no PC vira botão; o resto, código como sempre. */
export function CodigoOuCaminho({ className, children }: { className?: string; children?: ReactNode }) {
  const cwd = useContext(PastaDaConversaContext);
  const [erro, setErro] = useState<string | null>(null);
  const texto = typeof children === "string" ? children : Array.isArray(children) && children.every((c) => typeof c === "string") ? children.join("") : null;
  if (!NO_PC || className || texto === null || !pareceCaminho(texto)) return <code className={className}>{children}</code>;
  return (
    <button type="button" className="caminho-link" title={erro ?? "Abrir no Finder / app padrão"}
      onClick={() => { setErro(null); abrir(texto, cwd).catch((motivo) => setErro(String(motivo))); }}>
      <code>{texto}</code>
      {erro && <span className="caminho-erro"> ({erro})</span>}
    </button>
  );
}
