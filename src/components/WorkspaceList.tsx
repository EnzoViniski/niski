import { useState } from "react";
import { BoxIcon, CloseIcon, EditIcon, FolderPlusIcon } from "./ui/PixelIcon";
import { ResumoDeAgentes } from "./ConversaStatus";
import type { TerminalSession } from "../features/terminal/terminalService";
import type { WorkspaceState } from "../types/workspace";
// Fork: confirmação antes de fechar abas e encerrar as sessões do workspace.
import { ConfirmarFechamentoDoWorkspace } from "../features/workspace/ConfirmarFechamentoDoWorkspace";

interface WorkspaceListProps {
  workspaces: WorkspaceState[];
  activeWorkspaceId: string | null;
  attentionByWorkspace: Record<string, number>;
  /** Conversas de agente de todos os projetos (resumo por workspace). */
  agentSessions?: TerminalSession[];
  naoVistas?: Set<string>;
  onSelect: (workspaceId: string) => void;
  onCreate: () => void;
  onClose: (workspaceId: string) => void;
  onRename: (workspaceId: string, name: string) => void;
}

/**
 * Lista de workspaces sempre visível, no lugar do antigo dropdown (`WorkspaceSwitcher`). O
 * dropdown escondia tanto a natureza da coisa ("Workspace 1" parecia um rótulo, não um seletor)
 * quanto o aviso de agente pendente, que é justamente o que precisa ser visto sem clique.
 *
 * Sem portal aqui: a máquina de `createPortal` + medição do switcher existia só porque um popover
 * dentro da `<aside>.glass` cai no contexto de empilhamento dela (ver `NotificationBell`). Uma
 * lista no fluxo normal não tem esse problema.
 */
export function WorkspaceList({
  workspaces,
  activeWorkspaceId,
  attentionByWorkspace,
  agentSessions = [],
  naoVistas = new Set(),
  onSelect,
  onCreate,
  onClose,
  onRename,
}: WorkspaceListProps) {
  const [editandoId, setEditandoId] = useState<string | null>(null);
  const [fechandoId, setFechandoId] = useState<string | null>(null);
  const fechando = workspaces.find((workspace) => workspace.id === fechandoId);

  function confirmarRename(workspaceId: string, valor: string) {
    setEditandoId(null);
    const nome = valor.trim();
    if (nome) onRename(workspaceId, nome);
  }

  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between px-3 pt-1">
        <p className="text-text-muted text-[10px] font-medium uppercase tracking-wider">Workspaces</p>
        <button
          data-nav-item
          type="button"
          onClick={onCreate}
          aria-label="Criar workspace"
          title="Criar workspace"
          className="text-text-muted hover:text-text-primary rounded-none p-0.5 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
        >
          <FolderPlusIcon className="w-3.5 h-3.5" aria-hidden="true" />
        </button>
      </div>

      {workspaces.map((workspace) => {
        const pendentes = attentionByWorkspace[workspace.id] ?? 0;
        const ativo = workspace.id === activeWorkspaceId;

        if (editandoId === workspace.id) {
          return (
            <div key={workspace.id} className="flex items-stretch px-0.5">
              <input
                type="text"
                autoFocus
                defaultValue={workspace.name}
                aria-label={`Renomear workspace ${workspace.name}`}
                onFocus={(event) => event.currentTarget.select()}
                onBlur={(event) => confirmarRename(workspace.id, event.currentTarget.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") event.currentTarget.blur();
                  else if (event.key === "Escape") {
                    event.stopPropagation();
                    setEditandoId(null);
                  }
                }}
                className="min-w-0 flex-1 bg-transparent px-2 py-1.5 text-xs text-text-primary outline-none focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
              />
            </div>
          );
        }

        return (
          <div key={workspace.id}
            className={`lateral-item group mx-1 flex items-stretch border-l-2 ring-1 ring-inset transition-colors ${
              ativo ? "lateral-item-ativo border-accent ring-accent/50" : "border-transparent ring-border-subtle/40 hover:ring-border-subtle"
            }`}>
            <button
              type="button"
              data-nav-item
              aria-current={ativo ? "page" : undefined}
              onClick={() => onSelect(workspace.id)}
              onDoubleClick={(event) => {
                event.stopPropagation();
                setEditandoId(workspace.id);
              }}
              title="Clique duplo para renomear"
              className="flex min-w-0 flex-1 items-center gap-2.5 px-2 py-2 text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
            >
              <span className="lateral-avatar grid h-8 w-8 shrink-0 place-items-center border border-border-subtle">
                <BoxIcon className="h-3.5 w-3.5 text-accent" aria-hidden />
              </span>
              <span className="min-w-0 flex-1">
                <span className={`block truncate text-xs leading-4 ${ativo ? "font-semibold text-text-primary" : "text-text-secondary"}`}>
                  {workspace.name}
                </span>
                <span aria-hidden className="mt-0.5 flex">
                  <ResumoDeAgentes
                    sessions={agentSessions.filter((session) => workspace.projects.some((project) => project.id === session.project_id))}
                    naoVistas={naoVistas}
                  />
                </span>
              </span>
              {pendentes > 0 && (
                <span
                  aria-label={`${pendentes} ${pendentes === 1 ? "agente esperando" : "agentes esperando"} em ${workspace.name}`}
                  className="badge-pulse grid h-4 min-w-4 shrink-0 place-items-center bg-danger px-1 text-[10px] font-bold leading-none text-bg-primary"
                >
                  {pendentes > 9 ? "9+" : pendentes}
                </span>
              )}
            </button>
            <button
              type="button"
              aria-label={`Renomear workspace ${workspace.name}`}
              title="Renomear"
              onClick={(event) => {
                event.stopPropagation();
                setEditandoId(workspace.id);
              }}
              className="grid w-7 shrink-0 place-items-center text-text-muted opacity-60 hover:text-accent hover:opacity-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
            >
              <EditIcon className="w-3 h-3" aria-hidden />
            </button>
            <button
              type="button"
              aria-label={`Fechar workspace ${workspace.name}`}
              title="Fechar workspace"
              onClick={() => setFechandoId(workspace.id)}
              className="grid w-7 shrink-0 place-items-center text-text-muted opacity-60 hover:text-danger hover:opacity-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
            >
              <CloseIcon className="h-3.5 w-3.5" aria-hidden="true" />
            </button>
          </div>
        );
      })}
      {fechando && <ConfirmarFechamentoDoWorkspace nome={fechando.name}
        onCancelar={() => setFechandoId(null)} onConfirmar={() => {
          setFechandoId(null);
          onClose(fechando.id);
        }} />}
    </div>
  );
}
