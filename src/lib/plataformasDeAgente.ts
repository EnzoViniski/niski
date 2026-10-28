/** Nome e cor de cada plataforma de agente num lugar só. Antes eram oito mapas espalhados
 *  (celular, web, chat, uso das contas, skills, fluxo do orquestrador, cores), e metade deles sem o
 *  Antigravity ou sem o Cursor — o agente aparecia com o id cru ("antigravity") na tela. */

export type PlataformaDeAgente = "claude" | "codex" | "antigravity" | "cursor";

export interface InfoDaPlataforma {
  /** Nome curto, o das listas e do autor da mensagem ("Claude"). */
  nome: string;
  /** Nome do produto, onde há espaço ("Claude Code"). */
  nomeLongo: string;
  /** Cor na lista de conversas: a da marca quando ela é colorida (Claude, Antigravity), e uma cor
   *  própria e distinta quando a marca é preta/branca (Codex, Cursor). */
  cor: string;
}

export const PLATAFORMAS: Readonly<Record<PlataformaDeAgente, InfoDaPlataforma>> = {
  claude: { nome: "Claude", nomeLongo: "Claude Code", cor: "#d97757" },
  codex: { nome: "Codex", nomeLongo: "Codex", cor: "#a78bfa" },
  antigravity: { nome: "Antigravity", nomeLongo: "Antigravity", cor: "#4285f4" },
  cursor: { nome: "Cursor", nomeLongo: "Cursor", cor: "#a1a1aa" },
};

/** A plataforma pelo id que o engine manda (`provider`). `undefined` para id desconhecido: cada
 *  tela decide o que mostrar no lugar (o próprio id, "Agente", "Terminal"). */
export function plataforma(id: string | null | undefined): InfoDaPlataforma | undefined {
  return id && Object.prototype.hasOwnProperty.call(PLATAFORMAS, id) ? PLATAFORMAS[id as PlataformaDeAgente] : undefined;
}

export function nomeDaPlataforma(id: string | null | undefined): string | undefined {
  return plataforma(id)?.nome;
}

export function nomeLongoDaPlataforma(id: string | null | undefined): string | undefined {
  return plataforma(id)?.nomeLongo;
}
