import type { Project, WorkspacesAction } from "../../types/workspace";
import type { AgentView } from "../../types/settings";
import type { KanbanAction, KanbanState } from "../../types/kanban";
import type { AttentionItem } from "../terminal/useAttention";
import { AgentUsageStatus } from "../terminal/AgentUsageStatus";
import { GitBranchStatus, GitContaStatus } from "../git/GitBranchStatus";
import { findPane } from "./workspaceReducer";
import { PaneTree } from "./PaneTree";
import { useWorkspaceKeymap } from "./useWorkspaceKeymap";

interface WorkspaceViewProps {
  project: Project | null;
  engineOnline: boolean;
  attention: AttentionItem[];
  fileSaveMode: "auto" | "manual";
  /** Aba de agente: o chat (como na web) ou o terminal cru. Ver `AgentView` em `types/settings`. */
  agentView: AgentView;
  kanban: KanbanState;
  kanbanDispatch: React.Dispatch<KanbanAction>;
  dispatch: React.Dispatch<WorkspacesAction>;
}

export function WorkspaceView({
  project,
  engineOnline,
  attention,
  fileSaveMode,
  agentView,
  kanban,
  kanbanDispatch,
  dispatch,
}: WorkspaceViewProps) {
  useWorkspaceKeymap(project, attention, dispatch);

  if (!project) {
    return (
      <div className="h-full flex items-center justify-center text-center px-6">
        <div className="space-y-2">
          <p className="pixel-text text-text-primary text-xs">READY PLAYER ONE</p>
          <p className="text-text-secondary text-sm">Nenhum painel aberto</p>
          <p className="text-text-muted text-xs">Adicione um projeto na barra lateral para começar.</p>
        </div>
      </div>
    );
  }

  const paneCount = countPanes(project.layout);
  // Aba em foco: é ela que decide de quem é o uso no rodapé.
  const panePrincipal = findPane(project.layout, project.activePaneId);
  const abaAtiva = panePrincipal?.tabs.find((tab) => tab.id === panePrincipal.activeTabId) ?? null;

  return (
    <div className="h-full min-h-0 flex flex-col">
      {/* Nome, caminho e layouts do projeto ficam na barra superior (`TitleBar`). */}
      <h1 className="sr-only">{project.name}</h1>
      <div className="flex flex-1 min-h-0 p-2">
        <PaneTree
          node={project.layout}
          activePaneId={project.activePaneId}
          maximizedPaneId={project.maximizedPaneId}
          paneCount={paneCount}
          projectId={project.id}
          projectPath={project.path}
          fileSaveMode={fileSaveMode}
        agentView={agentView}
          kanban={kanban}
          kanbanDispatch={kanbanDispatch}
          dispatch={dispatch}
        />
      </div>

      <RodapeDoProjeto projectPath={project.path} engineOnline={engineOnline} activeSessionId={abaAtiva?.resourceId ?? null} />
    </div>
  );
}

/** Conta do git, branch, uso dos agentes e engine — no workspace e nos fluxos. */
export function RodapeDoProjeto({ projectPath, engineOnline, activeSessionId }: { projectPath: string | null; engineOnline: boolean; activeSessionId: string | null }) {
  return (
    <footer className="h-7 shrink-0 px-3 flex items-center justify-between gap-3 overflow-hidden whitespace-nowrap border-t-2 border-border-subtle bg-bg-elevated text-[10px] text-text-muted">
      <span className="flex min-w-0 max-w-[45%] shrink-0 items-center gap-3 overflow-hidden">
        {projectPath && <GitContaStatus projectPath={projectPath} />}
        {projectPath && <GitBranchStatus projectPath={projectPath} />}
      </span>
      <span className="flex min-w-0 flex-1 md:flex-initial items-center justify-between md:justify-end gap-3 lg:gap-4 overflow-hidden">
        <AgentUsageStatus engineOnline={engineOnline} activeSessionId={activeSessionId} />
        <span className="shrink-0">ENGINE: {engineOnline ? "ONLINE" : "CONNECTING"}</span>
      </span>
    </footer>
  );
}

function countPanes(node: Project["layout"]): number {
  return node.type === "pane" ? 1 : countPanes(node.first) + countPanes(node.second);
}
