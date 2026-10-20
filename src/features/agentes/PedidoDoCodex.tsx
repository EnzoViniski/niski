import { useState } from "react";
import { Button } from "../../components/ui/Button";
import type { PedidoDoGancho } from "../../mobile/api";
import { responderPedido } from "../../mobile/conversa/pedidoDoGancho";

/** Fork 3A: aceita/nega só esta ação. A aprovação da conexão do Medza permanece no Medza. */
export function PedidoDoCodex({ pedido, conversaId, busy, onRespondido }: {
  pedido: PedidoDoGancho; conversaId: string; busy: boolean; onRespondido?: () => void;
}) {
  const [ocupado, setOcupado] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  async function responder(permitir: boolean) {
    setOcupado(true); setErro(null);
    try { await responderPedido(conversaId, pedido, permitir); }
    catch { setErro("O pedido mudou ou a conexão caiu. Atualize a conversa."); }
    finally { setOcupado(false); onRespondido?.(); }
  }
  return <div className="bubble bubble-agent bubble-aprovacao glass" aria-label="Pedido de permissão do Codex">
    <p className="bubble-author">Aprovação pendente · {pedido.ferramenta}</p>
    <p className="mobile-message">{pedido.resumo}</p>
    {pedido.arriscado && <p className="text-[11px] text-warning">Atenção: este pedido {pedido.arriscado}.</p>}
    <p className="text-[11px] text-text-muted">Vale só para esta ação. Sem resposta em 10 minutos, ela será negada.</p>
    <div className="m-acoes">
      <Button variant="primary" disabled={busy || ocupado} onClick={() => void responder(true)}>Permitir</Button>
      <Button variant="danger" disabled={busy || ocupado} onClick={() => void responder(false)}>Negar</Button>
    </div>
    {erro && <p role="alert" className="text-[11px] text-danger">{erro}</p>}
  </div>;
}
