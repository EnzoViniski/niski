/**
 * Fork (Tarefas): assistente de IA do painel. Cada mensagem vai à CLI oficial sem tela
 * (`roadmaps_ia`: Claude/Codex na conta escolhida, sem ferramentas, fora do histórico) com as
 * tarefas abertas e a conversa recente; a resposta volta em JSON com o texto e as ações
 * (criar, editar, concluir, reabrir, apagar), que o app aplica e deixa desfazer.
 * A conversa fica só neste computador (`medza-tarefas-chat`), nunca no log.
 */
import { perguntarJson, type PedidoDeIa } from "../roadmaps/ia";
import type { IaEscolhida } from "../roadmaps/tipos";
import { concluir, criarTarefa, diaDe, normalizarTarefa, reabrir } from "./dominio";
import { CATEGORIAS, PRIORIDADES, RECORRENCIAS, STATUS, type Pessoa, type Tarefa } from "./tipos";

export const CHAVE_DO_CHAT = "medza-tarefas-chat";
export const CHAVE_DO_PEDIDO = "tarefas-assistente";

export type Acao =
  | { tipo: "criar"; tarefa: Partial<Tarefa> & { titulo: string } }
  | { tipo: "editar"; id: string; campos: Partial<Tarefa> }
  | { tipo: "concluir" | "reabrir" | "apagar"; id: string };

export interface Feito { descricao: string; id: string; antes: Tarefa | null }

export interface MensagemDoChat {
  de: "eu" | "ia";
  texto: string;
  em: string;
  /** O que a IA mudou nessa resposta (para mostrar e desfazer). */
  feitos?: Feito[];
  desfeito?: boolean;
}

export interface ChatSalvo { mensagens: MensagemDoChat[]; ia: IaEscolhida }

export function lerChat(): ChatSalvo {
  try {
    const bruto = JSON.parse(localStorage.getItem(CHAVE_DO_CHAT) ?? "null") as Partial<ChatSalvo> | null;
    const ia = bruto?.ia && (bruto.ia.cli === "claude" || bruto.ia.cli === "codex") ? bruto.ia : { cli: "claude" as const };
    return { mensagens: Array.isArray(bruto?.mensagens) ? bruto.mensagens.slice(-60) : [], ia };
  } catch {
    return { mensagens: [], ia: { cli: "claude" } };
  }
}
export function gravarChat(chat: ChatSalvo) {
  try { localStorage.setItem(CHAVE_DO_CHAT, JSON.stringify({ ...chat, mensagens: chat.mensagens.slice(-60) })); } catch { /* sem localStorage */ }
}

const CAMPOS_EDITAVEIS = ["titulo", "descricao", "status", "prioridade", "categoria", "etiquetas", "responsavel", "data", "hora", "duracaoMin", "local", "recorrencia", "checklist"] as const;

export const SISTEMA = [
  "Você é o assistente de tarefas do Niski, um app de produtividade. Fala português do Brasil, curto e simpático.",
  "Ajude a pessoa a organizar o dia: criar, editar, concluir, reagendar e quebrar tarefas em passos. Pode sugerir prioridades e horários.",
  "Responda SEMPRE só com JSON, sem texto fora dele, no formato:",
  "{\"resposta\":\"texto para a pessoa\",\"acoes\":[...]}",
  "Ações possíveis (use a lista vazia se só estiver conversando ou se faltar informação — então pergunte):",
  "{\"tipo\":\"criar\",\"tarefa\":{\"titulo\":\"...\",\"descricao\":\"...\",\"data\":\"AAAA-MM-DD\",\"hora\":\"HH:MM\",\"duracaoMin\":30,\"categoria\":\"...\",\"prioridade\":\"...\",\"responsavel\":\"id\",\"recorrencia\":\"...\",\"local\":\"...\",\"etiquetas\":[\"...\"],\"checklist\":[{\"texto\":\"...\"}]}}",
  "{\"tipo\":\"editar\",\"id\":\"id da tarefa\",\"campos\":{...só os campos que mudam...}}",
  "{\"tipo\":\"concluir\",\"id\":\"...\"} | {\"tipo\":\"reabrir\",\"id\":\"...\"} | {\"tipo\":\"apagar\",\"id\":\"...\"}",
  `Valores válidos — status: ${STATUS.join(", ")}; prioridade: ${PRIORIDADES.join(", ")}; recorrencia: ${RECORRENCIAS.join(", ")}; categoria: ${CATEGORIAS.map((c) => `${c.id} (${c.rotulo})`).join(", ")}.`,
  "Campos que você não souber, deixe de fora. Datas relativas (amanhã, sexta) viram AAAA-MM-DD a partir de hoje. Use só ids de tarefas e pessoas que aparecem no contexto.",
  "Apague só quando a pessoa pedir claramente. Na resposta, diga em uma frase o que fez.",
].join("\n");

function linhaDaTarefa(t: Tarefa) {
  const partes = [`id=${t.id}`, `"${t.titulo}"`, t.status, t.data && `data=${t.data}`, t.hora && `hora=${t.hora}`, `cat=${t.categoria}`, `prio=${t.prioridade}`, t.responsavel && `resp=${t.responsavel}`, t.recorrencia !== "nunca" && `repete=${t.recorrencia}`];
  return `- ${partes.filter(Boolean).join(" | ")}`;
}

/** O texto que vai para a IA: data de hoje, pessoas, tarefas relevantes e a conversa recente. */
export function montarPedido(pergunta: string, tarefas: Tarefa[], pessoas: Pessoa[], eu: Pessoa, historico: MensagemDoChat[], agora = new Date()): string {
  const hoje = diaDe(agora);
  const relevantes = tarefas.filter((t) => !t.apagada && (t.status !== "feito" || (t.concluidaEm && diaDe(new Date(t.concluidaEm)) === hoje)))
    .sort((a, b) => (a.data || "9999").localeCompare(b.data || "9999")).slice(0, 120);
  const conversa = historico.slice(-10).map((m) => `${m.de === "eu" ? "Pessoa" : "Assistente"}: ${m.texto}`).join("\n");
  return [
    `Agora: ${agora.toLocaleDateString("pt-BR", { weekday: "long" })}, ${hoje} ${agora.toTimeString().slice(0, 5)}.`,
    `Quem fala com você: ${eu.nome} (id=${eu.id}).`,
    `Pessoas: ${pessoas.map((p) => `${p.nome} (id=${p.id})`).join(", ") || eu.nome}.`,
    `Tarefas abertas e feitas hoje (${relevantes.length}):`,
    relevantes.map(linhaDaTarefa).join("\n") || "(nenhuma)",
    conversa ? `\nConversa até aqui:\n${conversa}` : "",
    `\nPessoa: ${pergunta}`,
  ].join("\n");
}

const texto = (v: unknown) => (typeof v === "string" ? v : "");

/** Confere o JSON da IA. Devolve a resposta pronta ou o que estava errado (para pedir correção). */
export function lerResposta(valor: unknown): { resposta: string; acoes: Acao[] } | string {
  const v = valor as { resposta?: unknown; acoes?: unknown } | null;
  if (!v || typeof v.resposta !== "string") return "faltou o campo \"resposta\" (texto)";
  const acoes: Acao[] = [];
  for (const bruta of Array.isArray(v.acoes) ? v.acoes : []) {
    const a = bruta as Record<string, unknown>;
    if (a.tipo === "criar" && a.tarefa && typeof a.tarefa === "object" && texto((a.tarefa as Record<string, unknown>).titulo).trim()) {
      acoes.push({ tipo: "criar", tarefa: a.tarefa as Partial<Tarefa> & { titulo: string } });
    } else if (a.tipo === "editar" && typeof a.id === "string" && a.campos && typeof a.campos === "object") {
      acoes.push({ tipo: "editar", id: a.id, campos: a.campos as Partial<Tarefa> });
    } else if ((a.tipo === "concluir" || a.tipo === "reabrir" || a.tipo === "apagar") && typeof a.id === "string") {
      acoes.push({ tipo: a.tipo, id: a.id });
    }
  }
  return { resposta: v.resposta.trim(), acoes };
}

function soEditaveis(campos: Record<string, unknown>): Partial<Tarefa> {
  const saida: Record<string, unknown> = {};
  for (const c of CAMPOS_EDITAVEIS) if (c in campos) saida[c] = campos[c];
  if (Array.isArray(saida.checklist)) {
    saida.checklist = (saida.checklist as unknown[]).map((i, n) => {
      const item = (typeof i === "string" ? { texto: i } : i) as Record<string, unknown>;
      return { id: typeof item.id === "string" ? item.id : `i-${Date.now().toString(36)}-${n}`, texto: texto(item.texto), feito: item.feito === true };
    });
  }
  return saida as Partial<Tarefa>;
}

/**
 * Aplica as ações: devolve as tarefas a gravar e o registro para desfazer. Tarefa desconhecida é
 * ignorada; campo inválido cai no padrão (`normalizarTarefa`).
 */
export function aplicarAcoes(acoes: Acao[], tarefas: Tarefa[], eu: string, agora = new Date()): { gravar: Tarefa[]; feitos: Feito[] } {
  const porId = new Map(tarefas.map((t) => [t.id, t]));
  const gravar: Tarefa[] = [];
  const feitos: Feito[] = [];
  for (const a of acoes) {
    if (a.tipo === "criar") {
      const base = criarTarefa({ titulo: a.tarefa.titulo.trim() }, eu, agora);
      const nova = normalizarTarefa({ ...base, ...soEditaveis(a.tarefa as Record<string, unknown>), id: base.id, criadaPor: eu, criadaEm: base.criadaEm });
      if (!nova) continue;
      gravar.push(nova); porId.set(nova.id, nova);
      feitos.push({ descricao: `Criada: ${nova.titulo}`, id: nova.id, antes: null });
      continue;
    }
    const atual = porId.get(a.id);
    if (!atual || atual.apagada) continue;
    let nova: Tarefa | null = null;
    let descricao = "";
    if (a.tipo === "editar") { nova = normalizarTarefa({ ...atual, ...soEditaveis(a.campos as Record<string, unknown>) }); descricao = `Alterada: ${atual.titulo}`; }
    if (a.tipo === "concluir") { nova = concluir(atual, agora); descricao = `Concluída: ${atual.titulo}`; }
    if (a.tipo === "reabrir") { nova = reabrir(atual, agora); descricao = `Reaberta: ${atual.titulo}`; }
    if (a.tipo === "apagar") { nova = { ...atual, apagada: true }; descricao = `Apagada: ${atual.titulo}`; }
    if (!nova) continue;
    gravar.push(nova); porId.set(nova.id, nova);
    feitos.push({ descricao, id: atual.id, antes: atual });
  }
  return { gravar, feitos };
}

/** Desfazer uma resposta: criadas são apagadas, as outras voltam ao que eram. */
export function desfazerFeitos(feitos: Feito[], tarefas: Tarefa[]): Tarefa[] {
  return feitos.slice().reverse().flatMap((f) => {
    if (f.antes) return [f.antes];
    const criada = tarefas.find((t) => t.id === f.id);
    return criada ? [{ ...criada, apagada: true }] : [];
  });
}

export function perguntarAoAssistente(ia: IaEscolhida, textoDoPedido: string, perguntar: typeof perguntarJson = perguntarJson) {
  const pedido: PedidoDeIa = { chave: CHAVE_DO_PEDIDO, ia, sistema: SISTEMA, texto: textoDoPedido, web: false };
  return perguntar(pedido, lerResposta);
}
