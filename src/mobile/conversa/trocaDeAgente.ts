// Fork (trocar agente): as duas chamadas do "Continuar com…" (`/conversas/{id}/trocar-agente` no engine).
// Mesma rota no PC (transporte do engine), na web e no celular.
import { request } from "../api";

export interface AgenteParaTrocar {
  id: string;
  label: string;
  /** A visão Conversa lê este agente (transcript + tela). O Cursor troca, mas segue no terminal. */
  conversa: boolean;
  profiles: { id: string; name: string }[];
}

/** `GET /conversas/{id}/trocar-agente`. */
export interface OpcoesDeTroca {
  provider: string | null;
  profile_id: string | null;
  agentes: AgenteParaTrocar[];
  /** `false` com o agente trabalhando ou esperando resposta: `motivo` diz o quê. */
  pode_trocar: boolean;
  motivo: string | null;
  live: boolean;
}

export interface PedidoDeTroca {
  provider: string;
  profile_id?: string | null;
  /** O que já está escrito no campo: vai junto, depois da passagem de bastão. */
  mensagem?: string | null;
}

/** `POST /conversas/{id}/trocar-agente`. */
export interface ResultadoDaTroca {
  conversation_id: string;
  terminal_session_id: string | null;
  /** `conta`: mesmo agente, continuação literal. `passagem`: outro agente, com o arquivo de passagem. */
  modo: "conta" | "passagem";
  provider: string;
  profile_id: string | null;
  /** Caminho absoluto do arquivo de passagem (só no modo `passagem`). */
  passagem: string | null;
  relativo?: string;
  /** A mensagem do campo foi junto (o campo pode ser limpo). */
  mensagem_enviada: boolean;
  /** `false` no Cursor: a conversa segue no terminal. */
  conversa_no_chat?: boolean;
}

const rota = (conversaId: string) => `/conversas/${encodeURIComponent(conversaId)}/trocar-agente`;

export function lerOpcoesDeTroca(conversaId: string): Promise<OpcoesDeTroca> {
  return request<OpcoesDeTroca>(rota(conversaId));
}

/** Troca o agente (ou a conta) da conversa sem sair dela. Leva alguns segundos: grava a passagem, fecha a
 *  CLI anterior e abre a nova na mesma aba. */
export function trocarAgente(conversaId: string, pedido: PedidoDeTroca): Promise<ResultadoDaTroca> {
  return request<ResultadoDaTroca>(rota(conversaId), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ provider: pedido.provider, profile_id: pedido.profile_id ?? null, mensagem: pedido.mensagem?.trim() || null }),
    // Gerar a passagem lê os transcripts inteiros e reabre a CLI: mais que os 15 s padrão.
    signal: AbortSignal.timeout(60_000),
  });
}

export interface Destino { provider: string; label: string; profileId: string | null; conta: string | null; conversa: boolean }

/** Para onde dá para ir: cada conta de cada agente instalado (agente sem conta cadastrada vira uma opção
 *  só), menos o agente+conta atual. */
export function destinosDaTroca(opcoes: OpcoesDeTroca): Destino[] {
  return opcoes.agentes.flatMap((agente) => {
    const contas = agente.profiles.length ? agente.profiles.map((p) => ({ profileId: p.id, conta: p.name })) : [{ profileId: null, conta: null }];
    return contas
      .filter((c) => !(agente.id === opcoes.provider && (c.profileId === opcoes.profile_id || agente.profiles.length <= 1)))
      .map((c) => ({ provider: agente.id, label: agente.label, conversa: agente.conversa, ...c }));
  });
}
