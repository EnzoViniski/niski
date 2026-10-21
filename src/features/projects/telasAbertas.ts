// Fork: índice das telas abertas em todos os layouts, sem consultar nem iniciar agentes.
import type { LayoutNode, PaneKind, PaneNode, WorkspaceCollection, WorkspaceState } from "../../types/workspace";
import { findPane, workspaceReducer } from "../workspace/workspaceReducer";

export interface ContextoDaTela { workspaceId: string; projectId: string }
export interface DestinoDaTela extends ContextoDaTela { paneId: string; tabId: string }
export interface AbaNaTela {
  tabId: string; titulo: string; sessionId?: string; provider?: string; projeto?: string; ativa: boolean;
  kind?: PaneKind; recurso?: string;
}
export interface PainelNaTela {
  paneId: string; emFoco: boolean; abas: AbaNaTela[];
  workspaceId?: string; projectId?: string; workspace?: string; projeto?: string; posicao?: number;
  /** Fork: a tela (layout do projeto) deste painel está à vista agora. */
  telaVisivel?: boolean;
  /** Fork: onde cada painel da mesma tela fica (frações 0–1), para o mini mapa da lateral. */
  mapaDaTela?: AreaDoPainel[];
}

/** Retângulo de um painel no layout, em frações da tela. */
export interface AreaDoPainel { paneId: string; posicao: number; x: number; y: number; w: number; h: number }

/** Fork: posição de cada painel, seguindo a divisão salva (horizontal = lado a lado). */
export function mapaDoLayout(node: LayoutNode): AreaDoPainel[] {
  const areas: AreaDoPainel[] = [];
  const andar = (n: LayoutNode, x: number, y: number, w: number, h: number) => {
    if (n.type === "pane") { areas.push({ paneId: n.id, posicao: areas.length + 1, x, y, w, h }); return; }
    const r = Math.min(0.9, Math.max(0.1, Number.isFinite(n.ratio) ? n.ratio : 0.5));
    if (n.direction === "horizontal") { andar(n.first, x, y, w * r, h); andar(n.second, x + w * r, y, w * (1 - r), h); }
    else { andar(n.first, x, y, w, h * r); andar(n.second, x, y + h * r, w, h * (1 - r)); }
  };
  andar(node, 0, 0, 1, 1);
  return areas;
}

function paineis(node: LayoutNode): PaneNode[] {
  return node.type === "pane" ? [node] : [...paineis(node.first), ...paineis(node.second)];
}

/** A ordem continua a dos workspaces, projetos e painéis salvos. Uma sessão repetida aparece uma
 *  vez: ganha a aba visível ou, fora da tela, a primeira ocorrência. Seletores ainda sem sessão
 *  continuam navegáveis, assim como arquivos, terminais e outras telas abertas. */
export function todasAsTelasAbertas(workspaces: WorkspaceState[], activeWorkspaceId: string | null): PainelNaTela[] {
  const grupos: PainelNaTela[] = [];
  const prioridade = new Map<AbaNaTela, number>();
  const sessaoEscolhida = new Map<string, AbaNaTela>();
  for (const workspace of workspaces) for (const projeto of workspace.projects) {
    const layoutVisivel = workspace.id === activeWorkspaceId && projeto.id === workspace.activeProjectId;
    const mapaDaTela = mapaDoLayout(projeto.layout);
    for (const [indice, pane] of paineis(projeto.layout).entries()) {
      const painelVisivel = layoutVisivel && (!projeto.maximizedPaneId || pane.id === projeto.maximizedPaneId);
      const emFoco = painelVisivel && pane.id === projeto.activePaneId;
      const grupo: PainelNaTela = {
        paneId: pane.id, emFoco, workspaceId: workspace.id, projectId: projeto.id,
        workspace: workspace.name, projeto: projeto.name, posicao: indice + 1, abas: [],
        telaVisivel: layoutVisivel, mapaDaTela,
      };
      for (const tab of pane.tabs) {
        const sessionId = (tab.kind === "agent" || tab.kind === "terminal") ? tab.resourceId : undefined;
        const ativa = painelVisivel && tab.id === pane.activeTabId;
        const aba: AbaNaTela = {
          tabId: tab.id, titulo: tab.title, sessionId, provider: tab.provider, ativa,
          kind: tab.kind, recurso: tab.resourceId,
          projeto: tab.projectId && tab.projectId !== projeto.id ? workspace.projects.find((item) => item.id === tab.projectId)?.name : undefined,
        };
        grupo.abas.push(aba);
        const grau = ativa ? (emFoco ? 3 : 2) : painelVisivel ? 1 : 0;
        prioridade.set(aba, grau);
        if (sessionId) {
          const anterior = sessaoEscolhida.get(sessionId);
          if (!anterior || grau > prioridade.get(anterior)!) sessaoEscolhida.set(sessionId, aba);
        }
      }
      grupos.push(grupo);
    }
  }
  return grupos.map((grupo) => ({ ...grupo,
    abas: grupo.abas.filter((aba) => !aba.sessionId || sessaoEscolhida.get(aba.sessionId) === aba),
  })).filter((grupo) => grupo.abas.length > 0);
}

/** Uma única transição escolhe o contexto completo. Destino já fechado vira no-op. Se havia outro
 *  painel maximizado, mantém a maximização no painel escolhido para a aba não continuar escondida. */
export function focarTelaAberta(colecao: WorkspaceCollection, destino: DestinoDaTela): WorkspaceCollection {
  const workspace = colecao.workspaces.find((item) => item.id === destino.workspaceId);
  const projeto = workspace?.projects.find((item) => item.id === destino.projectId);
  const pane = projeto ? findPane(projeto.layout, destino.paneId) : null;
  if (!workspace || !projeto || !pane?.tabs.some((tab) => tab.id === destino.tabId)) return colecao;
  let atualizado = workspaceReducer(workspace, { type: "SELECT_PROJECT", projectId: destino.projectId });
  atualizado = workspaceReducer(atualizado, { type: "FOCUS_PANE", paneId: destino.paneId });
  atualizado = workspaceReducer(atualizado, { type: "SELECT_TAB", paneId: destino.paneId, tabId: destino.tabId });
  if (projeto.maximizedPaneId && projeto.maximizedPaneId !== destino.paneId) {
    atualizado = { ...atualizado, projects: atualizado.projects.map((item) => item.id === projeto.id
      ? { ...item, maximizedPaneId: destino.paneId } : item) };
  }
  return { ...colecao, activeWorkspaceId: workspace.id,
    workspaces: colecao.workspaces.map((item) => item.id === workspace.id ? atualizado : item) };
}
