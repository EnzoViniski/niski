import { useState } from "react";
import { CheckIcon, ListIcon } from "../components/ui/PixelIcon";
import type { Tarefa } from "./api";
import "./conversaRica.css";

/**
 * A lista de tarefas do agente, fixa acima do campo de mensagem — o checklist que o Claude
 * Code mostra (Ctrl+T) e o plano do Codex. Atualiza a cada leitura da conversa; recolhida, mostra só
 * "3/7" e o que está sendo feito agora.
 *
 * Some quando a lista acabou e o agente parou: uma lista toda feita, para sempre em cima do campo,
 * só ocupa espaço. Com o agente trabalhando ela fica (ele pode acrescentar mais).
 */

const CHAVE = (conversa: string) => `omni-lista-recolhida:${conversa}`;

function lerRecolhida(conversa: string): boolean | null {
  try {
    const valor = sessionStorage.getItem(CHAVE(conversa));
    return valor === null ? null : valor === "1";
  } catch { return null; }
}

function gravarRecolhida(conversa: string, recolhida: boolean) {
  try { sessionStorage.setItem(CHAVE(conversa), recolhida ? "1" : "0"); } catch { /* sem armazenamento: vale só agora */ }
}

export function progressoDasTarefas(tarefas: Tarefa[]): { feitas: number; total: number; atual: Tarefa | undefined } {
  return {
    feitas: tarefas.filter((t) => t.estado === "feita").length,
    total: tarefas.length,
    atual: tarefas.find((t) => t.estado === "andamento"),
  };
}

const ROTULO_DO_ESTADO: Record<Tarefa["estado"], string> = { feita: "feita", andamento: "em andamento", pendente: "pendente" };

function MarcaDaTarefa({ estado }: { estado: Tarefa["estado"] }) {
  if (estado === "feita") return <span className="tarefa-marca is-feita" aria-hidden><CheckIcon className="h-3 w-3" /></span>;
  return <span className={`tarefa-marca${estado === "andamento" ? " is-andamento" : ""}`} aria-hidden />;
}

export function ListaDeTarefas({ tarefas, conversa, ativo }: {
  tarefas: Tarefa[];
  /** Id da conversa: lembra se você recolheu a lista (nesta aba). */
  conversa: string;
  /** Agente trabalhando: a lista fica mesmo toda feita. */
  ativo: boolean;
}) {
  const { feitas, total, atual } = progressoDasTarefas(tarefas);
  const [recolhida, setRecolhida] = useState(() => lerRecolhida(conversa) ?? false);
  if (total === 0 || (feitas === total && !ativo)) return null;
  const alternar = () => setRecolhida((antes) => { gravarRecolhida(conversa, !antes); return !antes; });
  return (
    <section className={`lista-tarefas${recolhida ? " is-recolhida" : ""}`} aria-label="Lista de tarefas do agente">
      <button type="button" className="lista-tarefas-topo" aria-expanded={!recolhida} onClick={alternar}
        title={recolhida ? "Mostrar a lista de tarefas" : "Recolher a lista de tarefas"}>
        <span className="acoes-seta" aria-hidden>{recolhida ? "▸" : "▾"}</span>
        <ListIcon aria-hidden className="h-3.5 w-3.5 shrink-0" />
        <span className="lista-tarefas-titulo">Tarefas</span>
        <span className="lista-tarefas-contagem">{feitas}/{total}</span>
        {/* `<progress>` e não uma barra com `style`: o celular manda `style-src 'self'`. */}
        <progress className="lista-tarefas-barra" value={feitas} max={total} aria-hidden />
        {recolhida && atual && <span className="lista-tarefas-atual">{atual.fazendo ?? atual.texto}</span>}
      </button>
      {!recolhida && (
        <ol className="lista-tarefas-itens">
          {tarefas.map((tarefa, i) => (
            <li key={`${i}-${tarefa.texto}`} className={`tarefa is-${tarefa.estado}`}>
              <MarcaDaTarefa estado={tarefa.estado} />
              <span className="tarefa-texto">{tarefa.estado === "andamento" ? tarefa.fazendo ?? tarefa.texto : tarefa.texto}</span>
              <span className="sr-only"> ({ROTULO_DO_ESTADO[tarefa.estado]})</span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
