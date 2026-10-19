/** Cor de cada agente na lista de conversas: a da marca quando ela é colorida (Claude, Antigravity),
 *  e uma cor própria e distinta quando a marca é preta/branca (Codex, Cursor). */
const CORES: Record<string, string> = {
  claude: "#d97757",
  codex: "#a78bfa",
  antigravity: "#4285f4",
  cursor: "#a1a1aa",
};

const NOMES: Record<string, string> = { claude: "Claude", codex: "Codex", antigravity: "Antigravity", cursor: "Cursor" };

export function corDoAgente(provider: string | null | undefined): string {
  return (provider && CORES[provider]) || "rgb(var(--c-accent))";
}

export function nomeDoAgente(provider: string | null | undefined): string {
  return (provider && NOMES[provider]) || provider || "Terminal";
}
