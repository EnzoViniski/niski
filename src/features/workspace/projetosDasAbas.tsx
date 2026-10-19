import { createContext, useContext } from "react";
import type { LayoutNode, WorkspaceState } from "../../types/workspace";
import { useSession } from "../terminal/sessionProviders";

/**
 * Fork: o que uma aba precisa saber do workspace inteiro, e não só do projeto que está na tela.
 *
 * - Conversa de **outro projeto** solta no layout (`colocarConversa.ts`): a aba mostra de qual
 *   projeto ela é, com a cor dele.
 * - Fechar aba encerra a sessão. Se outra aba (em qualquer projeto) ainda aponta para a mesma
 *   sessão, fechar esta não pode matar a conversa que continua aberta na outra.
 */
export interface ProjetosDasAbas {
  /** Projeto cujo layout está na tela. */
  projetoAtual: string | null;
  projetos: Record<string, { name: string; path: string }>;
  /** Abas presas à sessão em todos os projetos do workspace. */
  abasDaSessao: (sessionId: string) => number;
}

export const ProjetosDasAbasContext = createContext<ProjetosDasAbas>({
  projetoAtual: null,
  projetos: {},
  abasDaSessao: () => 1,
});

function contarRecursos(node: LayoutNode, contagem: Map<string, number>) {
  if (node.type === "split") {
    contarRecursos(node.first, contagem);
    contarRecursos(node.second, contagem);
    return;
  }
  for (const tab of node.tabs) {
    if (tab.resourceId) contagem.set(tab.resourceId, (contagem.get(tab.resourceId) ?? 0) + 1);
  }
}

export function montarProjetosDasAbas(workspace: WorkspaceState): ProjetosDasAbas {
  const contagem = new Map<string, number>();
  for (const project of workspace.projects) contarRecursos(project.layout, contagem);
  return {
    projetoAtual: workspace.activeProjectId,
    projetos: Object.fromEntries(workspace.projects.map((project) => [project.id, { name: project.name, path: project.path }])),
    abasDaSessao: (sessionId) => contagem.get(sessionId) ?? 0,
  };
}

export function useProjetosDasAbas() {
  return useContext(ProjetosDasAbasContext);
}

/** Matiz estável por projeto: o mesmo projeto tem sempre a mesma cor, em qualquer aba. */
export function corDoProjeto(projectId: string): string {
  let hash = 0;
  for (const letra of projectId) hash = (hash * 31 + letra.charCodeAt(0)) >>> 0;
  return `hsl(${hash % 360} 70% 62%)`;
}

export interface ProjetoDeFora { id: string; name: string; cor: string }

/** Projeto da sessão quando ele **não** é o que está na tela; `null` para sessão do próprio projeto
 *  (ou ainda desconhecida). */
export function useProjetoDeFora(sessionId: string | null | undefined): ProjetoDeFora | null {
  const { projetoAtual, projetos } = useProjetosDasAbas();
  const projectId = useSession(sessionId)?.project_id;
  if (!projectId || !projetoAtual || projectId === projetoAtual) return null;
  return { id: projectId, name: projetos[projectId]?.name ?? "outro projeto", cor: corDoProjeto(projectId) };
}

/** Etiqueta "📁 projeto" das abas e da prévia. */
export function EtiquetaDoProjeto({ projeto, className = "" }: { projeto: ProjetoDeFora; className?: string }) {
  return (
    <span className={`etiqueta-projeto ${className}`} style={{ "--cor-projeto": projeto.cor } as React.CSSProperties}
      title={`Conversa do projeto ${projeto.name}`}>
      {projeto.name}
    </span>
  );
}
