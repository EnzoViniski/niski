// Fork (Rotina): regras pequenas da tela, puras e testadas — estado salvo da tela (leitura tolerante),
// passos do registro rápido de série, formatação de números e datas.
import type { Serie } from "./tipos";

/** Último item (o alvo do TS ainda não tem `Array.prototype.at`). */
export const ultimo = <T,>(lista: T[]): T | undefined => lista[lista.length - 1];

export const CHAVE_DA_TELA = "medza-rotina";
export const ABAS = ["hoje", "semana", "treino", "alimentacao", "ajustes"] as const;
export type Aba = (typeof ABAS)[number];
export const ROTULOS_DAS_ABAS: Record<Aba, string> = { hoje: "Hoje", semana: "Semana", treino: "Treino", alimentacao: "Alimentação", ajustes: "Ajustes" };

export interface EstadoDaTela {
  aba: Aba;
  /** Coluna da conversa do agente aberta ao lado. */
  conversaAberta: boolean;
  /** Ficha escolhida no registro rápido (vazio = a sugerida). */
  ficha: string;
}

const PADRAO: EstadoDaTela = { aba: "hoje", conversaAberta: false, ficha: "" };

/** Lê o que a tela guardou. Valor de versão antiga, quebrado ou desconhecido vira o padrão campo a campo. */
export function lerEstadoDaTela(texto: string | null): EstadoDaTela {
  try {
    const v = JSON.parse(texto ?? "null") as Partial<EstadoDaTela> | null;
    if (!v || typeof v !== "object") return { ...PADRAO };
    return {
      aba: ABAS.includes(v.aba as Aba) ? (v.aba as Aba) : PADRAO.aba,
      conversaAberta: v.conversaAberta === true,
      ficha: typeof v.ficha === "string" ? v.ficha : "",
    };
  } catch {
    return { ...PADRAO };
  }
}

export function carregarEstadoDaTela(): EstadoDaTela {
  try { return lerEstadoDaTela(localStorage.getItem(CHAVE_DA_TELA)); } catch { return { ...PADRAO }; }
}

export function guardarEstadoDaTela(estado: EstadoDaTela) {
  try { localStorage.setItem(CHAVE_DA_TELA, JSON.stringify(estado)); } catch { /* sem localStorage */ }
}

/* ── Registro rápido de série ──────────────────────────────────────────────────────────────── */

export const PASSO_DA_CARGA = 2.5;

export function ajustarCarga(carga: number, passos: number): number {
  const nova = Math.round((carga + passos * PASSO_DA_CARGA) * 100) / 100;
  return Math.min(1000, Math.max(0, nova));
}

export function ajustarRepeticoes(reps: number, delta: number): number {
  return Math.min(500, Math.max(0, Math.round(reps + delta)));
}

/** Valores iniciais de uma série: a última série deste exercício hoje, senão a última vez, senão a ficha. */
export function valoresIniciais(hoje: Serie[], ultimaVez: Serie[], cargaDaFicha: number | null, repsDaFicha: string): { carga: number; reps: number } {
  const base = ultimo(hoje) ?? ultimo(ultimaVez);
  if (base) return { carga: base.carga_kg, reps: base.repeticoes };
  const reps = Number.parseInt(repsDaFicha, 10);
  return { carga: cargaDaFicha ?? 0, reps: Number.isFinite(reps) && reps > 0 ? reps : 10 };
}

/* ── Formatação ────────────────────────────────────────────────────────────────────────────── */

const numero = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 });
export const formatarNumero = (n: number) => numero.format(n);
export const formatarKg = (n: number) => `${numero.format(n)} kg`;
export const formatarLitros = (ml: number) => `${new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(ml / 1000)} L`;

/** `2026-10-23` → `23/10`. */
export function dataCurta(iso: string): string {
  const [, m, d] = iso.split("-");
  return d && m ? `${d}/${m}` : iso;
}

/** Data local de hoje em `AAAA-MM-DD`. */
export function hojeLocal(agora = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${agora.getFullYear()}-${p(agora.getMonth() + 1)}-${p(agora.getDate())}`;
}

export function somarDias(iso: string, dias: number): string {
  const [a, m, d] = iso.split("-").map(Number);
  return hojeLocal(new Date(a, (m ?? 1) - 1, (d ?? 1) + dias));
}

export function fracao(atual: number, meta: number): number {
  return meta > 0 ? Math.min(1, Math.max(0, atual / meta)) : 0;
}

/** Plataforma e conta padrão do agente da rotina: as do copiloto (`medza-copiloto`). */
export function padraoDoCopiloto(): { plataforma?: string } {
  try {
    const c = JSON.parse(localStorage.getItem("medza-copiloto") ?? "null") as { provider?: unknown } | null;
    return typeof c?.provider === "string" && c.provider ? { plataforma: c.provider } : {};
  } catch { return {}; }
}

/** Uma linha da ficha digitada: `Supino reto 4x8-10 60kg` → nome, séries, repetições, carga. */
export function lerLinhaDeExercicio(linha: string): { nome: string; series: number; repeticoes: string; carga_kg: number | null } | null {
  let resto = linha.trim();
  if (!resto) return null;
  let carga: number | null = null;
  const kg = resto.match(/(\d+(?:[.,]\d+)?)\s*kg\b/i);
  if (kg) { carga = Number(kg[1].replace(",", ".")); resto = resto.replace(kg[0], " "); }
  let series = 3, repeticoes = "10";
  const sr = resto.match(/(\d+)\s*[x×]\s*(\d+(?:\s*-\s*\d+)?)/i);
  if (sr) { series = Math.min(20, Math.max(1, Number(sr[1]))); repeticoes = sr[2].replace(/\s+/g, ""); resto = resto.replace(sr[0], " "); }
  const nome = resto.replace(/\s+/g, " ").trim();
  return nome ? { nome, series, repeticoes, carga_kg: carga } : null;
}

/** Séries da última vez que o exercício apareceu antes de `hoje` (sessões em qualquer ordem). */
export function ultimaVez(sessoes: { data: string; series: Serie[] }[], exercicio: string, hoje: string): Serie[] {
  const chave = (s: string) => s.trim().toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/\s+/g, " ");
  const alvo = chave(exercicio);
  const anteriores = sessoes.filter((s) => s.data < hoje).sort((a, b) => b.data.localeCompare(a.data));
  for (const s of anteriores) {
    const series = s.series.filter((x) => chave(x.exercicio) === alvo);
    if (series.length) return series;
  }
  return [];
}

export const mesmaChave = (a: string, b: string) => {
  const chave = (s: string) => s.trim().toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/\s+/g, " ");
  return chave(a) === chave(b);
};
