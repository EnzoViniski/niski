import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { ApiError, actionKey, request, type Timeline } from "../../mobile/api";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { SendIcon, StopIcon } from "../../components/ui/PixelIcon";
import { pararAgente, type FluxoAoVivo, type NoAoVivo } from "./orquestracaoService";

/**
 * Fork: interagir pelo fluxo do orquestrador — mandar prompt para qualquer nó ("Para: …"), aprovar ou
 * negar um pedido de permissão e parar um subagente. Tudo pelas rotas que o chat já usa
 * (`/conversas/{id}/prompt|aprovar`) e pela de parar do orquestrador.
 *
 * Aprovar usa a versão da tela que o nó mostrou (`If-Match`): se a tela mudou desde então, o engine
 * recusa (412) e o fluxo relê, para ninguém aprovar algo que não viu. O prompt pega a versão na hora
 * do envio, como o chat.
 */
export interface AcoesDoFluxo {
  erro: string | null;
  ocupado: string | null;
  aprovar: (no: NoAoVivo, permitir: boolean) => Promise<void>;
  parar: (orquestrador: string, no: NoAoVivo) => Promise<void>;
  enviar: (no: NoAoVivo, texto: string) => Promise<boolean>;
}

const json = { "Content-Type": "application/json" };
const conversa = (id: string) => `/conversas/${encodeURIComponent(id)}`;

function mensagem(motivo: unknown) {
  if (motivo instanceof ApiError && motivo.status === 412) return "A tela do agente mudou; confira o pedido de novo.";
  return motivo instanceof Error ? motivo.message : String(motivo);
}

export function useAcoesDoFluxo(recarregar: () => Promise<void>): AcoesDoFluxo {
  const [erro, setErro] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState<string | null>(null);
  const executar = useCallback(async (id: string, acao: () => Promise<unknown>) => {
    setOcupado(id);
    setErro(null);
    try {
      await acao();
      return true;
    } catch (motivo) {
      setErro(mensagem(motivo));
      return false;
    } finally {
      setOcupado(null);
      void recarregar().catch(() => undefined);
    }
  }, [recarregar]);

  return useMemo(() => ({
    erro, ocupado,
    aprovar: async (no, permitir) => {
      const revisao = no.capabilities?.revision;
      if (!revisao) return;
      await executar(no.id, () => request(`${conversa(no.id)}/aprovar`, {
        method: "POST", headers: { ...json, "Idempotency-Key": actionKey(), "If-Match": revisao }, body: JSON.stringify({ permitir }),
      }));
    },
    parar: async (orquestrador, no) => { await executar(no.id, () => pararAgente(orquestrador, no.id)); },
    enviar: (no, texto) => executar(no.id, async () => {
      const atual = (await request<Timeline>(`${conversa(no.id)}/timeline`)).conversation?.capabilities;
      if (!atual || (!atual.prompt && !atual.enfileirar)) {
        throw new Error(atual?.approve ? "Responda o pedido de permissão antes." : (atual?.reason ?? "Este agente não pode receber mensagem agora."));
      }
      await request(`${conversa(no.id)}/prompt`, {
        method: "POST", headers: { ...json, "Idempotency-Key": actionKey(), "If-Match": atual.revision },
        body: JSON.stringify({ texto, fila: !atual.prompt }),
      });
    }),
  }), [erro, ocupado, executar]);
}

export const AcoesContext = createContext<AcoesDoFluxo | null>(null);

/** Dentro do nó: aprovar/negar o pedido na tela e parar o subagente. */
export function AcoesDoNo({ no, orquestrador, centro }: { no: NoAoVivo; orquestrador: string; centro: boolean }) {
  const acoes = useContext(AcoesContext);
  const [confirmar, setConfirmar] = useState(false);
  useEffect(() => {
    if (!confirmar) return;
    const t = window.setTimeout(() => setConfirmar(false), 4_000);
    return () => window.clearTimeout(t);
  }, [confirmar]);
  if (!acoes) return null;
  const ocupado = acoes.ocupado === no.id;
  const pedido = no.capabilities?.approve ? no.capabilities.approval_text : null;
  const vivo = no.estado !== "parado" && no.estado !== "erro";
  return (
    <>
      {no.capabilities?.approve && (
        <div className="orq-pedido nodrag">
          {no.arriscado && <div className="orq-pedido-alerta">Só você aprova: {no.arriscado}</div>}
          <div className="orq-pedido-texto" title={pedido ?? undefined}>{pedido ?? "Pedido de permissão"}</div>
          <div className="orq-pedido-botoes">
            <button type="button" className="orq-botao is-sim" disabled={ocupado} onClick={() => void acoes.aprovar(no, true)}>Permitir</button>
            <button type="button" className="orq-botao is-nao" disabled={ocupado} onClick={() => void acoes.aprovar(no, false)}>Negar</button>
          </div>
        </div>
      )}
      {!no.capabilities?.approve && no.capabilities?.escolha && (
        <div className="orq-pedido nodrag"><div className="orq-pedido-texto">{no.capabilities.escolha.pergunta}</div>
          <div className="orq-pedido-dica">Clique no nó para responder na conversa.</div></div>
      )}
      {!centro && vivo && (
        <div className="orq-no-rodape nodrag">
          <button type="button" className="orq-botao is-discreto" disabled={ocupado} title="Encerra a CLI deste subagente"
            onClick={() => (confirmar ? (setConfirmar(false), void acoes.parar(orquestrador, no)) : setConfirmar(true))}>
            <StopIcon aria-hidden className="h-3 w-3" />{confirmar ? "Parar mesmo?" : "Parar"}
          </button>
        </div>
      )}
    </>
  );
}

function nomeDoNo(no: NoAoVivo, centro: boolean) {
  return `${centro ? "Orquestrador" : (no.tarefa ?? no.titulo ?? "Subagente")}`.slice(0, 60);
}

/** Embaixo do fluxo: "Para: [orquestrador ▾]" e a mensagem. Enter envia; Shift+Enter quebra linha. */
export function ParaQuem({ fluxo, acoes }: { fluxo: FluxoAoVivo; acoes: AcoesDoFluxo }) {
  const [destino, setDestino] = useState(fluxo.orquestrador.id);
  const [texto, setTexto] = useState("");
  const nos = [fluxo.orquestrador, ...fluxo.agentes];
  const alvo = nos.find((no) => no.id === destino) ?? fluxo.orquestrador;
  const parado = alvo.estado === "parado";
  const vazio = texto.trim().length === 0;

  async function mandar() {
    if (vazio || parado) return;
    if (await acoes.enviar(alvo, texto.trim())) setTexto("");
  }

  return (
    <form className="orq-composer" onSubmit={(e) => { e.preventDefault(); void mandar(); }}>
      <label className="orq-para">
        Para:
        <span className="orq-para-marca"><AgentIcon provider={alvo.plataforma} size={13} /></span>
        <select aria-label="Para quem" value={alvo.id} onChange={(e) => setDestino(e.target.value)}>
          {nos.map((no, i) => (
            <option key={no.id} value={no.id}>{nomeDoNo(no, i === 0)}{no.estado === "parado" ? " (parado)" : ""}</option>
          ))}
        </select>
      </label>
      <textarea aria-label="Mensagem" rows={1} value={texto} placeholder={parado ? "Este agente está parado" : `Mensagem para ${nomeDoNo(alvo, alvo.id === fluxo.orquestrador.id).toLowerCase()}`}
        disabled={parado} onChange={(e) => setTexto(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); void mandar(); } }} />
      <button type="submit" className="orq-botao is-destaque" disabled={vazio || parado || acoes.ocupado === alvo.id} aria-label="Enviar">
        <SendIcon aria-hidden className="h-3.5 w-3.5" />Enviar
      </button>
    </form>
  );
}
