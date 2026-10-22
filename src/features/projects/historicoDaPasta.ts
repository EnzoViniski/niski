import { useEffect, useSyncExternalStore } from "react";
import { listHistory, samePath, type HistoryEntry } from "../history/historyService";
import { listConversations, type Conversation, type TerminalSession } from "../terminal/terminalService";

/*
 * Fork (histórico na lateral): a pasta do projeto mostrava só as conversas com agente aberto; fechar
 * uma conversa (ou o engine reiniciar) tirava ela da lista, e as antigas só existiam na tela
 * Histórico. Como nos apps do Claude Code e do Codex, cada pasta agora lista também as conversas
 * anteriores daquela pasta, lidas dos transcripts das CLIs (todas as contas cadastradas), e um
 * clique retoma com `--resume` (`retomarDoHistorico`).
 *
 * Um estado só para todas as pastas: uma leitura ao abrir, de novo a cada minuto e quando a janela
 * volta ao foco. Falha (fora do Tauri, engine fora do ar) vira lista vazia, sem erro na tela.
 */

export const INTERVALO_DA_RELEITURA_MS = 60_000;

export interface ConversaAnterior {
  entry: HistoryEntry;
  titulo: string;
}

interface Estado {
  entradas: HistoryEntry[];
  /** Título do índice do app por id de sessão da CLI (renomear no app vale aqui também). */
  titulos: Record<string, string>;
}

let estado: Estado = { entradas: [], titulos: {} };
const ouvintes = new Set<() => void>();
let usuarios = 0;
let relogio: number | null = null;
let lendo: Promise<void> | null = null;

function avisar() {
  ouvintes.forEach((ouvinte) => ouvinte());
}

export function titulosDoIndice(conversas: Conversation[]): Record<string, string> {
  const titulos: Record<string, string> = {};
  for (const conversa of conversas) {
    for (const segmento of conversa.segments ?? []) {
      if (segmento.external_session_id && conversa.title?.trim()) titulos[segmento.external_session_id] = conversa.title.trim();
    }
  }
  return titulos;
}

/** Relê os transcripts. Chamadas juntas viram uma leitura só. */
export function relerConversasAnteriores(): Promise<void> {
  if (lendo) return lendo;
  lendo = (async () => {
    const [claude, codex, indice] = await Promise.all([
      listHistory("claude").catch(() => [] as HistoryEntry[]),
      listHistory("codex").catch(() => [] as HistoryEntry[]),
      listConversations().catch(() => [] as Conversation[]),
    ]);
    const novo = { entradas: [...claude, ...codex], titulos: titulosDoIndice(indice) };
    lidoEm = Date.now();
    // Fork (desempenho): lista igual não redesenha a lateral.
    if (JSON.stringify(novo) === JSON.stringify(estado)) return;
    estado = novo;
    avisar();
  })().finally(() => { lendo = null; });
  return lendo;
}

/** Fork (desempenho): trocar de janela várias vezes seguidas não revarre os transcripts a cada volta. */
export const FOCO_RELE_DEPOIS_DE_MS = 15_000;
let lidoEm = 0;

function aoFocar() {
  if (Date.now() - lidoEm < FOCO_RELE_DEPOIS_DE_MS) return;
  void relerConversasAnteriores();
}

/** Liga a releitura enquanto a lateral estiver na tela. */
function usar(): () => void {
  usuarios += 1;
  if (usuarios === 1) {
    void relerConversasAnteriores();
    relogio = window.setInterval(aoFocar, INTERVALO_DA_RELEITURA_MS);
    window.addEventListener("focus", aoFocar);
  }
  return () => {
    usuarios = Math.max(0, usuarios - 1);
    if (usuarios === 0) {
      if (relogio !== null) window.clearInterval(relogio);
      relogio = null;
      window.removeEventListener("focus", aoFocar);
    }
  };
}

/** Só para testes. */
export function redefinirConversasAnteriores(novo: Estado = { entradas: [], titulos: {} }) {
  lidoEm = 0;
  estado = novo;
  avisar();
}

export function useConversasAnteriores(): Estado {
  useEffect(() => usar(), []);
  return useSyncExternalStore(
    (ouvinte) => { ouvintes.add(ouvinte); return () => ouvintes.delete(ouvinte); },
    () => estado,
  );
}

function resumir(texto: string, limite = 80): string {
  const linha = texto.replace(/\s+/g, " ").trim();
  return linha.length > limite ? `${linha.slice(0, limite - 1)}…` : linha;
}

/**
 * Conversas anteriores de uma pasta, mais recentes primeiro: sem as que já estão abertas na lateral
 * (mesmo id de sessão da CLI) e sem transcript vazio (CLI aberta e fechada sem mensagem).
 * Uma mesma sessão em duas contas (cópia feita ao retomar noutra conta) aparece uma vez só.
 */
export function anterioresDaPasta(
  pasta: string,
  { entradas, titulos }: Estado,
  abertas: TerminalSession[],
): ConversaAnterior[] {
  const idsAbertos = new Set(abertas.map((sessao) => sessao.external_session_id).filter(Boolean));
  const vistos = new Set<string>();
  return entradas
    .filter((entry) => samePath(entry.cwd, pasta) && !idsAbertos.has(entry.session_id) && Boolean(entry.title || entry.first_prompt))
    .sort((a, b) => b.updated_at_ms - a.updated_at_ms)
    .filter((entry) => {
      const chave = `${entry.provider}:${entry.session_id}`;
      if (vistos.has(chave)) return false;
      vistos.add(chave);
      return true;
    })
    .map((entry) => ({ entry, titulo: resumir(titulos[entry.session_id] ?? entry.title ?? entry.first_prompt ?? "Conversa") }));
}
