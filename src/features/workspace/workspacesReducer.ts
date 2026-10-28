import { emptyWorkspace } from "../../services/workspaceService";
import type { WorkspaceCollection, WorkspacesAction } from "../../types/workspace";
import { collectResourceIds, workspaceReducer } from "./workspaceReducer";
import { abrirEmPainelProprio } from "./abrirEmPainelProprio"; // Conversa da lateral em painel próprio.
// Restaurar backup e adotar projetos de sessões órfãs/conversas recentes.
import { adotarProjetos, restaurarColecao } from "./recuperarWorkspace";
// A troca de agente atualiza também a marca salva nas abas.
import { sincronizarIdentidadeDasAbas } from "./identidadeDasAbas";
// "Na tela" alcança uma aba de qualquer workspace/projeto numa transição só.
import { focarTelaAberta } from "../projects/telasAbertas";

let sequence = 0;

function nextWorkspaceId(): string {
  sequence += 1;
  return `workspace-${Date.now().toString(36)}-${sequence.toString(36)}`;
}

export function workspacesReducer(state: WorkspaceCollection, action: WorkspacesAction): WorkspaceCollection {
  switch (action.type) {
    case "FOCUS_OPEN_TAB":
      return focarTelaAberta(state, action.destino);
    case "SYNC_SESSION_IDENTITIES":
      return sincronizarIdentidadeDasAbas(state, action.sessions);
    case "ADD_WORKSPACE": {
      const workspace = {
        ...emptyWorkspace(),
        id: nextWorkspaceId(),
        name: action.name?.trim() || `Workspace ${state.workspaces.length + 1}`,
      };
      return { ...state, workspaces: [...state.workspaces, workspace], activeWorkspaceId: workspace.id };
    }
    case "RESTAURAR_COLECAO":
      return restaurarColecao(action.colecao);
    case "ADOTAR_PROJETOS":
      return adotarProjetos(state, action.projetos);
    case "SELECT_WORKSPACE":
      return state.workspaces.some((workspace) => workspace.id === action.workspaceId)
        ? { ...state, activeWorkspaceId: action.workspaceId }
        : state;
    case "CLOSE_WORKSPACE": {
      const remaining = state.workspaces.filter((workspace) => workspace.id !== action.workspaceId);
      if (remaining.length === state.workspaces.length) return state;
      // Nunca deixa o app sem workspace nenhum.
      const workspaces = remaining.length > 0 ? remaining : [emptyWorkspace()];
      const activeWorkspaceId =
        state.activeWorkspaceId === action.workspaceId ? workspaces[0].id : (state.activeWorkspaceId ?? workspaces[0].id);
      return { ...state, workspaces, activeWorkspaceId };
    }
    case "RENAME_WORKSPACE": {
      const name = action.name.trim();
      return {
        ...state,
        workspaces: state.workspaces.map((workspace) =>
          workspace.id === action.workspaceId ? { ...workspace, name: name || workspace.name } : workspace
        ),
      };
    }
    case "OPEN_SESSION_OWN_PANE": {
      const alvo = state.workspaces.find((workspace) => workspace.id === action.workspaceId);
      if (!alvo?.projects.some((project) => project.id === action.projectId)) return state;
      const aberto = abrirEmPainelProprio(alvo, action.projectId, action.sessionId, action.title);
      return { ...state, activeWorkspaceId: action.workspaceId, workspaces: state.workspaces.map((workspace) => (workspace.id === action.workspaceId ? aberto : workspace)) };
    }
    case "FOCUS_SESSION": {
      const target = state.workspaces.find((workspace) => workspace.id === action.workspaceId);
      // Sem o projeto dentro do workspace alvo, o SELECT_PROJECT abaixo viraria no-op e o
      // ATTACH_TERMINAL grudaria a sessão no projeto errado — o bug que essa ação existe pra matar.
      if (!target?.projects.some((project) => project.id === action.projectId)) return state;
      // Conversa já aberta no layout de outro projeto (arrastada para lá pela lateral) é
      // focada onde está — abrir de novo no projeto dela deixaria duas abas na mesma sessão.
      const ondeEsta = target.projects.find((project) => collectResourceIds(project.layout).includes(action.sessionId));
      const selected = workspaceReducer(target, { type: "SELECT_PROJECT", projectId: ondeEsta?.id ?? action.projectId });
      // Ordem importa: ATTACH_TERMINAL passa por `updateActiveProject`, então só acha a pane certa
      // depois que o projeto alvo já é o ativo.
      const focused = workspaceReducer(selected, {
        type: "ATTACH_TERMINAL",
        sessionId: action.sessionId,
        title: action.title,
      });
      return {
        ...state,
        activeWorkspaceId: action.workspaceId,
        workspaces: state.workspaces.map((workspace) =>
          workspace.id === action.workspaceId ? focused : workspace
        ),
      };
    }
    default: {
      if (!state.activeWorkspaceId) return state;
      return {
        ...state,
        workspaces: state.workspaces.map((workspace) =>
          workspace.id === state.activeWorkspaceId ? workspaceReducer(workspace, action) : workspace
        ),
      };
    }
  }
}
