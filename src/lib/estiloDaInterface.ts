/**
 * Qual estilo de interface vale no <html> (PC, web e celular usam a mesma regra).
 *
 * O "Vidro" (pedido do Enzo, 21/10/2026) é o Minimalista com fundo preto e superfícies de vidro, no
 * espírito do Liquid Glass da Apple e do app do Codex no celular. Por isso ele liga as duas coisas:
 * `data-estilo="minimalista"` (todas as regras de forma do `minimalista.css` continuam valendo) e
 * `data-variante="vidro"` (as cores, o vidro e o movimento do `vidro.css`, carregado depois).
 */
import type { EstiloDaInterface } from "../types/settings";

/** Valor desconhecido (de uma versão mais nova) cai no Clássico, como antes. */
export function estiloValido(valor: unknown): EstiloDaInterface {
  return valor === "minimalista" || valor === "vidro" ? valor : "classico";
}

/** Minimalista e Vidro: cantos, fonte do sistema e fundo liso (sem o fundo animado do Clássico). */
export function usaFormaMinimalista(valor: unknown): boolean {
  return estiloValido(valor) !== "classico";
}

export function aplicarEstilo(root: HTMLElement, valor: unknown) {
  const estilo = estiloValido(valor);
  root.dataset.estilo = estilo === "classico" ? "classico" : "minimalista";
  if (estilo === "vidro") root.dataset.variante = "vidro";
  else delete root.dataset.variante;
}
