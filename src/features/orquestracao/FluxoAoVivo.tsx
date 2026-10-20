import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Background, BackgroundVariant, Controls, Handle, MarkerType, Position, ReactFlow, ReactFlowProvider, useReactFlow,
  type Edge, type Node, type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { usePoll } from "../../mobile/usePoll";
import { energiaPausada } from "../../lib/energia";
import { listTerminalSessions } from "../terminal/terminalService";
import { latestPerConversation } from "../terminal/useAttention";
import { ArestaDoFluxo, type ArestaOmni } from "../fluxos/ArestaDoFluxo";
import { pedirAbrirConversa } from "./abrirFluxo";
import { definirLimite, obterFluxo, type FluxoAoVivo as Fluxo, type NoAoVivo } from "./orquestracaoService";
import "./orquestracao.css";
import "./fluxoAoVivo.css";
// Fatia (c): mandar prompt a qualquer nó, aprovar/negar e parar pelo fluxo.
import { AcoesContext, AcoesDoNo, ParaQuem, useAcoesDoFluxo } from "./InteragirPeloFluxo";
import { nomeLongoDaPlataforma } from "../../lib/plataformasDeAgente";

/**
 * Fork: fluxo ao vivo de uma conversa orquestradora, numa aba do layout (`PaneKind "orquestracao"`).
 *
 * O orquestrador fica à esquerda e os subagentes descem em coluna à direita dele, com uma seta para cada. Cada nó
 * mostra a marca da plataforma, a conta, o modelo, o estado ao vivo, a tarefa e a última mensagem.
 * Clicar num nó abre a conversa dele ao lado. Montado do registro do engine (`GET /orquestracao/{id}`),
 * então sobrevive a reabrir o app. Visual do `NoDoFluxo` (classes `fx-*`) e a `ArestaDoFluxo`.
 *
 * Energia: relê a cada 2 s com a janela à vista; na pausa de energia, a cada 8 s; oculta, para
 * (`usePoll`).
 */
const INTERVALO_MS = 2_000;
const INTERVALO_PAUSADO_MS = 8_000;


export type DadosDoNoAoVivo = { no: NoAoVivo; centro: boolean; orquestrador: string };
export type NoDoOrquestrador = Node<DadosDoNoAoVivo, "orq">;

/** Subagentes por coluna antes de abrir a próxima coluna à direita. */
export const POR_COLUNA = 5;
const PASSO_X = 330, PASSO_Y = 200, PRIMEIRA_COLUNA_X = 400;

/**
 * Fork (pedido do Enzo): da esquerda para a direita. O orquestrador fica à esquerda, na altura do meio
 * da primeira coluna; os subagentes descem em coluna à direita dele, na ordem em que foram criados, e
 * a cada {@link POR_COLUNA} abre-se outra coluna mais à direita.
 */
export function posicoes(n: number): { centro: { x: number; y: number }; agentes: { x: number; y: number }[] } {
  const naPrimeira = Math.min(n, POR_COLUNA);
  const agentes = Array.from({ length: n }, (_, i) => ({
    x: PRIMEIRA_COLUNA_X + Math.floor(i / POR_COLUNA) * PASSO_X,
    y: (i % POR_COLUNA) * PASSO_Y,
  }));
  return { centro: { x: 0, y: Math.max(0, (naPrimeira - 1) * PASSO_Y) / 2 }, agentes };
}

/** Harness: quantos arquivos o subagente pode editar/criar; a lista vai no título. */
export function ArquivosDoNo({ no }: { no: NoAoVivo }) {
  const editar = no.editar ?? [], criar = no.criar ?? [];
  if (editar.length + criar.length === 0) return null;
  const lista = [...editar.map((c) => `edita ${c}`), ...criar.map((c) => `cria ${c}`)].join("\n");
  return (
    <div className="orq-arquivos" title={lista}>
      {editar.length > 0 && <span>edita {editar.length === 1 ? editar[0] : `${editar.length} caminhos`}</span>}
      {criar.length > 0 && <span>cria {criar.length === 1 ? criar[0] : `${criar.length} caminhos`}</span>}
    </div>
  );
}

export function rotuloDoNo(no: NoAoVivo): string {
  return [no.conta, no.modelo, no.esforco].filter(Boolean).join(" · ") || "conta e modelo padrão";
}

const NoAoVivoView = memo(function NoAoVivoView({ data }: NodeProps<NoDoOrquestrador>) {
  const { no, centro, orquestrador } = data;
  const plataforma = no.plataforma ?? "";
  return (
    <div className={`fx-no orq-no st-${no.estado}${centro ? " is-centro" : ""}`} title="Clique para abrir a conversa">
      {centro
        ? <Handle type="source" position={Position.Right} className="orq-porta" isConnectable={false} />
        : <Handle type="target" position={Position.Left} className="orq-porta" isConnectable={false} />}
      <div className="fx-no-topo">
        <span className="fx-no-icone"><AgentIcon provider={plataforma} size={14} /></span>
        <span className="fx-no-tipo">{centro ? "Orquestrador" : (nomeLongoDaPlataforma(plataforma) ?? plataforma)}</span>
        <span className={`orq-estado st-${no.estado}`}><span className="orq-ponto" aria-hidden />{no.estado_texto}</span>
      </div>
      <div className="fx-no-titulo">{centro ? (no.titulo ?? "Orquestrador") : (no.tarefa ?? no.titulo ?? "Subagente")}</div>
      <div className="fx-no-sub">{rotuloDoNo(no)}</div>
      <ArquivosDoNo no={no} />
      {no.pendente && <div className="orq-pendente">{no.pendente}</div>}
      {no.ultima_mensagem && <div className="orq-ultima" title={no.ultima_mensagem}>{no.ultima_mensagem}</div>}
      <AcoesDoNo no={no} orquestrador={orquestrador} centro={centro} />
    </div>
  );
});

const tiposDeNo = { orq: NoAoVivoView };
const tiposDeAresta = { omni: ArestaDoFluxo };

/** Lê o fluxo no ritmo certo para a energia. */
export function useFluxoAoVivo(orquestrador: string) {
  const [fluxo, setFluxo] = useState<Fluxo | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const lidoEm = useRef(0);
  const ler = useCallback(async () => {
    const dados = await obterFluxo(orquestrador);
    lidoEm.current = Date.now();
    setFluxo(dados);
    setErro(null);
  }, [orquestrador]);
  usePoll(async () => {
    if (energiaPausada() && Date.now() - lidoEm.current < INTERVALO_PAUSADO_MS) return;
    await ler();
  }, (motivo) => setErro(motivo instanceof Error ? motivo.message : String(motivo)), [orquestrador], INTERVALO_MS);
  return { fluxo, erro, recarregar: ler };
}

/** Abre a conversa do nó ao lado do fluxo (pela sessão mais nova dela). */
async function abrirConversa(no: NoAoVivo, paneId: string | undefined) {
  const sessoes = latestPerConversation(await listTerminalSessions());
  const sessao = sessoes.find((s) => s.conversation_id === no.id);
  if (sessao) pedirAbrirConversa(sessao.id, no.titulo ?? sessao.name, paneId);
}

function Canvas({ fluxo, orquestrador, paneId }: { fluxo: Fluxo; orquestrador: string; paneId?: string }) {
  const { fitView } = useReactFlow();
  const { nodes, edges } = useMemo(() => {
    const lugares = posicoes(fluxo.agentes.length);
    const nodes: NoDoOrquestrador[] = [
      { id: fluxo.orquestrador.id, type: "orq", position: lugares.centro, data: { no: fluxo.orquestrador, centro: true, orquestrador } },
      ...fluxo.agentes.map((no, i) => ({ id: no.id, type: "orq" as const, position: lugares.agentes[i],
        data: { no, centro: false, orquestrador } })),
    ];
    const edges: ArestaOmni[] = fluxo.agentes.map((no) => ({
      id: `${fluxo.orquestrador.id}->${no.id}`, source: fluxo.orquestrador.id, target: no.id, type: "omni",
      data: { editavel: false, viva: no.estado === "trabalhando" },
      markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16, color: no.estado === "trabalhando" ? "#f97316" : "#6b6b85" },
    }));
    return { nodes, edges };
  }, [fluxo, orquestrador]);
  // Enquadra de novo quando entra ou sai um subagente.
  useEffect(() => {
    const t = window.setTimeout(() => fitView({ padding: 0.25, maxZoom: 1, duration: 250 }), 60);
    return () => window.clearTimeout(t);
  }, [fluxo.agentes.length, fitView]);
  return (
    <ReactFlow<NoDoOrquestrador, Edge> nodes={nodes} edges={edges as Edge[]} nodeTypes={tiposDeNo} edgeTypes={tiposDeAresta}
      nodesDraggable={false} nodesConnectable={false} elementsSelectable={false} fitView fitViewOptions={{ padding: 0.25, maxZoom: 1 }}
      minZoom={0.3} proOptions={{ hideAttribution: true }}
      onNodeClick={(evento, node) => {
        // Botão dentro do nó (aprovar, parar) não abre a conversa.
        if ((evento.target as HTMLElement).closest("button, select, textarea")) return;
        void abrirConversa(node.data.no, paneId);
      }}>
      <Background variant={BackgroundVariant.Dots} gap={18} size={1} />
      <Controls showInteractive={false} />
    </ReactFlow>
  );
}

export function FluxoAoVivo({ orquestrador, paneId }: { orquestrador: string; paneId?: string }) {
  const { fluxo, erro, recarregar } = useFluxoAoVivo(orquestrador);
  const acoes = useAcoesDoFluxo(recarregar);
  const vivos = fluxo?.agentes.filter((a) => a.estado !== "parado" && a.estado !== "erro").length ?? 0;
  return (
    <div className="orq-fluxo">
      <div className="orq-topo">
        <span className="orq-topo-titulo">{fluxo?.orquestrador.titulo ?? "Fluxo do orquestrador"}</span>
        {fluxo && (
          <span className={`orq-selo${fluxo.ativo ? " is-ativo" : ""}`}>{fluxo.ativo ? "orquestrando" : "sem ferramentas"}</span>
        )}
        <span className="flex-1" />
        {fluxo && (
          <label className="orq-limite" title="Subagentes vivos ao mesmo tempo, por orquestrador">
            {vivos} vivo(s) · limite
            <select aria-label="Limite de subagentes" value={fluxo.limite} onChange={(e) => void definirLimite(Number(e.target.value)).then(recarregar).catch(() => undefined)}>
              {[1, 2, 3, 4, 5, 6, 7, 8].map((n) => <option key={n} value={n}>{n}</option>)}
            </select>
          </label>
        )}
      </div>
      {(acoes.erro || erro) && <div role="alert" className="orq-erro">{acoes.erro ?? erro}</div>}
      <div className="orq-canvas">
        {!fluxo ? (
          <p className="orq-vazio">{erro ? "Não consegui ler o fluxo." : "Abrindo o fluxo…"}</p>
        ) : (
          <>
            <AcoesContext.Provider value={acoes}>
              <ReactFlowProvider>
                <Canvas fluxo={fluxo} orquestrador={orquestrador} paneId={paneId} />
              </ReactFlowProvider>
            </AcoesContext.Provider>
            {fluxo.agentes.length === 0 && (
              <p className="orq-dica">
                {fluxo.ativo ? "Nenhum subagente ainda. Peça na conversa: “cria 2 agentes para…”."
                  : "Esta conversa não está orquestrando. Use “Tornar orquestradora” na faixa da conversa."}
              </p>
            )}
          </>
        )}
      </div>
      {fluxo && <ParaQuem key={fluxo.orquestrador.id} fluxo={fluxo} acoes={acoes} />}
    </div>
  );
}
