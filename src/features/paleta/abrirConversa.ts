import type { Conversation } from "../../mobile/api";
import type { Project, WorkspacesAction, WorkspaceState } from "../../types/workspace";
import type { TerminalSession } from "../terminal/terminalService";
import { latestPerConversation } from "../terminal/useAttention";
import { ondeEsta } from "../orquestracao/abrirFluxo";

/** Conversa viva segue a lateral; histórico já aberto ganha foco em vez de outra aba. */
export function acoesParaAbrirConversa(
  conversa: Conversation, workspaces: WorkspaceState[], sessions: TerminalSession[], projetoAtual: Project | null,
): WorkspacesAction[] {
  const sessao = latestPerConversation(sessions).find((item) => item.conversation_id === conversa.id);
  const dona = workspaces.find((item) => item.projects.some((projeto) => projeto.id === (sessao?.project_id ?? conversa.project_id)));
  if (sessao && dona) return [{ type: "FOCUS_SESSION", workspaceId: dona.id, projectId: sessao.project_id, sessionId: sessao.id, title: conversa.title }];
  for (const workspace of workspaces) for (const projeto of workspace.projects) {
    const aberta = ondeEsta({ ...workspace, activeProjectId: projeto.id }, "conversa-historica", conversa.id);
    if (aberta) return [
      { type: "SELECT_WORKSPACE", workspaceId: workspace.id }, { type: "SELECT_PROJECT", projectId: projeto.id },
      { type: "SELECT_TAB", ...aberta },
    ];
  }
  const projeto = dona?.projects.find((item) => item.id === conversa.project_id) ?? projetoAtual;
  if (!projeto) return [];
  return [
    ...(dona ? [{ type: "SELECT_WORKSPACE" as const, workspaceId: dona.id }, { type: "SELECT_PROJECT" as const, projectId: projeto.id }] : []),
    { type: "CREATE_TAB", paneId: projeto.activePaneId, kind: "conversa-historica", title: conversa.title, resourceId: conversa.id },
  ];
}
