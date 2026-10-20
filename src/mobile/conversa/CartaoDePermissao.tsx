// Fork (Onda 2.0): o pedido de aprovação da conversa (antes inline no `MobileApp.tsx`), sem mudança.
import { Button } from "../../components/ui/Button";
import { WarningIcon } from "../../components/ui/PixelIcon";

/** O agente pediu permissão: o texto da tela e os dois botões, como no terminal. */
export function CartaoDePermissao({ texto, busy, onResponder }: {
  /** O que a CLI está pedindo (`capabilities.approval_text`). */
  texto?: string | null;
  busy: boolean;
  onResponder: (permitir: boolean) => void;
}) {
  return <div className="bubble bubble-agent bubble-aprovacao glass">
    <p className="bubble-author"><WarningIcon aria-hidden className="h-3.5 w-3.5 text-warning" />Aprovação pendente</p>
    <p className="mobile-message">{texto}</p>
    <div className="m-acoes">
      <Button variant="primary" disabled={busy} onClick={() => onResponder(true)}>Permitir</Button>
      <Button variant="danger" disabled={busy} onClick={() => onResponder(false)}>Negar</Button>
    </div>
  </div>;
}
