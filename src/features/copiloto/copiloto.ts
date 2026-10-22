/**
 * Fork (Copiloto): estado da janela do copiloto — a conversa (sessão do engine), a pasta onde ele
 * roda, o modo (flutuante ou presa à direita), posição e tamanho. Fica em `medza-copiloto`
 * (espelhado em `estado-da-interface.json`).
 *
 * O copiloto é uma conversa normal do app (Claude ou Codex) no projeto `copiloto`: o engine a marca
 * como orquestradora ao abrir (`marcar_copiloto`), então ela já nasce com as ferramentas do app —
 * criar e comandar agentes de qualquer conta e provedor, e criar/editar/concluir tarefas.
 */
import { useSyncExternalStore } from "react";
import { listTerminalSessions, stopTerminal } from "../terminal/terminalService";

export const CHAVE_DO_COPILOTO = "medza-copiloto";
/** Mesmo valor de `PROJETO_DO_COPILOTO` no engine (`mobile/orquestracao.rs`). */
export const PROJETO_DO_COPILOTO = "copiloto";

export type ModoDoCopiloto = "flutuante" | "direita";
export interface Retangulo { x: number; y: number; w: number; h: number }

export interface EstadoDoCopiloto {
  aberto: boolean;
  modo: ModoDoCopiloto;
  /** Sessão do engine da conversa atual; vazio = mostra o lançador (escolher agente/conta/modelo). */
  sessao: string;
  provider: string;
  /** Pasta em que o agente roda; vazio = pasta do usuário. */
  pasta: string;
  janela: Retangulo | null;
  larguraDireita: number;
  /** Agentes que o copiloto cria abrem o fluxo ao vivo no painel. */
  abrirAgentesNoPainel: boolean;
}

const PADRAO: EstadoDoCopiloto = {
  aberto: false, modo: "flutuante", sessao: "", provider: "", pasta: "", janela: null, larguraDireita: 440, abrirAgentesNoPainel: true,
};

function retangulo(v: unknown): Retangulo | null {
  const r = v as Partial<Retangulo> | null;
  return r && [r.x, r.y, r.w, r.h].every((n) => typeof n === "number" && Number.isFinite(n)) ? (r as Retangulo) : null;
}

function ler(): EstadoDoCopiloto {
  try {
    const s = JSON.parse(localStorage.getItem(CHAVE_DO_COPILOTO) ?? "null") as Partial<EstadoDoCopiloto> | null;
    if (!s || typeof s !== "object") return { ...PADRAO };
    return {
      aberto: s.aberto === true,
      modo: s.modo === "direita" ? "direita" : "flutuante",
      sessao: typeof s.sessao === "string" ? s.sessao : "",
      provider: typeof s.provider === "string" ? s.provider : "",
      pasta: typeof s.pasta === "string" ? s.pasta : "",
      janela: retangulo(s.janela),
      larguraDireita: typeof s.larguraDireita === "number" ? Math.min(900, Math.max(340, s.larguraDireita)) : PADRAO.larguraDireita,
      abrirAgentesNoPainel: s.abrirAgentesNoPainel !== false,
    };
  } catch {
    return { ...PADRAO };
  }
}

let estado = ler();
const ouvintes = new Set<() => void>();

export function mudarCopiloto(mudanca: Partial<EstadoDoCopiloto>) {
  estado = { ...estado, ...mudanca };
  try { localStorage.setItem(CHAVE_DO_COPILOTO, JSON.stringify(estado)); } catch { /* sem localStorage */ }
  ouvintes.forEach((o) => o());
}

export function useCopiloto(): EstadoDoCopiloto {
  return useSyncExternalStore((o) => { ouvintes.add(o); return () => ouvintes.delete(o); }, () => estado);
}

export const alternarCopiloto = () => mudarCopiloto({ aberto: !estado.aberto });
export const abrirCopiloto = () => mudarCopiloto({ aberto: true });
/**
 * Começa outra conversa (a atual continua no Histórico). `mudanca` vai junto (ex.: pasta nova).
 *
 * Fork (revisão do copiloto): a conversa anterior é encerrada — antes ela seguia rodando escondida,
 * já que o copiloto fica fora do aviso de "agentes fora do workspace". Se ela estiver trabalhando
 * (a paleta não bloqueia como o botão), não é interrompida.
 */
export function novaConversaDoCopiloto(mudanca: Partial<EstadoDoCopiloto> = {}) {
  const anterior = estado.sessao;
  mudarCopiloto({ ...mudanca, sessao: "", provider: "" });
  if (!anterior) return Promise.resolve();
  return listTerminalSessions()
    .then((sessoes) => sessoes.find((s) => s.id === anterior))
    .then((s) => (s && s.state !== "working" ? stopTerminal(anterior) : undefined))
    .catch(() => undefined);
}

export const ehConversaDoCopiloto = (projectId: string | null | undefined) => projectId === PROJETO_DO_COPILOTO;

/** Janela flutuante padrão: canto inferior direito, à esquerda dos botões (não cobre o de Tarefas). */
export function janelaPadrao(): Retangulo {
  const w = Math.min(460, window.innerWidth - 120), h = Math.min(700, window.innerHeight - 48);
  return { x: window.innerWidth - w - 92, y: window.innerHeight - h - 24, w, h };
}

/** Só para testes. */
export function reiniciarCopilotoParaTeste() { estado = ler(); }
