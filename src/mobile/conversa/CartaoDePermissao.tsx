// Fork (Onda 2.0): o pedido de aprovação da conversa (antes inline no `MobileApp.tsx`).
// Fork (Onda 2, item 2): pedido que veio pelo gancho do Claude mostra a ferramenta, o prazo, "Negar"
// com o que fazer e "Responder no terminal"; responde direto ao engine com o id do pedido.
import { useState } from "react";
import { PedidoDoCodex } from "../../features/agentes/PedidoDoCodex"; // Fork: 3A.
import { Button } from "../../components/ui/Button";
import { WarningIcon } from "../../components/ui/PixelIcon";
import { ApiError, type PedidoDoGancho } from "../api";
import { formatarRestante, responderPedido, soltarPedido, useSegundosRestantes } from "./pedidoDoGancho";

/** O agente pediu permissão: o texto da tela e os dois botões, como no terminal. */
export function CartaoDePermissao({ texto, busy, onResponder, pedido, conversaId, onRespondido }: {
  /** O que a CLI está pedindo (`capabilities.approval_text`). */
  texto?: string | null;
  busy: boolean;
  onResponder: (permitir: boolean) => void;
  /** Pedido segurado pelo gancho (`capabilities.pedido`): responde direto, sem tecla na tela. */
  pedido?: PedidoDoGancho | null;
  conversaId?: string;
  /** Depois de responder (ou de o pedido mudar): reler a conversa. */
  onRespondido?: () => void;
}) {
  if (pedido && conversaId) {
    if (pedido.origem === "codex_app_server") return <PedidoDoCodex pedido={pedido} conversaId={conversaId} busy={busy} onRespondido={onRespondido} />; // Fork: sem handoff de gancho.
    return <PedidoDoGanchoCartao pedido={pedido} conversaId={conversaId} busy={busy} onRespondido={onRespondido} />;
  }
  return <div className="bubble bubble-agent bubble-aprovacao glass">
    <p className="bubble-author"><WarningIcon aria-hidden className="h-3.5 w-3.5 text-warning" />Aprovação pendente</p>
    <p className="mobile-message">{texto}</p>
    <div className="m-acoes">
      <Button variant="primary" disabled={busy} onClick={() => onResponder(true)}>Permitir</Button>
      <Button variant="danger" disabled={busy} onClick={() => onResponder(false)}>Negar</Button>
    </div>
  </div>;
}

function mensagemDoErro(motivo: unknown): string {
  if (motivo instanceof ApiError && motivo.status === 412) return "O pedido mudou; confira o novo antes de responder.";
  if (motivo instanceof ApiError && motivo.status === 409) return "Este pedido já foi respondido.";
  return motivo instanceof Error ? motivo.message : String(motivo);
}

function PedidoDoGanchoCartao({ pedido, conversaId, busy, onRespondido }: {
  pedido: PedidoDoGancho; conversaId: string; busy: boolean; onRespondido?: () => void;
}) {
  const [ocupado, setOcupado] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [negando, setNegando] = useState(false);
  const [mensagem, setMensagem] = useState("");
  const restantes = useSegundosRestantes(pedido.expira_em_ms);
  const travado = busy || ocupado;

  async function executar(acao: () => Promise<unknown>) {
    setOcupado(true); setErro(null);
    try { await acao(); }
    catch (motivo) { setErro(mensagemDoErro(motivo)); }
    finally { setOcupado(false); onRespondido?.(); }
  }

  return <div className="bubble bubble-agent bubble-aprovacao glass" aria-label="Pedido de permissão">
    <p className="bubble-author">
      <WarningIcon aria-hidden className="h-3.5 w-3.5 text-warning" />
      Aprovação pendente · {pedido.ferramenta}{pedido.servidor_mcp ? ` (${pedido.servidor_mcp})` : ""}
    </p>
    <p className="mobile-message">{pedido.resumo}</p>
    {pedido.arriscado && <p className="text-[11px] text-warning">Atenção: este pedido {pedido.arriscado}.</p>}
    {restantes !== null && <p className="text-[11px] text-text-muted" aria-live="off">
      {restantes > 0 ? `Vai para o terminal em ${formatarRestante(restantes)}` : "Indo para o terminal…"}
    </p>}
    {negando && <textarea aria-label="Dizer o que fazer" rows={2} className="m-campo-nome mt-2 w-full" value={mensagem} maxLength={2000}
      placeholder="Opcional: diga ao agente o que fazer em vez disso" onChange={(e) => setMensagem(e.target.value)} />}
    <div className="m-acoes">
      {negando ? <>
        <Button variant="danger" disabled={travado} onClick={() => void executar(() => responderPedido(conversaId, pedido, false, mensagem))}>Negar</Button>
        <Button variant="ghost" disabled={travado} onClick={() => setNegando(false)}>Voltar</Button>
      </> : <>
        <Button variant="primary" disabled={travado} onClick={() => void executar(() => responderPedido(conversaId, pedido, true))}>Permitir</Button>
        <Button variant="danger" disabled={travado} onClick={() => setNegando(true)}>Negar…</Button>
        <Button variant="ghost" disabled={travado} title="A CLI mostra o diálogo dela no terminal"
          onClick={() => void executar(() => soltarPedido(conversaId, pedido))}>Responder no terminal</Button>
      </>}
    </div>
    {erro && <p role="alert" className="text-[11px] text-danger">{erro}</p>}
  </div>;
}
