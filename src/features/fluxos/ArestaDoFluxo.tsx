import { BaseEdge, EdgeLabelRenderer, getSmoothStepPath, useReactFlow, type Edge, type EdgeProps } from "@xyflow/react";

export type DadosDaAresta = { saida?: string; editavel: boolean; viva?: boolean };
export type ArestaOmni = Edge<DadosDaAresta, "omni">;

const ROTULO: Record<string, string> = { sim: "sim", nao: "não", esgotado: "esgotou" };

/** Conexão entre duas etapas: curva em degrau com seta, a saída escrita no meio (sim/não/esgotou) e
 *  um × para desfazer — aparece ao passar o mouse ou com a conexão selecionada. */
export function ArestaDoFluxo({ id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data, markerEnd, selected }: EdgeProps<ArestaOmni>) {
  const [caminho, meioX, meioY] = getSmoothStepPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, borderRadius: 16, offset: 28 });
  const { deleteElements } = useReactFlow();
  const saida = data?.saida;
  // Saída nomeada (sim/não/esgotou): rótulo logo depois da porta — no meio, as voltas passam por cima dos nós.
  const [x, y] = saida ? [sourceX + 30, sourceY] : [meioX, meioY];
  return (
    <>
      <BaseEdge id={id} path={caminho} markerEnd={markerEnd} interactionWidth={24}
        className={`fx-aresta${saida ? ` is-${saida}` : ""}${data?.viva ? " is-viva" : ""}${selected ? " is-sel" : ""}`} />
      {(saida || data?.editavel) && (
        <EdgeLabelRenderer>
          <div className={`fx-aresta-meio nodrag nopan${selected ? " is-sel" : ""}`} style={{ transform: `translate(-50%, -50%) translate(${x}px, ${y}px)` }}>
            {saida && <span className={`fx-aresta-saida is-${saida}`}>{ROTULO[saida] ?? saida}</span>}
            {data?.editavel && (
              <button type="button" className="fx-aresta-x" title="Remover esta conexão" aria-label="Remover conexão"
                onClick={() => void deleteElements({ edges: [{ id }] })}>×</button>
            )}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}
