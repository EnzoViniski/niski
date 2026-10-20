import { useRef, useState } from "react";
import "./revisaoPorAgente.css";
import { actionKey } from "../../mobile/api";
import { mandarAchados, revisarComAgente, type PedidoDeRevisao, type RevisaoCriada } from "./revisaoPorAgenteService";

// Metadados por origem: anexar o revisor troca a aba e desmonta este painel.
function ultimaRevisao(sessionId?: string): RevisaoCriada | null {
  if (!sessionId) return null;
  try {
    const valor = JSON.parse(sessionStorage.getItem(`medza:revisao:${sessionId}`) ?? "null") as RevisaoCriada | null;
    return valor && [valor.id, valor.session_id, valor.name, valor.origem].every(v => typeof v === "string" && v.length > 0) ? valor : null;
  } catch { return null; }
}

/** Fork: o agente autor mantém seu contexto; a revisão abre uma conversa irmã visível no fluxo. */
export function RevisarComAgente({ sessionId, onAbrirSessao }: { sessionId?: string; onAbrirSessao?: (id: string, titulo: string) => void }) {
  const [aberto, setAberto] = useState(false);
  const [pedido, setPedido] = useState<PedidoDeRevisao>({ provider: "claude", escopo: "local" });
  const [revisao, setRevisao] = useState<RevisaoCriada | null>(() => ultimaRevisao(sessionId));
  const [ocupado, setOcupado] = useState(false);
  const [erro, setErro] = useState("");
  const [aviso, setAviso] = useState("");
  const chave = useRef({ pedido: "", chave: "" });
  const chaveAchados = useRef(actionKey());
  async function iniciar() {
    if (!sessionId) return;
    setOcupado(true); setErro(""); setAviso("");
    const json = JSON.stringify(pedido);
    if (chave.current.pedido !== json) chave.current = { pedido: json, chave: actionKey() };
    try {
      const nova = await revisarComAgente(sessionId, pedido, chave.current.chave);
      setRevisao(nova); setAberto(false); chaveAchados.current = actionKey();
      try { sessionStorage.setItem(`medza:revisao:${sessionId}`,JSON.stringify(nova)); } catch { /* sessão atual continua aberta */ }
      onAbrirSessao?.(nova.session_id,nova.name);
      setAviso("Revisor aberto em conversa separada, vinculado à origem no Fluxo.");
    } catch (e) { setErro(e instanceof Error ? e.message : String(e)); }
    finally { setOcupado(false); }
  }
  return <div className="revisao-agente revisao-acoes">
    <button disabled={!sessionId || ocupado} onClick={() => { setAberto(true); setErro(""); chave.current = { pedido:"", chave:"" }; }}>Revisar com agente</button>
    {revisao && <>
      <button onClick={() => onAbrirSessao?.(revisao.session_id,revisao.name)}>Abrir conversa da revisão</button>
      <button disabled={ocupado} onClick={() => {
        if (!sessionId) return;
        setOcupado(true); setErro("");
        void mandarAchados(sessionId,revisao,chaveAchados.current).then(() => setAviso("Achados enviados ao agente de origem.")).catch(e => setErro(String(e instanceof Error ? e.message : e))).finally(() => setOcupado(false));
      }}>Mandar achados para a origem</button>
    </>}
    {aberto && <section role="dialog" aria-label="Revisar com agente" className="revisao-comentar">
      <label>CLI da revisão <select aria-label="CLI da revisão" value={pedido.provider} disabled={ocupado} onChange={e => setPedido({ ...pedido,provider:e.target.value as PedidoDeRevisao["provider"] })}><option value="claude">Claude Code</option><option value="codex">Codex</option></select></label>
      <label>Escopo do agente <select aria-label="Escopo do agente" value={pedido.escopo} disabled={ocupado} onChange={e => setPedido({ ...pedido,escopo:e.target.value as PedidoDeRevisao["escopo"] })}><option value="local">Não commitado</option><option value="branch">Branch atual vs. base</option></select></label>
      {pedido.escopo === "branch" && <label>Branch base <input aria-label="Branch base" value={pedido.base ?? ""} disabled={ocupado} onChange={e => setPedido({ ...pedido,base:e.target.value })} placeholder="main" /></label>}
      <p>Um agente novo revisará a mesma pasta. A resposta aparecerá na conversa dele e o vínculo ficará no Fluxo.</p>
      <button disabled={ocupado || (pedido.escopo === "branch" && !pedido.base?.trim())} onClick={() => void iniciar()}>{ocupado ? "Abrindo revisor…" : "Iniciar revisão"}</button>
      <button disabled={ocupado} onClick={() => setAberto(false)}>Cancelar revisão</button>
    </section>}
    {erro && <p role="alert">{erro}</p>}
    {aviso && <p role="status">{aviso}</p>}
  </div>;
}
