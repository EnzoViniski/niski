// Fork (Tarefas): o painel por cima do app — fundo escurecido, tarefas no meio e o assistente de IA
// à direita. Esc ou clique fora fecham (Esc primeiro fecha os detalhes de uma tarefa aberta).
import { useEffect } from "react";
import { Button } from "../../components/ui";
import { SparkleIcon } from "../../components/ui/PixelIcon";
import { AssistenteDeTarefas } from "./AssistenteDeTarefas";
import { alternarListaDoDia, fecharPainelDeTarefas, mudarPainel, usePainel } from "./painel";
import { TarefasView } from "./TarefasView";
import "./painel.css";

export function PainelDeTarefas({ onAbrirAjustes }: { onAbrirAjustes: () => void }) {
  const painel = usePainel();
  useEffect(() => {
    if (!painel.aberto) return;
    const tecla = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      // Detalhes abertos ou digitando: o Esc é deles.
      if (document.querySelector(".tarefas-painel .tarefa-gaveta") || (e.target instanceof HTMLElement && e.target.closest("input, textarea"))) return;
      fecharPainelDeTarefas();
    };
    window.addEventListener("keydown", tecla);
    return () => window.removeEventListener("keydown", tecla);
  }, [painel.aberto]);

  if (!painel.aberto) return null;
  return (
    <div className="tarefas-painel-fundo" onMouseDown={(e) => { if (e.target === e.currentTarget) fecharPainelDeTarefas(); }}>
      <div className="tarefas-painel" role="dialog" aria-modal="true" aria-label="Tarefas">
        <TarefasView onClose={fecharPainelDeTarefas} onAbrirAjustes={() => { fecharPainelDeTarefas(); onAbrirAjustes(); }}
          acoes={<>
            <Button size="sm" variant={painel.listaFixada ? "primary" : "ghost"} onClick={alternarListaDoDia}
              title="Lista do dia flutuante: deixa as tarefas de hoje por cima do app">📌 Hoje</Button>
            {!painel.iaVisivel && (
              <Button size="sm" variant="ghost" onClick={() => mudarPainel({ iaVisivel: true })} title="Conversar com a IA para criar e organizar tarefas">
                <SparkleIcon className="h-3.5 w-3.5" aria-hidden /> Assistente
              </Button>
            )}
          </>}
          lateral={painel.iaVisivel ? <AssistenteDeTarefas onFechar={() => mudarPainel({ iaVisivel: false })} /> : undefined} />
      </div>
    </div>
  );
}
