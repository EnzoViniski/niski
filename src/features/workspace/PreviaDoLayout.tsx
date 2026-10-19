import { useEffect, useMemo, useState } from "react";
import { ChatIcon, TerminalIcon } from "../../components/ui/PixelIcon";
import { AGENT_ICON } from "../../components/ui/AgentIcon";
import type { LayoutNode, PaneNode, Project, WorkspaceAction, WorkspaceState, WorkspaceTab } from "../../types/workspace";
import { useSession } from "../terminal/sessionProviders";
import { workspaceReducer } from "./workspaceReducer";
import { EtiquetaDoProjeto, useProjetoDeFora } from "./projetosDasAbas";
import { TAB_DRAG_EVENT, type TabDragState } from "./tabPointerDrag";

/**
 * Fork: prévia do layout **final** enquanto se arrasta uma aba ou uma conversa da lateral.
 *
 * Em vez de só pintar a metade do painel onde a aba vai cair, mostra todos os painéis já na posição
 * que vão ficar — os que estavam lá encolhem para o lado, e o novo aparece no lugar dele, destacado.
 * Muda a cada zona por onde o ponteiro passa.
 *
 * O layout previsto sai do **mesmo reducer** que vai aplicar o soltar (`PLACE_SESSION`, `MOVE_TAB`,
 * `SPLIT_WITH_TAB`), então a prévia não tem como divergir do resultado. É só desenho: fica por cima
 * dos painéis com `pointer-events: none`, e a zona continua sendo medida nos painéis reais por baixo
 * (que não se mexem durante o arraste) — se a prévia recebesse o ponteiro, a zona trocaria a cada
 * quadro, porque o próprio layout mudaria debaixo do mouse.
 */
export function PreviaDoLayout({ project }: { project: Project }) {
  const [arraste, setArraste] = useState<TabDragState>(null);
  useEffect(() => {
    const atualizar = (evento: Event) => setArraste((evento as CustomEvent<TabDragState>).detail);
    window.addEventListener(TAB_DRAG_EVENT, atualizar);
    return () => window.removeEventListener(TAB_DRAG_EVENT, atualizar);
  }, []);

  const daLateral = arraste?.tab.paneId === "lateral";
  const titulo = useSession(daLateral ? arraste?.tab.tabId : null)?.name;
  const previsto = useMemo(() => (arraste?.target ? preverLayout(project, arraste, titulo ?? "Conversa") : null), [project, arraste, titulo]);
  if (!arraste?.target || !previsto) return null;

  return (
    <div className="previa-layout" role="status" aria-label="Prévia do layout">
      <NoDaPrevia node={previsto.layout} destaque={previsto.destaque} />
    </div>
  );
}

/** Layout depois do soltar, e a aba arrastada nele (para destacar o painel onde ela cai). */
export function preverLayout(project: Project, arraste: NonNullable<TabDragState>, titulo: string): { layout: LayoutNode; destaque: (tab: WorkspaceTab) => boolean } | null {
  const alvo = arraste.target;
  if (!alvo) return null;
  const estado: WorkspaceState = { version: 1, id: "previa", name: "", projects: [project], activeProjectId: project.id, updatedAt: "" };
  const { paneId, tabId } = arraste.tab;
  let acao: WorkspaceAction;
  let destaque: (tab: WorkspaceTab) => boolean;
  if (paneId === "lateral") {
    acao = { type: "PLACE_SESSION", sessionId: tabId, title: titulo, targetPaneId: alvo.paneId, zone: alvo.zone };
    destaque = (tab) => tab.resourceId === tabId;
  } else {
    // Soltar no próprio painel só faz algo quando divide e sobra aba para trás (como no `PaneView`).
    if (alvo.paneId === paneId && (alvo.zone === "center" || contarAbas(project.layout, paneId) === 1)) return null;
    acao = alvo.zone === "center"
      ? { type: "MOVE_TAB", sourcePaneId: paneId, tabId, targetPaneId: alvo.paneId }
      : { type: "SPLIT_WITH_TAB", sourcePaneId: paneId, tabId, targetPaneId: alvo.paneId,
          direction: alvo.zone === "left" || alvo.zone === "right" ? "horizontal" : "vertical",
          position: alvo.zone === "left" || alvo.zone === "top" ? "before" : "after" };
    destaque = (tab) => tab.id === tabId;
  }
  const depois = workspaceReducer(estado, acao).projects[0];
  // Soltar que não muda nada (a aba no próprio painel): sem prévia, como antes.
  if (!depois || depois.layout === project.layout) return null;
  return { layout: depois.layout, destaque };
}

function contarAbas(node: LayoutNode, paneId: string): number {
  if (node.type === "pane") return node.id === paneId ? node.tabs.length : 0;
  return contarAbas(node.first, paneId) + contarAbas(node.second, paneId);
}

function NoDaPrevia({ node, destaque }: { node: LayoutNode; destaque: (tab: WorkspaceTab) => boolean }) {
  if (node.type === "pane") return <PainelDaPrevia pane={node} destaque={destaque} />;
  return (
    <div className={`previa-split ${node.direction === "horizontal" ? "flex-row" : "flex-col"}`}>
      <div className="previa-lado" style={{ flexBasis: `${node.ratio * 100}%` }}><NoDaPrevia node={node.first} destaque={destaque} /></div>
      <div className="previa-lado flex-1"><NoDaPrevia node={node.second} destaque={destaque} /></div>
    </div>
  );
}

function PainelDaPrevia({ pane, destaque }: { pane: PaneNode; destaque: (tab: WorkspaceTab) => boolean }) {
  const ativa = pane.tabs.find((tab) => tab.id === pane.activeTabId) ?? pane.tabs[0];
  const novo = Boolean(ativa && destaque(ativa));
  const outras = pane.tabs.length - 1;
  return (
    <div className={`previa-painel${novo ? " is-novo" : ""}`}>
      {ativa ? <CabecalhoDaPrevia tab={ativa} /> : <span className="previa-titulo text-text-muted">Painel vazio</span>}
      {outras > 0 && <span className="previa-outras">+{outras} {outras === 1 ? "aba" : "abas"}</span>}
      {novo && <span className="previa-aviso">Vai abrir aqui</span>}
    </div>
  );
}

function CabecalhoDaPrevia({ tab }: { tab: WorkspaceTab }) {
  const session = useSession(tab.resourceId);
  const deFora = useProjetoDeFora(tab.resourceId);
  const provider = tab.provider ?? session?.provider;
  const Icone = (provider ? AGENT_ICON[provider] : undefined) ?? (tab.kind === "terminal" ? TerminalIcon : ChatIcon);
  return (
    <span className="previa-titulo">
      <Icone className="h-3.5 w-3.5 shrink-0" aria-hidden />
      <span className="truncate">{session?.name ?? tab.title}</span>
      {deFora && <EtiquetaDoProjeto projeto={deFora} />}
    </span>
  );
}
