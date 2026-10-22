/**
 * Fork (Manhã): rotina da manhã — ajustes, sessão do dia e histórico em `medza-manha` (localStorage,
 * espelhado em `estado-da-interface.json`). A sessão aparece por cima de tudo a partir do horário
 * escolhido: com o app aberto, na hora; com o PC ligado depois, assim que o app abrir.
 */
import { useSyncExternalStore } from "react";
import { criarRng, embaralhar, gerarQuestao, semente, sortearTemas, TEMAS_GERADOS, type Nivel, type Questao, type Tema, type TemaGerado } from "./gerador";
import type { IaEscolhida } from "../roadmaps/tipos";

export const CHAVE_DA_MANHA = "medza-manha";

export interface AjustesDaManha {
  ativo: boolean;
  /** `HH:MM`, no fuso do PC. */
  horario: string;
  quantidade: number;
  dificuldade: "facil" | "medio" | "dificil" | "adaptativa";
  temas: Tema[];
  /** Assunto do tema livre (ex.: "inglês", "anatomia", "história do Brasil"). */
  temaLivre: string;
  ia: IaEscolhida;
  /** Mostrar as tarefas de hoje no fim da sessão. */
  tarefasNoFim: boolean;
}

export interface Sessao {
  dia: string;
  questoes: Questao[];
  /** Índice escolhido por questão (null = ainda não respondeu). */
  respostas: (number | null)[];
  segundos: number[];
  inicio: string;
  fim: string;
  /** "Agora não": volta a aparecer depois deste horário (ISO). */
  adiadaAte: string;
  pulada: boolean;
}

export interface DiaFeito { dia: string; acertos: number; total: number; segundos: number }

interface Gravado { ajustes: AjustesDaManha; sessao: Sessao | null; historico: DiaFeito[] }

export const AJUSTES_PADRAO: AjustesDaManha = {
  ativo: true, horario: "06:00", quantidade: 5, dificuldade: "adaptativa",
  temas: ["aritmetica", "porcentagem", "fracoes", "equacoes", "sequencias", "logica", "potencias", "problemas"],
  temaLivre: "", ia: { cli: "claude" }, tarefasNoFim: true,
};

const TODOS_OS_TEMAS: Tema[] = [...TEMAS_GERADOS, "roadmaps", "livre"];

function lerGravado(): Gravado {
  try {
    const bruto = JSON.parse(localStorage.getItem(CHAVE_DA_MANHA) ?? "null") as Partial<Gravado> | null;
    const a = (bruto?.ajustes ?? {}) as Partial<AjustesDaManha>;
    const ajustes: AjustesDaManha = {
      ativo: typeof a.ativo === "boolean" ? a.ativo : AJUSTES_PADRAO.ativo,
      horario: typeof a.horario === "string" && /^([01]\d|2[0-3]):[0-5]\d$/.test(a.horario) ? a.horario : AJUSTES_PADRAO.horario,
      quantidade: typeof a.quantidade === "number" ? Math.min(20, Math.max(1, Math.round(a.quantidade))) : AJUSTES_PADRAO.quantidade,
      dificuldade: ["facil", "medio", "dificil", "adaptativa"].includes(a.dificuldade as string) ? (a.dificuldade as AjustesDaManha["dificuldade"]) : AJUSTES_PADRAO.dificuldade,
      temas: Array.isArray(a.temas) ? a.temas.filter((t): t is Tema => TODOS_OS_TEMAS.includes(t as Tema)) : AJUSTES_PADRAO.temas,
      temaLivre: typeof a.temaLivre === "string" ? a.temaLivre.slice(0, 200) : "",
      ia: a.ia && typeof a.ia === "object" && (a.ia.cli === "claude" || a.ia.cli === "codex") ? a.ia : AJUSTES_PADRAO.ia,
      tarefasNoFim: a.tarefasNoFim !== false,
    };
    const sessao = bruto?.sessao && typeof bruto.sessao === "object" && Array.isArray(bruto.sessao.questoes) ? (bruto.sessao as Sessao) : null;
    const historico = Array.isArray(bruto?.historico) ? (bruto.historico as DiaFeito[]).filter((d) => typeof d?.dia === "string").slice(-120) : [];
    return { ajustes, sessao, historico };
  } catch {
    return { ajustes: { ...AJUSTES_PADRAO }, sessao: null, historico: [] };
  }
}

let gravado = lerGravado();
const ouvintes = new Set<() => void>();
function gravar(novo: Partial<Gravado>) {
  gravado = { ...gravado, ...novo };
  try { localStorage.setItem(CHAVE_DA_MANHA, JSON.stringify(gravado)); } catch { /* sem localStorage: vale a memória */ }
  ouvintes.forEach((o) => o());
}
export function useManha(): Gravado {
  return useSyncExternalStore((o) => { ouvintes.add(o); return () => ouvintes.delete(o); }, () => gravado);
}
export const estadoDaManha = () => gravado;
export function salvarAjustesDaManha(mudanca: Partial<AjustesDaManha>) { gravar({ ajustes: { ...gravado.ajustes, ...mudanca } }); }
/** Só para testes. */
export function recarregarDaManha() { gravado = lerGravado(); }

// ---- Quando aparece ----

export function diaLocal(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** A sessão de hoje deve estar na tela agora? */
export function deveMostrar(g: Gravado, agora: Date): boolean {
  if (!g.ajustes.ativo || !g.ajustes.temas.length) return false;
  const [h, m] = g.ajustes.horario.split(":").map(Number);
  if (agora.getHours() * 60 + agora.getMinutes() < h * 60 + m) return false;
  const s = g.sessao;
  if (!s || s.dia !== diaLocal(agora)) return true;
  if (s.fim || s.pulada) return false;
  return !s.adiadaAte || agora.getTime() >= Date.parse(s.adiadaAte);
}

/** Dias seguidos com a sessão feita, terminando hoje ou ontem. */
export function sequencia(historico: DiaFeito[], hoje: string): number {
  const dias = new Set(historico.map((d) => d.dia));
  const cursor = new Date(`${hoje}T12:00:00`);
  if (!dias.has(hoje)) cursor.setDate(cursor.getDate() - 1);
  let n = 0;
  while (dias.has(diaLocal(cursor))) { n++; cursor.setDate(cursor.getDate() - 1); }
  return n;
}

/** Nível pela dificuldade escolhida; "adaptativa" sobe com acerto alto e desce com baixo (últimos 5 dias). */
export function nivelDoDia(ajustes: AjustesDaManha, historico: DiaFeito[]): Nivel {
  if (ajustes.dificuldade === "facil") return 1;
  if (ajustes.dificuldade === "medio") return 2;
  if (ajustes.dificuldade === "dificil") return 3;
  const recentes = historico.slice(-5);
  if (recentes.length < 2) return 2;
  const taxa = recentes.reduce((s, d) => s + d.acertos, 0) / Math.max(1, recentes.reduce((s, d) => s + d.total, 0));
  return taxa >= 0.85 ? 3 : taxa < 0.5 ? 1 : 2;
}

// ---- Montar a sessão ----

/** Questões feitas fora do gerador (revisão dos roadmaps, tema livre por IA). */
export interface FontesExtras {
  roadmaps?: (quantidade: number, rng: () => number) => Promise<Questao[]>;
  livre?: (quantidade: number, assunto: string, ia: IaEscolhida, nivel: Nivel) => Promise<Questao[]>;
}

/**
 * As questões do dia: temas distribuídos entre os escolhidos; o que as fontes extras não
 * conseguirem entregar (sem cards, IA fora do ar) vira questão de matemática, para nunca faltar.
 */
export async function montarQuestoes(ajustes: AjustesDaManha, historico: DiaFeito[], dia: string, fontes: FontesExtras): Promise<Questao[]> {
  const rng = criarRng(semente(`manha-${dia}`));
  const nivel = nivelDoDia(ajustes, historico);
  const temas = sortearTemas(ajustes.temas.length ? ajustes.temas : AJUSTES_PADRAO.temas, ajustes.quantidade, rng);
  const pedidos = { roadmaps: temas.filter((t) => t === "roadmaps").length, livre: temas.filter((t) => t === "livre").length };
  const [deRoadmaps, deIa] = await Promise.all([
    pedidos.roadmaps && fontes.roadmaps ? fontes.roadmaps(pedidos.roadmaps, rng).catch(() => []) : Promise.resolve([]),
    pedidos.livre && fontes.livre && ajustes.temaLivre.trim() ? fontes.livre(pedidos.livre, ajustes.temaLivre.trim(), ajustes.ia, nivel).catch(() => []) : Promise.resolve([]),
  ]);
  const extras = { roadmaps: deRoadmaps.slice(), livre: deIa.slice() };
  const geradosEscolhidos = ajustes.temas.filter((t): t is TemaGerado => TEMAS_GERADOS.includes(t as TemaGerado));
  const reserva = geradosEscolhidos.length ? geradosEscolhidos : (["aritmetica", "logica", "sequencias"] as TemaGerado[]);
  return temas.map((tema, i) => {
    const id = `${dia}-${i}`;
    if (tema === "roadmaps" || tema === "livre") {
      const pronta = extras[tema].shift();
      if (pronta) return { ...pronta, id };
      return gerarQuestao(reserva[i % reserva.length], nivel, rng, id);
    }
    return gerarQuestao(tema, nivel, rng, id);
  });
}

export function iniciarSessao(questoes: Questao[], agora = new Date()) {
  gravar({ sessao: { dia: diaLocal(agora), questoes, respostas: questoes.map(() => null), segundos: questoes.map(() => 0), inicio: agora.toISOString(), fim: "", adiadaAte: "", pulada: false } });
}

export function responder(indice: number, escolha: number, segundos: number) {
  const s = gravado.sessao;
  if (!s || s.respostas[indice] !== null) return;
  const respostas = s.respostas.slice(); respostas[indice] = escolha;
  const tempos = s.segundos.slice(); tempos[indice] = Math.round(segundos);
  gravar({ sessao: { ...s, respostas, segundos: tempos } });
}

export function terminar(agora = new Date()) {
  const s = gravado.sessao;
  if (!s || s.fim) return;
  const acertos = s.questoes.filter((q, i) => s.respostas[i] === q.correta).length;
  const feito: DiaFeito = { dia: s.dia, acertos, total: s.questoes.length, segundos: s.segundos.reduce((a, b) => a + b, 0) };
  gravar({ sessao: { ...s, fim: agora.toISOString() }, historico: [...gravado.historico.filter((d) => d.dia !== s.dia), feito].slice(-120) });
}

export function adiar(minutos: number, agora = new Date()) {
  const s = gravado.sessao;
  const ate = new Date(agora.getTime() + minutos * 60_000).toISOString();
  if (s && s.dia === diaLocal(agora)) gravar({ sessao: { ...s, adiadaAte: ate } });
  else gravar({ sessao: { dia: diaLocal(agora), questoes: [], respostas: [], segundos: [], inicio: "", fim: "", adiadaAte: ate, pulada: false } });
}

export function pularHoje(agora = new Date()) {
  const s = gravado.sessao;
  const base: Sessao = s && s.dia === diaLocal(agora) ? s : { dia: diaLocal(agora), questoes: [], respostas: [], segundos: [], inicio: "", fim: "", adiadaAte: "", pulada: false };
  gravar({ sessao: { ...base, pulada: true } });
}

/** "Fazer agora" (pelas Configurações ou ⌘K): recomeça a sessão de hoje mesmo fora do horário. */
export function refazerHoje() {
  gravar({ sessao: null });
}

export { embaralhar };
