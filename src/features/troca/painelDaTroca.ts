/**
 * Fork (troca-de-contexto): o que a lateral e o atalho sabem de cada conversa na tela, sem ler nada
 * a mais do engine.
 *
 * Cada conversa aberta já lê a própria timeline (com ETag) e monta o cartão "onde parei". Ela publica
 * aqui só o essencial (tom, uma linha, quantas novidades), e a lista "Na tela" lê daqui. Conversa que
 * não está montada (outro projeto, aba no modo Terminal) não publica: a lateral cai no estado da sessão.
 *
 * Também guarda o pedido "abra o cartão desta conversa" do atalho, que chega antes de o painel montar.
 */
import { useSyncExternalStore } from "react";
import type { Tom } from "./ondeParei";

export interface ResumoNaTela {
  tom: Tom;
  /** "Aprovar: Bash · npm test", "Terminou: …" (ver `linhaEssencial`). */
  linha: string;
  /** Mensagens depois da sua última olhada (0 enquanto você olha). */
  novas: number;
}

const resumos = new Map<string, ResumoNaTela>();
const ouvintes = new Set<() => void>();
let versao = 0;

function avisar() {
  versao += 1;
  ouvintes.forEach((ouvinte) => ouvinte());
}

/** Publica o resumo de uma conversa. Igual ao anterior: não avisa ninguém (a lateral não redesenha). */
export function publicarResumo(conversaId: string, resumo: ResumoNaTela | null) {
  const anterior = resumos.get(conversaId);
  if (!resumo) {
    if (anterior && resumos.delete(conversaId)) avisar();
    return;
  }
  if (anterior && anterior.tom === resumo.tom && anterior.linha === resumo.linha && anterior.novas === resumo.novas) return;
  resumos.set(conversaId, resumo);
  avisar();
}

export function resumoNaTela(conversaId: string | null | undefined): ResumoNaTela | null {
  return conversaId ? resumos.get(conversaId) ?? null : null;
}

function ouvir(ouvinte: () => void) {
  ouvintes.add(ouvinte);
  return () => { ouvintes.delete(ouvinte); };
}

/** O resumo publicado de uma conversa (redesenha só quando algum resumo muda). */
export function useResumoNaTela(conversaId: string | null | undefined): ResumoNaTela | null {
  useSyncExternalStore(ouvir, () => versao, () => versao);
  return resumoNaTela(conversaId);
}

/* ── Pedido de abrir o cartão (atalho "próximo agente que precisa de você") ─────────────────── */

export const EVENTO_ABRIR_ONDE_PAREI = "medza-abrir-onde-parei";
/** O painel tem este tempo para montar e atender o pedido. */
const VALIDADE_DO_PEDIDO_MS = 10_000;
const pedidos = new Map<string, number>();

/** Pede que o cartão da conversa abra (agora, se ela está na tela, ou quando montar). */
export function pedirOndeParei(conversaId: string, agora = Date.now()) {
  pedidos.set(conversaId, agora);
  window.dispatchEvent(new CustomEvent(EVENTO_ABRIR_ONDE_PAREI, { detail: conversaId }));
}

/** O pedido ainda vale? Consome (cada pedido abre o cartão uma vez). */
export function consumirPedido(conversaId: string, agora = Date.now()): boolean {
  const quando = pedidos.get(conversaId);
  pedidos.delete(conversaId);
  return quando !== undefined && agora - quando <= VALIDADE_DO_PEDIDO_MS;
}

/** Só para testes. */
export function limparPainelDaTroca() {
  resumos.clear();
  pedidos.clear();
  avisar();
}
