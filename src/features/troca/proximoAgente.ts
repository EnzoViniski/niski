/**
 * Fork (troca-de-contexto): o atalho "Ir pro agente pedindo atenção" (Ctrl+Tab) passa a andar em roda
 * pelos agentes que precisam de você (antes ia sempre ao primeiro, e com duas aprovações pendentes
 * não chegava na segunda) e abre o cartão "onde parei" de quem ele mostrar.
 */
import type { AttentionItem } from "../terminal/useAttention";
import { pedirOndeParei } from "./painelDaTroca";

let ultimo: string | null = null;

/** O próximo da lista (já em ordem de prioridade) depois do último visitado por este atalho. */
export function proximoDaRoda<T extends Pick<AttentionItem, "conversationKey">>(lista: T[], anterior: string | null): T | null {
  if (lista.length === 0) return null;
  const onde = anterior === null ? -1 : lista.findIndex((item) => item.conversationKey === anterior);
  return lista[(onde + 1) % lista.length];
}

/** Escolhe o próximo, lembra dele e pede o cartão aberto. `null`: ninguém precisa de você. */
export function irParaOProximo(lista: AttentionItem[]): AttentionItem | null {
  const proximo = proximoDaRoda(lista, ultimo);
  if (!proximo) return null;
  ultimo = proximo.conversationKey;
  pedirOndeParei(proximo.conversationKey);
  return proximo;
}

/** Só para testes. */
export function esquecerRoda() { ultimo = null; }
