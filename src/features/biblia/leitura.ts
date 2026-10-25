/**
 * Fork (Bíblia do dia): ajustes, onde parou em cada trilha e o que já foi lido, em `medza-biblia`
 * (localStorage, espelhado em `estado-da-interface.json`).
 *
 * Nunca repete: cada trilha tem um marcador que só anda quando você marca "Li". "Deixar para amanhã"
 * e "Ler mais tarde" não mexem nele: a mesma leitura espera por você. Só no fim de uma trilha inteira
 * ela recomeça do início (ciclo + 1).
 */
import { useSyncExternalStore } from "react";
import { diaLocal } from "../manha/rotina";
import { marcadorDepois, passagemAtual, passagemPeloId, TRILHAS, type Biblia, type Marcador, type Passagem, type Trilha } from "./biblia";

export const CHAVE_DA_BIBLIA = "medza-biblia";

export interface AjustesDaBiblia {
  ativa: boolean;
  /** Uma passagem por trilha, por dia. */
  trilhas: Trilha[];
  /** Tamanho da letra na leitura. */
  letra: "normal" | "grande";
}

/** O que você decidiu hoje. Sem registro do dia = ainda não decidiu. */
export interface DecisaoDoDia { dia: string; estado: "lida" | "amanha" | "depois"; em: string }

export interface LeituraFeita { dia: string; ids: string[]; trilhas: Trilha[]; refs: string[] }

export interface Gravado {
  ajustes: AjustesDaBiblia;
  marcadores: Partial<Record<Trilha, Marcador>>;
  hoje: DecisaoDoDia | null;
  historico: LeituraFeita[];
}

export const AJUSTES_PADRAO: AjustesDaBiblia = { ativa: true, trilhas: ["evangelhos", "salmos", "sabedoria"], letra: "normal" };

function marcadorValido(m: unknown): m is Marcador {
  const x = m as Marcador;
  return !!x && [x.livro, x.capitulo, x.versiculo, x.ciclo].every((n) => typeof n === "number" && Number.isFinite(n));
}

function lerGravado(): Gravado {
  try {
    const bruto = JSON.parse(localStorage.getItem(CHAVE_DA_BIBLIA) ?? "null") as Partial<Gravado> | null;
    const a = (bruto?.ajustes ?? {}) as Partial<AjustesDaBiblia>;
    const ajustes: AjustesDaBiblia = {
      ativa: typeof a.ativa === "boolean" ? a.ativa : AJUSTES_PADRAO.ativa,
      trilhas: Array.isArray(a.trilhas) ? TRILHAS.filter((t) => a.trilhas?.includes(t)) : AJUSTES_PADRAO.trilhas,
      letra: a.letra === "grande" ? "grande" : "normal",
    };
    const marcadores: Gravado["marcadores"] = {};
    for (const t of TRILHAS) { const m = bruto?.marcadores?.[t]; if (marcadorValido(m)) marcadores[t] = m; }
    const h = bruto?.hoje;
    const hoje = h && typeof h.dia === "string" && ["lida", "amanha", "depois"].includes(h.estado) ? h : null;
    const historico = Array.isArray(bruto?.historico)
      ? bruto.historico.filter((d) => typeof d?.dia === "string" && Array.isArray(d.ids) && Array.isArray(d.refs)).slice(-400)
      : [];
    return { ajustes, marcadores, hoje, historico };
  } catch {
    return { ajustes: { ...AJUSTES_PADRAO }, marcadores: {}, hoje: null, historico: [] };
  }
}

let gravado = lerGravado();
const ouvintes = new Set<() => void>();
function gravar(novo: Partial<Gravado>) {
  gravado = { ...gravado, ...novo };
  try { localStorage.setItem(CHAVE_DA_BIBLIA, JSON.stringify(gravado)); } catch { /* sem localStorage: vale a memória */ }
  ouvintes.forEach((o) => o());
}
export function useLeitura(): Gravado {
  return useSyncExternalStore((o) => { ouvintes.add(o); return () => ouvintes.delete(o); }, () => gravado);
}
export const estadoDaLeitura = () => gravado;
export function salvarAjustesDaBiblia(mudanca: Partial<AjustesDaBiblia>) { gravar({ ajustes: { ...gravado.ajustes, ...mudanca } }); }
/** Só para testes. */
export function recarregarDaBiblia() { gravado = lerGravado(); }

// ---- O dia ----

export interface LeituraDoDia { trilha: Trilha; passagem: Passagem; indice: number; total: number; ciclo: number }

/** As passagens de agora: uma por trilha escolhida, a partir de onde parou. */
export function leituraDeAgora(biblia: Biblia, g: Gravado = gravado): LeituraDoDia[] {
  return TRILHAS.filter((t) => g.ajustes.trilhas.includes(t)).flatMap((trilha) => {
    const atual = passagemAtual(biblia, trilha, g.marcadores[trilha]);
    return atual ? [{ trilha, ...atual, ciclo: g.marcadores[trilha]?.ciclo ?? 0 }] : [];
  });
}

/** O que foi lido hoje (pode ser mais de uma rodada, se pediu "ler mais"). */
export function lidasHoje(biblia: Biblia, g: Gravado, agora: Date): Passagem[] {
  const dia = diaLocal(agora);
  return g.historico.filter((h) => h.dia === dia)
    .flatMap((h) => h.ids.map((id, i) => passagemPeloId(biblia, h.trilhas[i], id)))
    .filter((p): p is Passagem => !!p);
}

export function lidaHoje(g: Gravado, agora: Date): boolean {
  return g.hoje?.dia === diaLocal(agora) && g.hoje.estado === "lida";
}

/** Ainda falta ler hoje (para o selo da lateral e o cartão da Rotina). */
export function pendenteHoje(g: Gravado, agora: Date): boolean {
  return g.ajustes.ativa && g.ajustes.trilhas.length > 0 && !lidaHoje(g, agora);
}

/**
 * Abre sozinha? Só uma vez por dia, depois dos exercícios da manhã (ou, com os exercícios
 * desligados, a partir do horário da manhã). "Mais tarde" e "amanhã" contam como decisão do dia.
 */
export function deveAbrirSozinha(g: Gravado, manha: { ativo: boolean; horario: string; terminouHoje: boolean }, agora: Date): boolean {
  if (!g.ajustes.ativa || !g.ajustes.trilhas.length) return false;
  if (g.hoje?.dia === diaLocal(agora)) return false;
  if (manha.ativo) return manha.terminouHoje;
  const [h, m] = manha.horario.split(":").map(Number);
  return agora.getHours() * 60 + agora.getMinutes() >= h * 60 + m;
}

export function marcarComoLida(biblia: Biblia, leitura: LeituraDoDia[], refs: string[], agora = new Date()) {
  if (!leitura.length) return;
  const marcadores = { ...gravado.marcadores };
  for (const l of leitura) marcadores[l.trilha] = marcadorDepois(biblia, l.passagem, marcadores[l.trilha]);
  const dia = diaLocal(agora);
  const feita: LeituraFeita = { dia, ids: leitura.map((l) => l.passagem.id), trilhas: leitura.map((l) => l.trilha), refs };
  gravar({ marcadores, hoje: { dia, estado: "lida", em: agora.toISOString() }, historico: [...gravado.historico, feita].slice(-400) });
}

/** "Deixar para amanhã" e "Ler mais tarde": não mexem no marcador (nada se perde, nada se repete). */
export function decidir(estado: "amanha" | "depois", agora = new Date()) {
  if (lidaHoje(gravado, agora)) return;
  gravar({ hoje: { dia: diaLocal(agora), estado, em: agora.toISOString() } });
}

/** Dias seguidos com leitura feita, terminando hoje ou ontem. */
export function diasSeguidos(g: Gravado, agora: Date): number {
  const dias = new Set(g.historico.map((d) => d.dia));
  const cursor = new Date(agora);
  cursor.setHours(12, 0, 0, 0);
  if (!dias.has(diaLocal(cursor))) cursor.setDate(cursor.getDate() - 1);
  let n = 0;
  while (dias.has(diaLocal(cursor))) { n++; cursor.setDate(cursor.getDate() - 1); }
  return n;
}
