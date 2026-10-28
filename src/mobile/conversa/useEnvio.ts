// Fork (Onda 2.0): o envio da conversa — balão otimista, fila da CLI, Idempotency-Key e If-Match —
// saiu do `MobileApp.tsx` sem mudança de lógica.
import { useCallback, useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import { request, actionKey, ApiError, type Conversation, type Mensagem, type Timeline } from "../api";
import { indiceDaMensagem } from "./useTimeline";

/** Prompt mandado daqui que ainda não apareceu no transcript. Some quando chega. */
export interface Enviado {
  texto: string;
  em: number;
  acao: string | null;
  /** Índice da última mensagem já vista no envio. Qualquer mensagem do usuário depois dela é esta. */
  base: number;
  anexos: { nome: string; previa: string | null }[];
}

/** Quanto esperar a mensagem enviada aparecer no transcript antes de desistir do balão otimista.
 *  Sem isto, uma mensagem que a CLI não aceitou deixava o "trabalhando" girando para sempre. */
const LIMITE_DE_CONFIRMACAO_MS = 120_000;

/** As rotas de ação da conversa que passam pela fila do engine (`POST /conversas/<id>/<tipo>`). */
export type TipoDeAcao = "prompt" | "aprovar" | "escolher" | "modo" | "ajustes" | "comando" | "barra"; // `barra` = `/skill` (lista do `/`).

/** `post` manda uma ação com a chave de idempotência e a versão da tela; `falhou` mostra o erro e
 *  decide se a chave pode ser reaproveitada no próximo clique. */
export function useAcoesDaConversa(conversation: Conversation, setError: Dispatch<SetStateAction<string | null>>) {
  const pending = useRef<{ signature: string; key: string; revision: string } | null>(null);

  async function post(kind: TipoDeAcao, body: Record<string, unknown>, revisao?: string) {
    const capability = conversation.capabilities!;
    const signature = JSON.stringify([kind, body]);
    if (pending.current?.signature !== signature) pending.current = { signature, key: actionKey(), revision: revisao ?? capability.revision };
    else if (revisao) pending.current.revision = revisao;
    const enviar = () => request<{ id?: string }>(`/conversas/${encodeURIComponent(conversation.id)}/${kind}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "Idempotency-Key": pending.current!.key, "If-Match": pending.current!.revision },
      body: JSON.stringify(body),
    });
    let acao: { id?: string };
    try {
      acao = await enviar();
    } catch (reason) {
      // Modo e ajustes levam a um estado absoluto ("fique em plano"), não respondem a um diálogo:
      // tela redesenhada desde a última leitura (o próprio rodapé do modo muda a cada troca) não
      // os torna perigosos. Relê a versão da tela e tenta uma vez — antes isso virava erro e
      // exigia um segundo clique. Prompt e aprovação continuam recusando: ali o If-Match protege.
      // Escolha também, desde que a pergunta na tela seja a mesma: marcar uma caixa redesenha o menu
      // e o clique seguinte chegava com a versão velha e sumia. Pergunta trocada continua recusando.
      // Na escolha, 409 (menu não lido naquele quadro, a TUI no meio de um redesenho) também relê.
      const relerEscolha = kind === "escolher" && (reason instanceof ApiError && reason.status === 409);
      if (!relerEscolha && !(reason instanceof ApiError && reason.status === 412 && (kind === "modo" || kind === "ajustes" || kind === "escolher"))) throw reason;
      if (relerEscolha) await new Promise((r) => setTimeout(r, 400));
      const atual = await request<Timeline>(`/conversas/${encodeURIComponent(conversation.id)}/timeline`);
      const revisao = atual.conversation?.capabilities?.revision;
      if (!revisao) throw reason;
      if (kind === "escolher" && atual.conversation?.capabilities?.escolha?.pergunta !== capability.escolha?.pergunta) throw reason;
      pending.current = { signature, key: actionKey(), revision: revisao };
      acao = await enviar();
    }
    pending.current = null;
    return acao;
  }

  function falhou(reason: unknown) {
    // A network timeout has an unknown outcome: reuse the key on retry, even after refresh.
    if (reason instanceof ApiError && reason.status >= 400 && reason.status < 500) pending.current = null;
    setError(String(reason));
  }

  return { post, falhou };
}

/** Tira o balão otimista e os itens da fila quando a mensagem chega ao transcript (ou é recusada),
 *  com um relógio próprio para não deixar o "trabalhando" girando sem rede. */
export function useConfirmacaoDoEnvio({ data, mensagens, enviado, setEnviado, fila, setFila, setFalhaEnvio }: {
  data: Timeline | null;
  mensagens: Mensagem[];
  enviado: Enviado | null;
  setEnviado: Dispatch<SetStateAction<Enviado | null>>;
  fila: Enviado[];
  setFila: Dispatch<SetStateAction<Enviado[]>>;
  setFalhaEnvio: Dispatch<SetStateAction<string | null>>;
}) {
  /** Tira o balão otimista da tela, devolvendo as prévias locais que ele segurava. */
  const descartarEnviado = useCallback((motivo: string | null) => {
    setEnviado(atual => {
      atual?.anexos.forEach(a => a.previa && URL.revokeObjectURL(a.previa));
      return null;
    });
    if (motivo) setFalhaEnvio(motivo);
  }, []);

  // O prompt otimista sai quando chega de verdade no transcript, ou quando a fila o recusa.
  useEffect(() => {
    if (!enviado) return;
    const chegou = mensagens.some(m => m.role === "user" && (m.kind ?? "text") === "text"
      && (m.text.trim() === enviado.texto.trim() || indiceDaMensagem(m.id) > enviado.base));
    const acao = data?.actions.find(a => a.id === enviado.acao);
    if (chegou) { descartarEnviado(null); return; }
    if (acao?.state === "rejected") descartarEnviado(`Não foi enviado: ${acao.error ?? "o agente recusou a entrada"}`);
  }, [data, enviado, mensagens, descartarEnviado]);

  // Item da fila sai quando a mensagem aparece no transcript (a CLI a consumiu) ou é recusado.
  useEffect(() => {
    if (fila.length === 0) return;
    const restantes = fila.filter((item) => {
      const chegou = mensagens.some(m => m.role === "user" && (m.kind ?? "text") === "text"
        && indiceDaMensagem(m.id) > item.base && m.text.trim() === item.texto.trim());
      const acao = data?.actions.find(a => a.id === item.acao);
      if (acao?.state === "rejected") setFalhaEnvio(`Não entrou na fila: ${acao.error ?? "o agente recusou a entrada"}`);
      // Fila parada há muito tempo (o turno caiu, a sessão fechou): some em vez de mentir.
      return !chegou && acao?.state !== "rejected" && Date.now() - item.em < 30 * 60_000;
    });
    if (restantes.length !== fila.length) setFila(restantes);
  }, [data, fila, mensagens]);

  // Rede de segurança com relógio próprio: sem rede não há polling, e era justamente aí que o
  // "trabalhando" ficava girando para sempre.
  useEffect(() => {
    if (!enviado) return;
    const restante = Math.max(0, enviado.em + LIMITE_DE_CONFIRMACAO_MS - Date.now());
    const timer = window.setTimeout(
      () => descartarEnviado("Não consegui confirmar o envio desta mensagem. Veja no PC se ela chegou ao agente."),
      restante,
    );
    return () => window.clearTimeout(timer);
  }, [enviado, descartarEnviado]);
}
