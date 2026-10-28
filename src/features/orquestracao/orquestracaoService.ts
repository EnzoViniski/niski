import { useEffect, useState } from "react";
import { request, type Conversation } from "../../mobile/api";
import { energiaPausada } from "../../lib/energia";

/**
 * Orquestrador de agentes. Uma conversa vira orquestradora pelo botão "Tornar orquestradora":
 * o engine reabre a CLI com um servidor MCP só daquela sessão (`crates/omni-engine/src/orquestracao.rs`)
 * e ela passa a criar e comandar subagentes. Aqui ficam as rotas que a tela usa (pelo protocolo local
 * do engine, como o chat) e o resumo compartilhado pela faixa da conversa e pela lateral.
 */

export type EstadoDoNo = "esperando_voce" | "trabalhando" | "pronto" | "erro" | "parado" | "abrindo";

export interface NoAoVivo {
  id: string;
  titulo: string | null;
  plataforma: string | null;
  conta_id: string | null;
  conta: string | null;
  modelo: string | null;
  esforco: string | null;
  tarefa: string | null;
  criado_em: number | null;
  /** Harness: o que o subagente pode editar e criar (relativo à pasta do orquestrador). */
  editar?: string[];
  criar?: string[];
  /** Fork (Copiloto abre conversa na tela): o usuário pediu para ver; a janela do copiloto abre numa aba. */
  na_tela?: boolean;
  estado: EstadoDoNo;
  estado_texto: string;
  /** A tarefa inicial ainda não foi digitada (a CLI está abrindo), ou o motivo de não ter ido. */
  pendente: string | null;
  /** Pedido de permissão que só o usuário aprova (o orquestrador é barrado), com o motivo. */
  arriscado?: string | null;
  ultima_mensagem: string | null;
  capabilities: Conversation["capabilities"];
}

export interface FluxoAoVivo {
  orquestrador: NoAoVivo;
  ativo: boolean;
  pode_orquestrar: boolean;
  e_subagente: boolean;
  limite: number;
  agentes: NoAoVivo[];
}

export interface ResumoDaOrquestracao {
  limite: number;
  orquestradores: { orquestrador: string; ativo: boolean; agentes: number; subagentes: string[] }[];
}

const json = { "Content-Type": "application/json" };
const caminho = (id: string) => `/orquestracao/${encodeURIComponent(id)}`;

export const obterFluxo = (orquestrador: string) => request<FluxoAoVivo>(caminho(orquestrador));

export async function ativarOrquestracao(conversa: string, ativo: boolean) {
  const resposta = await request<{ ativo: boolean; reaberta: boolean }>(`${caminho(conversa)}/ativar`, {
    method: "POST", headers: json, body: JSON.stringify({ ativo }),
  });
  void atualizarResumo();
  return resposta;
}

export const definirLimite = (limite: number) =>
  request<{ limite: number }>("/orquestracao/limite", { method: "POST", headers: json, body: JSON.stringify({ limite }) });

export const pararAgente = (orquestrador: string, agente: string) =>
  request<{ ok: boolean; mensagem: string }>(`${caminho(orquestrador)}/agentes/${encodeURIComponent(agente)}/parar`, { method: "POST" });

/* ── Resumo compartilhado (um poll só para a faixa e a lateral) ─────────────────────────────── */

const VAZIO: ResumoDaOrquestracao = { limite: 3, orquestradores: [] };
let resumo: ResumoDaOrquestracao = VAZIO;
const ouvintes = new Set<(valor: ResumoDaOrquestracao) => void>();
let timer: number | undefined;
let lidoEm = 0;

export async function atualizarResumo() {
  try {
    const lido = await request<Partial<ResumoDaOrquestracao>>("/orquestracao");
    // Engine antigo responde outra coisa (ou `{}`): só aceita o formato certo.
    if (!Array.isArray(lido?.orquestradores)) return;
    resumo = { limite: lido.limite ?? VAZIO.limite, orquestradores: lido.orquestradores.map((o) => ({ ...o, subagentes: o.subagentes ?? [] })) };
    lidoEm = Date.now();
    for (const ouvir of ouvintes) ouvir(resumo);
  } catch {
    // Engine antigo (sem a rota) ou fora do app: fica o último resumo.
  }
}

/** Relido a cada 5 s com a janela à vista; oculta ou na pausa de energia, a cada 30 s. */
function agendar() {
  window.clearTimeout(timer);
  if (ouvintes.size === 0) return;
  timer = window.setTimeout(async () => {
    const parado = (typeof document !== "undefined" && document.hidden) || energiaPausada();
    if (!parado || Date.now() - lidoEm > 30_000) await atualizarResumo();
    agendar();
  }, 5_000);
}

export function useResumoDaOrquestracao(): ResumoDaOrquestracao {
  const [valor, setValor] = useState(resumo);
  useEffect(() => {
    ouvintes.add(setValor);
    if (ouvintes.size === 1) { void atualizarResumo(); agendar(); }
    return () => { ouvintes.delete(setValor); if (ouvintes.size === 0) window.clearTimeout(timer); };
  }, []);
  return valor;
}

/** O que a faixa de uma conversa precisa saber dela. */
export function papelDaConversa(resumo: ResumoDaOrquestracao, conversa: string | null | undefined) {
  if (!conversa) return { orquestradora: false, agentes: 0, dona: null as string | null, teveFluxo: false };
  const propria = resumo.orquestradores.find((o) => o.orquestrador === conversa);
  const dona = resumo.orquestradores.find((o) => o.subagentes.includes(conversa))?.orquestrador ?? null;
  return { orquestradora: Boolean(propria?.ativo), agentes: propria?.agentes ?? 0, dona, teveFluxo: Boolean(propria && propria.agentes > 0) };
}

/** Só para os testes. */
export function _zerarResumo() {
  resumo = VAZIO;
  lidoEm = 0;
}

export const PODE_ORQUESTRAR = ["claude", "codex"];
