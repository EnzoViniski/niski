import { useEffect, useState } from "react";
import { ChartIcon, CloseIcon } from "../../components/ui/PixelIcon";
import { ListaDeUsoDasContas } from "./UsoDeTodasAsContas";

/**
 * Botão da barra de título que abre, da direita para a esquerda, a gaveta com o uso de todas as
 * contas. Fica sempre montada (para animar); fechada, sai da tela com `visibility: hidden` (nem foco recebe).
 */
export function GavetaDeUso() {
  const [aberta, setAberta] = useState(false);

  useEffect(() => {
    if (!aberta) return;
    const esc = (evento: KeyboardEvent) => { if (evento.key === "Escape") setAberta(false); };
    document.addEventListener("keydown", esc);
    return () => document.removeEventListener("keydown", esc);
  }, [aberta]);

  return (
    <>
      <button type="button" aria-label="Uso das contas" title="Uso das contas" aria-expanded={aberta} aria-controls="gaveta-uso"
        onClick={() => setAberta((atual) => !atual)} className={`titlebar-acao${aberta ? " text-accent" : ""}`}>
        <ChartIcon aria-hidden className="h-3.5 w-3.5" />
        <span>Uso</span>
      </button>
      <div className={`gaveta-uso-fundo${aberta ? " is-aberta" : ""}`} onClick={() => setAberta(false)} aria-hidden />
      <aside id="gaveta-uso" role="dialog" aria-label="Uso das contas" aria-hidden={!aberta}
        className={`gaveta-uso${aberta ? " is-aberta" : ""}`}>
        <div className="flex items-center justify-end border-b border-border-subtle px-2 py-1">
          <button type="button" aria-label="Fechar uso das contas" onClick={() => setAberta(false)}
            className="grid h-6 w-6 place-items-center text-text-muted hover:text-text-primary">
            <CloseIcon aria-hidden className="h-3.5 w-3.5" />
          </button>
        </div>
        <div className="flex min-h-0 flex-1 flex-col text-[11px]">
          <ListaDeUsoDasContas ativo={aberta} engineOnline contaAtiva={null} />
        </div>
      </aside>
    </>
  );
}
