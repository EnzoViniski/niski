import { useCallback, useState } from "react";

/**
 * A ordem que a pessoa escolheu, arrastando, para os projetos da lateral e para as conversas
 * de cada projeto. Sem ordem salva vale a natural (projetos na ordem em que foram adicionados,
 * conversas da mais antiga para a mais nova); item novo entra no fim.
 */
const CHAVE = "omni-ordem-lateral";

export interface OrdemDaLateral {
  projetos: string[];
  /** Por projeto: chaves das conversas (`conversationKey`). */
  conversas: Record<string, string[]>;
}

function ler(): OrdemDaLateral {
  try {
    const salvo = JSON.parse(localStorage.getItem(CHAVE) ?? "null");
    const projetos = Array.isArray(salvo?.projetos) ? salvo.projetos.filter((id: unknown): id is string => typeof id === "string") : [];
    const conversas: Record<string, string[]> = {};
    if (salvo?.conversas && typeof salvo.conversas === "object") {
      for (const [projeto, lista] of Object.entries(salvo.conversas)) {
        if (Array.isArray(lista)) conversas[projeto] = lista.filter((id): id is string => typeof id === "string");
      }
    }
    return { projetos, conversas };
  } catch {
    return { projetos: [], conversas: {} };
  }
}

/** Os itens na ordem salva; os que ela não conhece vão para o fim, na ordem em que vieram. */
export function ordenar<T>(itens: T[], chave: (item: T) => string, ordem: string[]): T[] {
  const posicao = new Map(ordem.map((id, i) => [id, i]));
  return itens
    .map((item, natural) => ({ item, natural, salvo: posicao.get(chave(item)) }))
    .sort((a, b) => (a.salvo ?? Infinity) - (b.salvo ?? Infinity) || a.natural - b.natural)
    .map(({ item }) => item);
}

/** Tira `id` de onde está e o põe antes da posição `antesDe` da lista original (`length` = no fim). */
export function mover(ids: string[], id: string, antesDe: number): string[] {
  const de = ids.indexOf(id);
  if (de < 0) return ids;
  const resto = ids.filter((outro) => outro !== id);
  const para = Math.max(0, Math.min(resto.length, antesDe > de ? antesDe - 1 : antesDe));
  return [...resto.slice(0, para), id, ...resto.slice(para)];
}

/** Tira `id` da lista e o põe na posição `posicao` entre as outras (a prévia do arraste). */
export function colocar(ids: string[], id: string, posicao: number): string[] {
  if (!ids.includes(id)) return ids;
  const resto = ids.filter((outro) => outro !== id);
  const para = Math.max(0, Math.min(resto.length, posicao));
  return [...resto.slice(0, para), id, ...resto.slice(para)];
}

export function useOrdemDaLateral() {
  const [ordem, setOrdem] = useState<OrdemDaLateral>(ler);
  const salvar = useCallback((nova: OrdemDaLateral) => {
    setOrdem(nova);
    try { localStorage.setItem(CHAVE, JSON.stringify(nova)); } catch { /* sem armazenamento: vale só agora */ }
  }, []);
  const ordenarProjetos = useCallback((ids: string[]) => salvar({ ...ordem, projetos: ids }), [ordem, salvar]);
  const ordenarConversas = useCallback((projeto: string, ids: string[]) =>
    salvar({ ...ordem, conversas: { ...ordem.conversas, [projeto]: ids } }), [ordem, salvar]);
  return { ordem, ordenarProjetos, ordenarConversas };
}
