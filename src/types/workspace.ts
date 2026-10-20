export type PaneKind =
  // Fork: conversa arquivada, sem criar PTY ao abrir pela paleta.
  | "conversa-historica"
  | "agent"
  | "terminal"
  | "command"
  | "file"
  | "markdown"
  | "git-diff"
  /** Fork: revisão do projeto/worktree da sessão em resourceId. */
  | "revisao"
  | "git-graph"
  | "kanban"
  | "logs"
  | "docker-logs"
  /** Fork: página da web aberta de um link (`features/navegador`). `resourceId` = endereço. */
  | "web"
  /** Fork: fluxo ao vivo de uma conversa orquestradora (`features/orquestracao`). `resourceId` = id da conversa. */
  | "orquestracao";

export interface WorkspaceTab {
  id: string;
  kind: PaneKind;
  title: string;
  resourceId?: string;
  /** CLI que roda nesta aba (`claude`, `codex`, `cursor`). Só existe em aba de agente e é o que
   *  troca o ícone da aba pela marca do provider. */
  provider?: string;
  /** Fork: projeto da aba quando não é o dono do layout (agente novo aberto pelo "+" num painel que
   *  mostra conversa de outro projeto). Ausente = projeto do layout. */
  projectId?: string;
}

export interface PaneNode {
  type: "pane";
  id: string;
  tabs: WorkspaceTab[];
  activeTabId: string | null;
}

export interface SplitNode {
  type: "split";
  id: string;
  direction: "horizontal" | "vertical";
  ratio: number;
  first: LayoutNode;
  second: LayoutNode;
}

export type LayoutNode = PaneNode | SplitNode;

export interface Project {
  id: string;
  name: string;
  path: string;
  gitRoot: string | null;
  branch: string | null;
  composeFile: string | null;
  lastOpenedAt: string;
  layout: LayoutNode;
  activePaneId: string;
  maximizedPaneId: string | null;
}

export interface WorkspaceState {
  version: 1;
  id: string;
  name: string;
  projects: Project[];
  activeProjectId: string | null;
  updatedAt: string;
}

/** Vários workspaces independentes — cada um com seus próprios projects/layout. */
export interface WorkspaceCollection {
  version: 1;
  workspaces: WorkspaceState[];
  activeWorkspaceId: string | null;
}

export type WorkspaceAction =
  /** `id` opcional: quem precisa do id antes do reducer rodar (retomar conversa do Histórico). */
  | { type: "ADD_PROJECT"; path: string; id?: string }
  | { type: "SELECT_PROJECT"; projectId: string }
  | { type: "CLOSE_PROJECT"; projectId: string }
  | { type: "FOCUS_PANE"; paneId: string }
  | { type: "SELECT_TAB"; paneId: string; tabId: string }
  | { type: "CREATE_TAB"; paneId: string; kind?: PaneKind; title?: string; resourceId?: string; projectId?: string }
  | { type: "BIND_TAB_RESOURCE"; paneId: string; tabId: string; resourceId: string; title?: string; provider?: string }
  | { type: "ATTACH_TERMINAL"; sessionId: string; title: string }
  | { type: "RENAME_TAB_RESOURCE"; fromResourceId: string; toResourceId: string; title?: string }
  | { type: "CLOSE_TABS_BY_RESOURCE_PREFIX"; prefix: string }
  | { type: "CLOSE_TAB"; paneId: string; tabId: string }
  | { type: "MOVE_TAB"; sourcePaneId: string; targetPaneId: string; tabId: string }
  | {
      type: "SPLIT_WITH_TAB";
      sourcePaneId: string;
      targetPaneId: string;
      tabId: string;
      direction: SplitNode["direction"];
      position: "before" | "after";
    }
  | { type: "SPLIT_PANE"; paneId: string; direction: SplitNode["direction"] }
  | { type: "CLOSE_PANE"; paneId: string }
  | { type: "TOGGLE_MAXIMIZE"; paneId: string }
  | { type: "RESIZE_SPLIT"; splitId: string; ratio: number }
  | {
      type: "MOVE_PANE";
      sourcePaneId: string;
      targetPaneId: string;
      direction: SplitNode["direction"];
      position: "before" | "after";
    }
  | { type: "SWAP_PANE"; sourcePaneId: string; targetPaneId: string }
  | { type: "APPLY_LAYOUT_PRESET"; preset: LayoutPreset }
  /** Fork: conversa arrastada da lateral para um painel (`features/workspace/colocarConversa.ts`). */
  /** Fork: link clicado abre numa aba de navegador ao lado (`features/workspace/abrirLink.ts`). */
  | { type: "ABRIR_LINK"; url: string; title: string; paneId?: string }
  /** Fork: fluxo do orquestrador numa aba ao lado (`features/orquestracao/abrirFluxo.ts`). */
  | { type: "ABRIR_FLUXO"; orquestrador: string; title: string; paneId?: string }
  /** Fork: revisão ao lado do agente de origem. */
  | { type: "ABRIR_REVISAO"; sessionId: string; paneId: string; projectId?: string }
  | { type: "PLACE_SESSION"; sessionId: string; title: string; targetPaneId: string; zone: "center" | "left" | "right" | "top" | "bottom" };

/** Layouts prontos (estilo Snap Layouts do Windows) — `buildPresetLayout` em
 * `workspaceReducer.ts` monta a árvore de splits correspondente a partir das panes atuais. */
export type LayoutPreset = "columns-2" | "rows-2" | "grid-2x2" | "main-plus-side";

export type WorkspacesAction =
  | { type: "ADD_WORKSPACE"; name?: string }
  | { type: "SELECT_WORKSPACE"; workspaceId: string }
  | { type: "CLOSE_WORKSPACE"; workspaceId: string }
  | { type: "RENAME_WORKSPACE"; workspaceId: string; name: string }
  /** Pula pra uma sessão que pode estar em OUTRO workspace: troca o ativo, seleciona o projeto
   *  e abre a aba. Precisa viver aqui, no nível da coleção — as ações de `WorkspaceAction` são
   *  roteadas só pro workspace ativo (`workspacesReducer`), que é exatamente o que impede
   *  alcançar um agente de outro workspace. */
  | { type: "FOCUS_SESSION"; workspaceId: string; projectId: string; sessionId: string; title: string }
  | WorkspaceAction;
