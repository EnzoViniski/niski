/**
 * Fork (troca-de-contexto): na lista "Na tela" da lateral, embaixo do nome de cada agente, uma linha
 * com o estado e o essencial ("Aprovar: Bash · npm test", "Terminou: …") e quantas mensagens chegaram
 * desde a sua última olhada.
 *
 * Vem do que a conversa aberta publicou (`painelDaTroca`). Conversa que não está montada (outro
 * projeto, aba no Terminal) cai no estado da sessão, sem detalhe.
 */
import type { Conversation } from "../../mobile/api";
import type { TerminalSession } from "../terminal/terminalService";
import { estadoAgora, linhaEssencial, type Tom } from "./ondeParei";
import { useResumoNaTela } from "./painelDaTroca";
import "./troca.css";

/** O essencial pela sessão só (sem timeline): o rótulo do estado. */
export function essencialDaSessao(sessao: Pick<TerminalSession, "state" | "notice">): { tom: Tom; linha: string } | null {
  const estado = estadoAgora({ state: sessao.state, notice: sessao.notice ?? null, capabilities: null, live: true } as unknown as Conversation);
  if (estado.rotulo === "Sem estado ainda") return null;
  return { tom: estado.tom, linha: linhaEssencial(estado, null) };
}

/** `id`: o botão da aba aponta para cá (`aria-describedby`), e o nome dele continua só o do agente. */
export function EssencialNaTela({ sessao, id }: { sessao: TerminalSession; id?: string }) {
  const publicado = useResumoNaTela(sessao.conversation_id);
  const essencial = publicado ?? essencialDaSessao(sessao);
  if (!essencial) return null;
  const novas = publicado?.novas ?? 0;
  return (
    <span id={id} aria-hidden className="na-tela-essencial" data-tom={essencial.tom} title={essencial.linha}>
      {essencial.linha}
      {novas > 0 && <span className="sr-only">. {novas} {novas === 1 ? "mensagem nova" : "mensagens novas"} desde a sua última olhada</span>}
    </span>
  );
}

export function NovasNaTela({ sessao }: { sessao: TerminalSession }) {
  const novas = useResumoNaTela(sessao.conversation_id)?.novas ?? 0;
  if (novas <= 0) return null;
  const rotulo = `${novas} ${novas === 1 ? "mensagem nova" : "mensagens novas"} desde a sua última olhada`;
  return <span className="na-tela-novas" title={rotulo} aria-hidden>{novas > 99 ? "99+" : novas}</span>;
}
