/**
 * Fork (troca-de-contexto): a "última olhada" em cada conversa — até que mensagem você viu e quando.
 *
 * É o que o cartão "onde parei" compara para dizer o que chegou desde então. Mora no localStorage
 * (chave própria, espelhada em `estado-da-interface.json` por `espelhoDaInterface.ts`, então sobrevive
 * a fechar, atualizar e ao WebKit perder as últimas gravações). No celular fica no navegador dele.
 *
 * Formato: `{ v: 1, conversas: { <id>: { n, em, tom? } } }`. Leitura tolerante: item estragado é
 * ignorado sozinho, versão desconhecida vira "nunca olhou" (nunca quebra a abertura). Guarda no máximo
 * `MAXIMO` conversas, as olhadas mais recentes.
 */

export const CHAVE_DA_OLHADA = "medza-ultima-olhada";
const MAXIMO = 400;

/** Em que pé a conversa estava quando você olhou (o tom do estado, ver `ondeParei.ts`). */
export type TomDaOlhada = "precisa" | "erro" | "trabalhando" | "pronto" | "parado";
const TONS: readonly TomDaOlhada[] = ["precisa", "erro", "trabalhando", "pronto", "parado"];

export interface Olhada {
  /** Maior número de mensagem (`<conversa>:<n>`) que estava na timeline quando você olhou. */
  n: number;
  /** Quando você deixou de olhar (ms). */
  em: number;
  tom?: TomDaOlhada | null;
}

type Ouvinte = () => void;
const ouvintes = new Set<Ouvinte>();
let versao = 0;

function valida(item: unknown): Olhada | null {
  if (!item || typeof item !== "object" || Array.isArray(item)) return null;
  const { n, em, tom } = item as Record<string, unknown>;
  if (typeof n !== "number" || !Number.isFinite(n) || typeof em !== "number" || !Number.isFinite(em) || em <= 0) return null;
  return { n, em, tom: typeof tom === "string" && (TONS as readonly string[]).includes(tom) ? (tom as TomDaOlhada) : null };
}

// A conversa aberta lê a olhada a cada redesenho: o texto lido por último fica guardado já interpretado.
let cache: { texto: string | null; olhadas: Record<string, Olhada> } | null = null;

function atuais(): Record<string, Olhada> {
  let texto: string | null;
  try { texto = localStorage.getItem(CHAVE_DA_OLHADA); } catch { return {}; }
  if (!cache || cache.texto !== texto) cache = { texto, olhadas: interpretar(texto) };
  return cache.olhadas;
}

export function lerOlhadas(): Record<string, Olhada> {
  return { ...atuais() };
}

function interpretar(texto: string | null): Record<string, Olhada> {
  try {
    const bruto = JSON.parse(texto ?? "null");
    if (!bruto || typeof bruto !== "object" || bruto.v !== 1 || !bruto.conversas || typeof bruto.conversas !== "object") return {};
    const saida: Record<string, Olhada> = {};
    for (const [id, item] of Object.entries(bruto.conversas as Record<string, unknown>)) {
      const olhada = valida(item);
      if (olhada) saida[id] = olhada;
    }
    return saida;
  } catch {
    return {};
  }
}

export function lerOlhada(conversaId: string | null | undefined): Olhada | null {
  if (!conversaId) return null;
  return atuais()[conversaId] ?? null;
}

/** Grava a olhada de uma conversa. Nunca anda para trás no número (uma leitura atrasada da timeline
 *  não desmarca o que você já viu). Sem armazenamento: só não lembra. */
export function marcarOlhada(conversaId: string, olhada: Olhada) {
  if (!conversaId || !Number.isFinite(olhada.n) || !Number.isFinite(olhada.em)) return;
  const todas = lerOlhadas();
  const anterior = todas[conversaId];
  const nova: Olhada = { n: Math.max(olhada.n, anterior?.n ?? -1), em: Math.max(olhada.em, anterior?.em ?? 0), tom: olhada.tom ?? anterior?.tom ?? null };
  if (anterior && anterior.n === nova.n && anterior.em === nova.em && anterior.tom === nova.tom) return;
  todas[conversaId] = nova;
  const ordenadas = Object.entries(todas).sort((a, b) => b[1].em - a[1].em).slice(0, MAXIMO);
  try {
    localStorage.setItem(CHAVE_DA_OLHADA, JSON.stringify({ v: 1, conversas: Object.fromEntries(ordenadas) }));
  } catch {
    return;
  }
  versao += 1;
  ouvintes.forEach((ouvinte) => ouvinte());
}

/** Para `useSyncExternalStore`: muda a cada gravação. */
export function versaoDasOlhadas() { return versao; }
export function ouvirOlhadas(ouvinte: Ouvinte) {
  ouvintes.add(ouvinte);
  return () => { ouvintes.delete(ouvinte); };
}
