/**
 * Fork (Tarefas): regras sem I/O — normalizar o que vem do disco, mesclar edições de duas pessoas,
 * recorrência, filtros do dia/semana e a criação rápida por texto ("dentista amanhã 14h #saude").
 */
import {
  CATEGORIAS, PRIORIDADES, RECORRENCIAS, STATUS, VERSAO_DA_TAREFA,
  type Comentario, type ItemDoChecklist, type Pessoa, type Prioridade, type Recorrencia, type Status, type Tarefa,
} from "./tipos";

const texto = (v: unknown, max = 20_000) => (typeof v === "string" ? v.slice(0, max) : "");
const lista = <T>(v: unknown, ler: (x: unknown) => T | null): T[] => (Array.isArray(v) ? v.map(ler).filter((x): x is T => x !== null) : []);
const umDe = <T extends string>(opcoes: readonly T[], v: unknown, padrao: T): T => (opcoes.includes(v as T) ? (v as T) : padrao);
const DIA = /^\d{4}-\d{2}-\d{2}$/;
const HORA = /^([01]\d|2[0-3]):[0-5]\d$/;

export function novoId(prefixo = "t"): string {
  const aleatorio = Math.floor(Math.random() * 0xffffff).toString(36);
  return `${prefixo}-${Date.now().toString(36)}-${aleatorio}`;
}

/** Tarefa lida do disco: campo inválido cai no padrão, sem descartar a tarefa. */
export function normalizarTarefa(bruto: unknown): Tarefa | null {
  if (!bruto || typeof bruto !== "object") return null;
  const t = bruto as Record<string, unknown>;
  if (typeof t.id !== "string" || !/^[A-Za-z0-9_-]{1,80}$/.test(t.id)) return null;
  const agora = new Date(0).toISOString();
  return {
    versao: typeof t.versao === "number" ? t.versao : VERSAO_DA_TAREFA,
    id: t.id,
    titulo: texto(t.titulo, 300) || "Sem título",
    descricao: texto(t.descricao),
    status: umDe(STATUS, t.status, "a_fazer"),
    prioridade: umDe(PRIORIDADES, t.prioridade, "media"),
    categoria: CATEGORIAS.some((c) => c.id === t.categoria) ? (t.categoria as string) : "outro",
    etiquetas: lista(t.etiquetas, (x) => (typeof x === "string" && x.trim() ? x.trim().slice(0, 40) : null)),
    responsavel: texto(t.responsavel, 80),
    criadaPor: texto(t.criadaPor, 80),
    data: typeof t.data === "string" && DIA.test(t.data) ? t.data : "",
    hora: typeof t.hora === "string" && HORA.test(t.hora) ? t.hora : "",
    duracaoMin: typeof t.duracaoMin === "number" && t.duracaoMin > 0 ? Math.round(t.duracaoMin) : 0,
    local: texto(t.local, 200),
    recorrencia: umDe(RECORRENCIAS, t.recorrencia, "nunca"),
    checklist: lista(t.checklist, (x): ItemDoChecklist | null => {
      const i = x as Record<string, unknown> | null;
      return i && typeof i.id === "string" ? { id: i.id, texto: texto(i.texto, 500), feito: i.feito === true } : null;
    }),
    relacionadas: lista(t.relacionadas, (x) => (typeof x === "string" ? x : null)),
    comentarios: lista(t.comentarios, (x): Comentario | null => {
      const c = x as Record<string, unknown> | null;
      return c && typeof c.id === "string" ? { id: c.id, autor: texto(c.autor, 80), texto: texto(c.texto, 5000), em: texto(c.em, 40) } : null;
    }),
    posicao: typeof t.posicao === "number" && Number.isFinite(t.posicao) ? t.posicao : 0,
    criadaEm: texto(t.criadaEm, 40) || agora,
    atualizadaEm: texto(t.atualizadaEm, 40) || agora,
    concluidaEm: texto(t.concluidaEm, 40),
    apagada: t.apagada === true,
  };
}

export function normalizarPessoa(bruto: unknown): Pessoa | null {
  const p = bruto as Record<string, unknown> | null;
  if (!p || typeof p.id !== "string" || typeof p.nome !== "string" || !p.nome.trim()) return null;
  return { id: p.id, nome: p.nome.trim().slice(0, 60), cor: texto(p.cor, 20) || "#7d8a99", atualizadaEm: texto(p.atualizadaEm, 40) };
}

export function criarTarefa(campos: Partial<Tarefa> & { titulo: string }, autor: string, agora = new Date()): Tarefa {
  const iso = agora.toISOString();
  return {
    versao: VERSAO_DA_TAREFA, id: novoId(), descricao: "", status: "a_fazer", prioridade: "media", categoria: "outro",
    etiquetas: [], responsavel: autor, criadaPor: autor, data: "", hora: "", duracaoMin: 0, local: "", recorrencia: "nunca",
    checklist: [], relacionadas: [], comentarios: [], posicao: agora.getTime(), criadaEm: iso, atualizadaEm: iso, concluidaEm: "",
    apagada: false, ...campos,
  };
}

/**
 * Duas versões da mesma tarefa (a minha e a que chegou pela pasta): vale a editada por último, mas
 * comentários das duas se juntam — ninguém perde o que escreveu porque o outro salvou depois.
 */
export function mesclar(a: Tarefa, b: Tarefa): Tarefa {
  const [nova, velha] = a.atualizadaEm >= b.atualizadaEm ? [a, b] : [b, a];
  const ids = new Set(nova.comentarios.map((c) => c.id));
  const comentarios = [...nova.comentarios, ...velha.comentarios.filter((c) => !ids.has(c.id))].sort((x, y) => x.em.localeCompare(y.em));
  return comentarios.length === nova.comentarios.length ? nova : { ...nova, comentarios };
}

// ---- Datas (no fuso do PC) ----

export function diaDe(data: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${data.getFullYear()}-${p(data.getMonth() + 1)}-${p(data.getDate())}`;
}
export function paraData(dia: string): Date {
  const [a, m, d] = dia.split("-").map(Number);
  return new Date(a, m - 1, d);
}
export function somarDias(dia: string, n: number): string {
  const d = paraData(dia);
  d.setDate(d.getDate() + n);
  return diaDe(d);
}

/** Próxima data de uma tarefa recorrente, a partir da data dela (ou de hoje, se não tiver). */
export function proximaData(dia: string, recorrencia: Recorrencia, hoje: string): string {
  const base = dia || hoje;
  if (recorrencia === "diaria") return somarDias(base, 1);
  if (recorrencia === "semanal") return somarDias(base, 7);
  if (recorrencia === "dias_uteis") {
    let d = somarDias(base, 1);
    while ([0, 6].includes(paraData(d).getDay())) d = somarDias(d, 1);
    return d;
  }
  if (recorrencia === "mensal") {
    const d = paraData(base);
    const alvo = new Date(d.getFullYear(), d.getMonth() + 1, 1);
    const ultimo = new Date(alvo.getFullYear(), alvo.getMonth() + 1, 0).getDate();
    alvo.setDate(Math.min(d.getDate(), ultimo));
    return diaDe(alvo);
  }
  return base;
}

/**
 * Concluir: a recorrente não some — volta para "A fazer" na próxima data, com checklist zerado.
 * Devolve a tarefa atualizada.
 */
export function concluir(t: Tarefa, agora = new Date()): Tarefa {
  const iso = agora.toISOString();
  if (t.recorrencia !== "nunca") {
    return {
      ...t, status: "a_fazer", data: proximaData(t.data, t.recorrencia, diaDe(agora)),
      checklist: t.checklist.map((i) => ({ ...i, feito: false })), atualizadaEm: iso, concluidaEm: iso,
    };
  }
  return { ...t, status: "feito", atualizadaEm: iso, concluidaEm: iso };
}

export function reabrir(t: Tarefa, agora = new Date()): Tarefa {
  return { ...t, status: "a_fazer", concluidaEm: "", atualizadaEm: agora.toISOString() };
}

// ---- Filtros e ordem ----

const PESO: Record<Prioridade, number> = { urgente: 0, alta: 1, media: 2, baixa: 3 };

/** Ordem do dia: com hora primeiro (pela hora), depois por prioridade. */
export function ordenarDoDia(a: Tarefa, b: Tarefa): number {
  if (a.hora && b.hora) return a.hora.localeCompare(b.hora);
  if (a.hora !== b.hora) return a.hora ? -1 : 1;
  return PESO[a.prioridade] - PESO[b.prioridade] || a.posicao - b.posicao;
}

export const aberta = (t: Tarefa) => !t.apagada && t.status !== "feito";
export const atrasada = (t: Tarefa, hoje: string) => aberta(t) && !!t.data && t.data < hoje;

export interface Dia { atrasadas: Tarefa[]; hoje: Tarefa[]; feitasHoje: Tarefa[]; semData: Tarefa[]; proximas: Tarefa[] }

/** As listas da visão "Meu dia". */
export function montarDia(tarefas: Tarefa[], hoje: string): Dia {
  const vivas = tarefas.filter((t) => !t.apagada);
  const semana = somarDias(hoje, 7);
  return {
    atrasadas: vivas.filter((t) => atrasada(t, hoje)).sort((a, b) => a.data.localeCompare(b.data) || ordenarDoDia(a, b)),
    hoje: vivas.filter((t) => aberta(t) && (t.data === hoje || (t.status === "fazendo" && !t.data))).sort(ordenarDoDia),
    feitasHoje: vivas.filter((t) => t.concluidaEm && diaDe(new Date(t.concluidaEm)) === hoje).sort((a, b) => b.concluidaEm.localeCompare(a.concluidaEm)),
    semData: vivas.filter((t) => aberta(t) && !t.data && t.status !== "fazendo").sort((a, b) => PESO[a.prioridade] - PESO[b.prioridade] || a.posicao - b.posicao),
    proximas: vivas.filter((t) => aberta(t) && t.data > hoje && t.data <= semana).sort((a, b) => a.data.localeCompare(b.data) || ordenarDoDia(a, b)),
  };
}

export function filtrar(tarefas: Tarefa[], filtro: { pessoa: string; eu: string; categoria: string; busca: string }): Tarefa[] {
  const busca = filtro.busca.trim().toLowerCase();
  return tarefas.filter((t) => {
    if (t.apagada) return false;
    if (filtro.pessoa === "minhas" && t.responsavel !== filtro.eu) return false;
    if (filtro.pessoa !== "todas" && filtro.pessoa !== "minhas" && t.responsavel !== filtro.pessoa) return false;
    if (filtro.categoria && t.categoria !== filtro.categoria) return false;
    if (busca && !`${t.titulo}\n${t.descricao}\n${t.etiquetas.join(" ")}\n${t.local}`.toLowerCase().includes(busca)) return false;
    return true;
  });
}

/** Coluna do quadro em ordem; feitas há mais de 7 dias somem se `ocultarAntigas`. */
export function coluna(tarefas: Tarefa[], status: Status, hoje: string, ocultarAntigas: boolean): Tarefa[] {
  const corte = somarDias(hoje, -7);
  return tarefas
    .filter((t) => t.status === status && !(ocultarAntigas && status === "feito" && t.concluidaEm && diaDe(new Date(t.concluidaEm)) < corte))
    .sort((a, b) => (status === "feito" ? b.concluidaEm.localeCompare(a.concluidaEm) : a.posicao - b.posicao));
}

/** Posição para soltar entre dois vizinhos (média), ou antes/depois das pontas. */
export function posicaoEntre(antes: number | undefined, depois: number | undefined): number {
  if (antes === undefined && depois === undefined) return Date.now();
  if (antes === undefined) return (depois as number) - 1024;
  if (depois === undefined) return antes + 1024;
  return (antes + depois) / 2;
}

// ---- Criação rápida ----

const DIAS_DA_SEMANA: Record<string, number> = { domingo: 0, segunda: 1, terca: 2, terça: 2, quarta: 3, quinta: 4, sexta: 5, sabado: 6, sábado: 6 };

export interface Rascunho {
  titulo: string;
  data: string;
  hora: string;
  categoria: string;
  prioridade: Prioridade;
  responsavel: string;
  recorrencia: Recorrencia;
}

/**
 * "Consulta dentista amanhã 14h #saude !alta @primo" → título e campos. Entende: hoje, amanhã,
 * depois de amanhã, dia da semana, dd/mm; 14h, 14:30, 9h15; #categoria; !prioridade; @pessoa;
 * "todo dia", "toda semana", "todo mês", "dias úteis".
 */
export function lerCriacaoRapida(entrada: string, hoje: string, pessoas: Pessoa[], eu: string): Rascunho {
  let resto = ` ${entrada.trim()} `;
  const r: Rascunho = { titulo: "", data: "", hora: "", categoria: "outro", prioridade: "media", responsavel: eu, recorrencia: "nunca" };
  const tirar = (re: RegExp, aoAchar: (m: RegExpMatchArray) => boolean | void) => {
    const m = resto.match(re);
    if (m && aoAchar(m) !== false) resto = resto.replace(m[0], " ");
  };
  const semAcento = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

  tirar(/\s(todo dia|todos os dias|diariamente)\s/i, () => { r.recorrencia = "diaria"; });
  tirar(/\s(dias [uú]teis)\s/i, () => { r.recorrencia = "dias_uteis"; });
  tirar(/\s(toda semana|semanalmente)\s/i, () => { r.recorrencia = "semanal"; });
  tirar(/\s(todo m[eê]s|mensalmente)\s/i, () => { r.recorrencia = "mensal"; });
  tirar(/\sdepois de amanh[aã]\s/i, () => { r.data = somarDias(hoje, 2); });
  tirar(/\samanh[aã]\s/i, () => { r.data = somarDias(hoje, 1); });
  tirar(/\shoje\s/i, () => { r.data = hoje; });
  tirar(/\s(\d{1,2})\/(\d{1,2})(?:\/(\d{2,4}))?\s/, (m) => {
    const ano = m[3] ? (m[3].length === 2 ? 2000 + Number(m[3]) : Number(m[3])) : paraData(hoje).getFullYear();
    const d = new Date(ano, Number(m[2]) - 1, Number(m[1]));
    if (d.getDate() !== Number(m[1])) return false;
    // Sem ano e já passou: é do ano que vem.
    if (!m[3] && diaDe(d) < hoje) d.setFullYear(d.getFullYear() + 1);
    r.data = diaDe(d);
  });
  tirar(/\s(?:na |no |n[ao] próxim[ao] )?(domingo|segunda|ter[cç]a|quarta|quinta|sexta|s[aá]bado)(?:-feira)?\s/i, (m) => {
    const alvo = DIAS_DA_SEMANA[semAcento(m[1])] ?? DIAS_DA_SEMANA[m[1].toLowerCase()];
    const atual = paraData(hoje).getDay();
    r.data = somarDias(hoje, ((alvo - atual + 7) % 7) || 7);
  });
  tirar(/\s(?:[àa]s?\s)?(\d{1,2})(?::(\d{2})|h(\d{2})?)\s/i, (m) => {
    const h = Number(m[1]), min = Number(m[2] ?? m[3] ?? 0);
    if (h > 23 || min > 59) return false;
    r.hora = `${String(h).padStart(2, "0")}:${String(min).padStart(2, "0")}`;
    if (!r.data) r.data = hoje;
  });
  tirar(/\s!(urgente|alta|media|média|baixa)\s/i, (m) => { r.prioridade = semAcento(m[1]) as Prioridade; });
  tirar(/\s#([\p{L}\d_-]+)\s/u, (m) => {
    const alvo = semAcento(m[1]);
    const achada = CATEGORIAS.find((c) => c.id === alvo || semAcento(c.rotulo).startsWith(alvo) || (alvo === "medico" && c.id === "saude"));
    if (!achada) return false;
    r.categoria = achada.id;
  });
  tirar(/\s@([\p{L}\d_-]+)\s/u, (m) => {
    const alvo = semAcento(m[1]);
    const achada = pessoas.find((p) => semAcento(p.nome).split(/\s+/)[0] === alvo || semAcento(p.nome).startsWith(alvo) || p.id === m[1]);
    if (!achada) return false;
    r.responsavel = achada.id;
  });
  r.titulo = resto.replace(/\s+/g, " ").trim();
  return r;
}

export function progressoDoChecklist(t: Tarefa): { feitos: number; total: number } {
  return { feitos: t.checklist.filter((i) => i.feito).length, total: t.checklist.length };
}

/** "Hoje", "Amanhã", "Ontem", "qua, 22 out". */
export function rotuloDoDia(dia: string, hoje: string): string {
  if (!dia) return "Sem data";
  if (dia === hoje) return "Hoje";
  if (dia === somarDias(hoje, 1)) return "Amanhã";
  if (dia === somarDias(hoje, -1)) return "Ontem";
  return paraData(dia).toLocaleDateString("pt-BR", { weekday: "short", day: "numeric", month: "short" }).replace(/\./g, "");
}

export function slug(nome: string): string {
  return nome.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 40) || "eu";
}

export type { Status };
