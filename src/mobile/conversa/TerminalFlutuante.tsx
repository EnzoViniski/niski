// Fork: o seletor oficial da CLI sobre a Conversa, na mesma PTY.
import { lazy, Suspense, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import "./controlesDaConversa.css";

const TerminalEspelho = lazy(() => import("../../web/TerminalEspelho"));

// Fork (pergunta-da-cli): "pergunta" abre o terminal para responder um diálogo da CLI.
export type TipoDoTerminal = "rewind" | "branch" | "pergunta";
const TEXTOS: Record<TipoDoTerminal, { titulo: string; dica: string }> = {
  rewind: { titulo: "Voltar a um ponto", dica: "Não desfaz comandos de Bash nem edições de subagentes em segundo plano. Escolha no terminal o que deseja restaurar." },
  branch: { titulo: "Ramificar aqui", dica: "A CLI passa para a ramificação nesta mesma aba. A conversa original continua no histórico." },
  pergunta: { titulo: "Responder no terminal", dica: "A CLI está esperando uma resposta. Use as setas, Enter ou a tecla que ela pedir." },
};

export function TerminalFlutuante({ conversaId, tipo, onFechar }: {
  conversaId: string; tipo: TipoDoTerminal; onFechar: () => void;
}) {
  const textos = TEXTOS[tipo];
  const fechar = useRef<HTMLButtonElement>(null);
  const painel = useRef<HTMLDivElement>(null);
  const rota = `/conversas/${encodeURIComponent(conversaId)}`;
  useEffect(() => {
    const anterior = document.activeElement as HTMLElement | null;
    fechar.current?.focus();
    return () => anterior?.focus();
  }, []);
  return createPortal(<div className="conversa-terminal-fundo" onMouseDown={(e) => { if (e.target === e.currentTarget) onFechar(); }}>
    <div ref={painel} className="conversa-terminal-painel" role="dialog" aria-modal="true" aria-label={textos.titulo}
      onKeyDown={(e) => {
        // Esc pertence ao seletor da CLI; fechar o painel é um botão explícito.
        if (e.key !== "Tab") return;
        const elementos = painel.current?.querySelectorAll<HTMLElement>('button, textarea, [tabindex="0"]');
        if (!elementos?.length) return;
        const primeiro = elementos[0]; const ultimo = elementos[elementos.length - 1];
        if (e.shiftKey && document.activeElement === primeiro) { e.preventDefault(); ultimo.focus(); }
        else if (!e.shiftKey && document.activeElement === ultimo) { e.preventDefault(); primeiro.focus(); }
      }}>
      <div className="conversa-terminal-topo"><strong>{textos.titulo}</strong>
        <button ref={fechar} type="button" className="m-chip-aplicar" onClick={onFechar}>Fechar terminal</button></div>
      <p className="m-dica">{textos.dica}</p>
      <p className="m-dica">{tipo === "pergunta" ? "Esc costuma cancelar o diálogo da CLI." : "Esc fecha o seletor da CLI."} Fechar este painel só esconde o terminal.</p>
      <Suspense fallback={<p role="status">Abrindo terminal…</p>}>
        <TerminalEspelho saida={`${rota}/terminal`} teclas={`${rota}/teclas`} rotulo="Use as setas e Enter para escolher na CLI" />
      </Suspense>
    </div>
  </div>, document.body);
}
