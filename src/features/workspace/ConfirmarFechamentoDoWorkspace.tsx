// Fork: fechar um workspace só encerra as sessões depois da confirmação explícita.
import { useEffect, useId, useRef, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { Button } from "../../components/ui";

export function ConfirmarFechamentoDoWorkspace({ nome, onCancelar, onConfirmar }: {
  nome: string;
  onCancelar: () => void;
  onConfirmar: () => void;
}) {
  const titulo = useId();
  const descricao = useId();
  const cancelar = useRef<HTMLButtonElement>(null);
  const confirmar = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const anterior = document.activeElement;
    cancelar.current?.focus();
    return () => { if (anterior instanceof HTMLElement && anterior.isConnected) anterior.focus(); };
  }, []);

  function teclado(evento: KeyboardEvent) {
    // O teclado fica no diálogo: atalhos do workspace não podem fechar painéis por baixo.
    evento.stopPropagation();
    if (evento.key === "Escape") { evento.preventDefault(); onCancelar(); }
    if (evento.key === "Tab") {
      evento.preventDefault();
      (document.activeElement === cancelar.current ? confirmar : cancelar).current?.focus();
    }
  }

  return createPortal(
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-4"
      onPointerDown={(evento) => { if (evento.target === evento.currentTarget) onCancelar(); }}>
      <section role="alertdialog" aria-modal="true" aria-labelledby={titulo} aria-describedby={descricao}
        className="popover w-full max-w-md p-4" onKeyDown={teclado}>
        <h2 id={titulo} className="text-sm font-semibold text-text-primary">Fechar workspace {nome}?</h2>
        <p id={descricao} className="mt-2 text-xs text-text-secondary">
          As abas deste workspace serão fechadas e os agentes em execução serão encerrados. As conversas continuam no Histórico.
        </p>
        <div className="mt-4 flex justify-end gap-2">
          <Button ref={cancelar} variant="ghost" size="sm" onClick={onCancelar}>Cancelar</Button>
          <Button ref={confirmar} variant="danger" size="sm" onClick={onConfirmar}>Fechar</Button>
        </div>
      </section>
    </div>, document.body,
  );
}
