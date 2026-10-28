// A sessão atual manda na marca da conversa; a cópia na aba só serve sem engine.
import type { LayoutNode, WorkspaceCollection, WorkspaceTab } from "../../types/workspace";

export interface IdentidadeDaSessao { id: string; provider?: string | null; name?: string }

export function providerDaAba(tab: WorkspaceTab, sessao: Pick<IdentidadeDaSessao, "provider"> | null | undefined) {
  return sessao?.provider ?? tab.provider;
}

/** Espelha só metadados em todos os layouts, inclusive projetos/workspaces que estão fora da tela.
 *  Preserva referências no poll sem mudança para não salvar/re-renderizar a cada segundo. */
export function sincronizarIdentidadeDasAbas(colecao: WorkspaceCollection, sessoes: IdentidadeDaSessao[]): WorkspaceCollection {
  const porId = new Map(sessoes.map((sessao) => [sessao.id, sessao]));
  function atualizar(node: LayoutNode): LayoutNode {
    if (node.type === "split") {
      const first = atualizar(node.first);
      const second = atualizar(node.second);
      return first === node.first && second === node.second ? node : { ...node, first, second };
    }
    let mudou = false;
    const tabs = node.tabs.map((tab) => {
      const sessao = tab.resourceId ? porId.get(tab.resourceId) : undefined;
      if (!sessao || (tab.kind !== "agent" && tab.kind !== "terminal")) return tab;
      const title = sessao.name || tab.title;
      const provider = providerDaAba(tab, sessao);
      if (title === tab.title && provider === tab.provider) return tab;
      mudou = true;
      return { ...tab, title, provider };
    });
    return mudou ? { ...node, tabs } : node;
  }
  let mudou = false;
  const workspaces = colecao.workspaces.map((workspace) => {
    let mudouProjeto = false;
    const projects = workspace.projects.map((project) => {
      const layout = atualizar(project.layout);
      if (layout === project.layout) return project;
      mudouProjeto = true;
      return { ...project, layout };
    });
    if (!mudouProjeto) return workspace;
    mudou = true;
    return { ...workspace, projects, updatedAt: new Date().toISOString() };
  });
  return mudou ? { ...colecao, workspaces } : colecao;
}
