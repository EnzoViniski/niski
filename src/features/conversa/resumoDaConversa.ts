/**
 * Fork (desempenho, 2026-10-22): o resumo de uma conversa (estado, capacidades, título, tokens) para
 * a faixa da aba de agente e o topo da Conversa no PC.
 *
 * Antes cada aba pedia a cada 3 s a lista de sessões e a timeline inteira (150 itens com textos e
 * diffs) só para ler o `conversation` que vem junto — e o objeto novo a cada leitura redesenhava a
 * conversa toda. Agora: `GET /conversas/{id}/resumo` com ETag (parado, volta 304 e nada muda na
 * tela). Engine antigo, sem a rota (404 sem corpo JSON): segue lendo a timeline, como antes.
 */
import { useEffect, useRef, useState } from "react";
import { ApiError, esquecerVersao, request, requestSeMudou, type Conversation, type Timeline } from "../../mobile/api";
import { usePoll } from "../../mobile/usePoll";

export const RITMO_DO_RESUMO_MS = 3_000;

/** 404 da rota que não existe (engine antigo) — diferente do 404 "Conversa não encontrada", que tem `error`. */
function rotaAusente(erro: unknown) {
  return erro instanceof ApiError && erro.status === 404 && !("error" in erro.detalhes);
}

export function useResumoDaConversa(conversaId: string | null): Conversation | null {
  const [conversa, setConversa] = useState<Conversation | null>(null);
  const instancia = useRef(Math.random().toString(36).slice(2));
  const semRota = useRef(false);
  const base = conversaId ? `/conversas/${encodeURIComponent(conversaId)}` : null;
  const chave = base ? `${base}/resumo#${instancia.current}` : null;
  useEffect(() => () => { if (chave) esquecerVersao(chave); }, [chave]);

  usePoll(async () => {
    if (!base || !chave) return;
    if (!semRota.current) {
      try {
        const resumo = await requestSeMudou<Conversation>(`${base}/resumo`, undefined, chave);
        if (resumo) setConversa(resumo);
        return;
      } catch (erro) {
        if (!rotaAusente(erro)) throw erro;
        semRota.current = true;
      }
    }
    const dados = await request<Timeline>(`${base}/timeline`);
    if (dados.conversation) setConversa(dados.conversation);
  }, () => undefined, [base, chave], RITMO_DO_RESUMO_MS);

  return conversa;
}
