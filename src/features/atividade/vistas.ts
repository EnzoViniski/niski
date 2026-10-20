import type { LayoutNode, Project } from "../../types/workspace";
import { findPane } from "../workspace/workspaceReducer";
import type { TerminalSession } from "../terminal/terminalService";
import { useAtividadeVista } from "./atividadeService";

function abasVisiveis(no: LayoutNode): string[] {
  if (no.type !== "pane") return [...abasVisiveis(no.first), ...abasVisiveis(no.second)];
  const aba = no.tabs.find(item => item.id === no.activeTabId);
  return aba?.resourceId && ["agent", "terminal", "conversa-historica"].includes(aba.kind) ? [aba.resourceId] : [];
}
export function conversasVisiveis(projeto: Project | null, sessoes: TerminalSession[]): string[] {
  if (!projeto) return [];
  const raiz = projeto.maximizedPaneId ? findPane(projeto.layout, projeto.maximizedPaneId) ?? projeto.layout : projeto.layout;
  return abasVisiveis(raiz).map(id => sessoes.find(sessao => sessao.id === id)?.conversation_id ?? id);
}
export function useAtividadeDoDesktop(projeto: Project | null, sessoes: TerminalSession[], visivel: boolean, avulsa: string | null = null) {
  useAtividadeVista(avulsa ? [avulsa] : conversasVisiveis(projeto, sessoes), visivel);
}
