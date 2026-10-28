// `/` no campo da Conversa — a lista de comandos, skills e comandos personalizados que o engine
// acha para a conversa (`GET /conversas/<id>/barra`), o filtro e para onde vai o envio.
import { useCallback, useRef } from "react";
import { request } from "../api";
import type { ComandoDaConversa } from "./ControlesDaConversa";

export interface ItemDaBarra {
  nome: string;
  descricao: string;
  /** `cli`: comando da própria CLI que o chat roda; `skill`; `comando`: personalizado. */
  tipo: "cli" | "skill" | "comando";
  origem: "cli" | "conta" | "projeto";
  /** Texto que entra no campo ao escolher (`/compact`, `/revisar `, `$doc `…). */
  insere: string;
}

/** A `/busca` aberta: só no começo da mensagem e antes do primeiro espaço, como nas CLIs. */
export function barraNoCursor(texto: string, cursor: number): { busca: string } | null {
  const achado = /^\/([^\s/]*)$/.exec(texto.slice(0, cursor));
  return achado ? { busca: achado[1] } : null;
}

/** Começa com a busca primeiro, depois contém no nome, depois na descrição. */
export function filtrarItens(itens: ItemDaBarra[], busca: string, limite = 50): ItemDaBarra[] {
  const b = busca.toLowerCase();
  if (!b) return itens.slice(0, limite);
  const comeca: ItemDaBarra[] = [], noNome: ItemDaBarra[] = [], naDescricao: ItemDaBarra[] = [];
  for (const item of itens) {
    const nome = item.nome.toLowerCase();
    if (nome.startsWith(b)) comeca.push(item);
    else if (nome.includes(b)) noNome.push(item);
    else if (item.descricao.toLowerCase().includes(b)) naDescricao.push(item);
  }
  return [...comeca, ...noNome, ...naDescricao].slice(0, limite);
}

export type EnvioDaBarra =
  | { tipo: "oficial"; comando: ComandoDaConversa; argumento?: string }
  | { tipo: "barra"; texto: string };

const OFICIAIS: Record<string, ComandoDaConversa> = { compact: "compact", rewind: "rewind", branch: "branch", fast: "fast" };

/** Mensagem começando com `/`: comando da lista fechada vai por `/comando`; o resto (skill ou
 *  comando personalizado do Claude), por `/barra`, que confere o nome. `null`: não é comando. */
export function interpretarEnvio(texto: string): EnvioDaBarra | null {
  const limpo = texto.trim();
  if (!limpo.startsWith("/")) return null;
  const [cabeca, ...resto] = limpo.slice(1).split(/\s+/);
  const argumento = resto.join(" ").trim();
  const oficial = OFICIAIS[cabeca];
  if (oficial) return argumento ? { tipo: "oficial", comando: oficial, argumento } : { tipo: "oficial", comando: oficial };
  return { tipo: "barra", texto: limpo };
}

const VALIDADE_MS = 30_000;

/** Lê a lista uma vez por conversa e guarda por 30 s: abrir o `/` de novo não refaz a varredura. */
export function useComandosDaBarra(conversaId: string) {
  const cache = useRef<{ id: string; em: number; itens: ItemDaBarra[] } | null>(null);
  return useCallback(async () => {
    const atual = cache.current;
    if (atual && atual.id === conversaId && Date.now() - atual.em < VALIDADE_MS) return atual.itens;
    const resposta = await request<{ itens?: ItemDaBarra[] }>(`/conversas/${encodeURIComponent(conversaId)}/barra`);
    const itens = Array.isArray(resposta?.itens) ? resposta.itens : [];
    cache.current = { id: conversaId, em: Date.now(), itens };
    return itens;
  }, [conversaId]);
}
