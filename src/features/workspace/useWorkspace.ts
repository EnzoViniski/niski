import { useEffect, useReducer, useRef } from "react";
import { loadWorkspaces, saveWorkspaces } from "../../services/workspaceService";
import { listTerminalSessions, publishWorkspace, restartTerminal, type PublishedTheme } from "../terminal/terminalService";
// Fork (atualização automática): religar as abas visíveis depois do reinício de atualização.
import { consumirReinicioDeAtualizacao } from "../../lib/atualizacaoRemota";
import { retomarAbasVisiveisAposAtualizar } from "./recuperarWorkspace";
import { workspacesReducer } from "./workspacesReducer";

/** `tema` é publicado junto com os projetos para o celular sair com a mesma cara do PC. */
export function useWorkspace(tema: PublishedTheme | null = null) {
  // Despacho direto no reducer. A animação de conversa mora no painel (animarConversa.ts); a
  // View Transitions API da 0.9.2 travou os cliques da janela e não volta.
  const [collection, dispatch] = useReducer(workspacesReducer, undefined, loadWorkspaces);
  const publicado = useRef<string | null>(null);

  useEffect(() => {
    saveWorkspaces(collection);
  }, [collection]);

  // Fork (atualização automática): só na abertura que veio de "Atualizar agora"/"Reiniciar app e
  // engine"; o sinalizador é consumido aqui, uma vez. Abertura normal não muda.
  const inicial = useRef(collection);
  useEffect(() => {
    if (!consumirReinicioDeAtualizacao()) return;
    void retomarAbasVisiveisAposAtualizar(inicial.current, { listar: listTerminalSessions, reiniciar: restartTerminal });
  }, []);

  // Publica os projetos no engine para o celular enxergar. A projeção serializada **é** o debounce:
  // arrastar aba, mexer em split ou trocar de tab produz a mesma string e não gera IPC nenhum. Só
  // abrir, fechar ou renomear projeto publica de fato.
  useEffect(() => {
    const projetos = [
      ...new Map(
        collection.workspaces.flatMap((workspace) =>
          workspace.projects.map((project) => [project.id, { id: project.id, name: project.name, path: project.path }] as const)
        )
      ).values(),
    ];
    const projecao = JSON.stringify([projetos, tema]);
    if (projecao === publicado.current) return;
    publicado.current = projecao;
    // Engine fora do ar é estado normal aqui: o celular fica com a última lista gravada em disco.
    void publishWorkspace(projetos, tema).catch(() => undefined);
  }, [collection, tema]);

  const workspace =
    collection.workspaces.find((item) => item.id === collection.activeWorkspaceId) ?? collection.workspaces[0];

  const activeProject =
    workspace.projects.find((project) => project.id === workspace.activeProjectId) ?? null;

  return {
    workspace,
    activeProject,
    dispatch,
    workspaces: collection.workspaces,
    activeWorkspaceId: collection.activeWorkspaceId,
  };
}
