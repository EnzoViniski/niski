import {
  BookIcon,
  ChatIcon,
  ChevronDownIcon,
  CloseIcon,
  CopyIcon,
  DockerIcon,
  FolderIcon,
  FolderPlusIcon as FolderPlus,
  GitBranchIcon,
  KanbanIcon,
  ListIcon,
  ReloadIcon,
  SlidersIcon as Sliders,
  TerminalIcon,
} from "./ui/PixelIcon";
import { EtiquetaDoStatus, IconeDoStatus, rotuloDoStatus, statusDaConversa } from "./ConversaStatus";
import { conversationKey } from "../features/terminal/useAttention";
import { useEffect, useRef, useState } from "react";
import gsap from "gsap";
import { ProjectPicker } from "../features/projects/ProjectPicker";
import { EditableLabel } from "./ui/EditableLabel";
import { renameConversation } from "../features/terminal/terminalService";
import { useStagger } from "../lib/motion";
import { AgentIcon } from "./ui/AgentIcon";
import { OmniLogo } from "./ui/OmniLogo";
import { NotificationBell } from "./NotificationBell";
import type { SettingsSection } from "../hooks/useNotifications";
import type { KanbanState } from "../types/kanban";
import type { Project, WorkspaceState } from "../types/workspace";
import type { TerminalSession } from "../features/terminal/terminalService";
import type { AttentionItem } from "../features/terminal/useAttention";
import { WorkspaceList } from "./WorkspaceList";
import { AttentionPanel } from "./AttentionPanel";
import { FileTree } from "./FileTree";
import { GitPanel } from "./GitPanel";
import { DockerPanel } from "./DockerPanel";
import { KanbanPanel } from "./KanbanPanel";
import { SkillsList } from "./SkillsList";
/* O mesmo arquivo que o Tauri usa como ícone do app (tauri.conf.json →
   bundle.icon). Importado em vez de copiado para src/assets: uma cópia
   dessincronizaria do ícone real na próxima troca de marca. */

/** Seções do spec 7.3. Vazias por enquanto — Workspace/Project/Pane/Session
 * (Fase 1) é quem vai preencher cada uma com dados de verdade. */
const SECTIONS = ["CHATS", "TERMINALS", "COMMANDS", "DOCKER", "FILES", "SKILLS", "GIT", "KANBAN"] as const;

const SECTION_ICON: Record<(typeof SECTIONS)[number], React.ComponentType<{ className?: string; "aria-hidden"?: boolean }>> = {
  CHATS: ChatIcon,
  TERMINALS: TerminalIcon,
  COMMANDS: ListIcon,
  DOCKER: DockerIcon,
  FILES: FolderIcon,
  SKILLS: BookIcon,
  GIT: GitBranchIcon,
  KANBAN: KanbanIcon,
};

interface SidebarProps {
  workspaces: WorkspaceState[];
  activeWorkspaceId: string | null;
  onSelectWorkspace: (workspaceId: string) => void;
  onCreateWorkspace: () => void;
  onCloseWorkspace: (workspaceId: string) => void;
  onRenameWorkspace: (workspaceId: string, name: string) => void;
  attention: AttentionItem[];
  attentionByWorkspace: Record<string, number>;
  onFocusSession: (item: AttentionItem) => void;
  onDismissAttention: (item: AttentionItem) => void;
  /** Terminal novo no projeto ativo rodando `command` (logs/shell de container). */
  onRunInTerminal: (title: string, command: string) => void;
  projects: Project[];
  activeProjectId: string | null;
  activeProjectPath: string | null;
  onOpenFile: (path: string, kind: "file" | "markdown") => void;
  onFileRenamed: (fromPath: string, toPath: string, title: string) => void;
  onPathDeleted: (path: string) => void;
  terminalSessions: TerminalSession[];
  showSettings: boolean;
  showHistory: boolean;
  onOpenHistory: () => void;
  onHome: () => void;
  onAddProject: (path: string) => void;
  onSelectProject: (projectId: string) => void;
  onCloseProject: (projectId: string) => void;
  onAttachTerminal: (sessionId: string, title: string) => void;
  onCloseTerminal: (sessionId: string) => void;
  onDuplicateTerminal: (sessionId: string) => void;
  onRestartTerminal: (sessionId: string) => void;
  onOpenSettings: (secao?: SettingsSection) => void;
  onOpenGitDiff: (file: string, staged: boolean) => void;
  onOpenGitGraph: () => void;
  kanban: KanbanState;
  onOpenKanban: () => void;
}

export function Sidebar({
  workspaces,
  activeWorkspaceId,
  onSelectWorkspace,
  onCreateWorkspace,
  onCloseWorkspace,
  onRenameWorkspace,
  attention,
  attentionByWorkspace,
  onFocusSession,
  onDismissAttention,
  onRunInTerminal,
  projects,
  activeProjectId,
  activeProjectPath,
  onOpenFile,
  onFileRenamed,
  onPathDeleted,
  terminalSessions,
  showSettings,
  showHistory,
  onOpenHistory,
  onHome,
  onAddProject,
  onSelectProject,
  onCloseProject,
  onAttachTerminal,
  onCloseTerminal,
  onDuplicateTerminal,
  onRestartTerminal,
  onOpenSettings,
  onOpenGitDiff,
  onOpenGitGraph,
  kanban,
  onOpenKanban,
}: SidebarProps) {
  const navRef = useStagger<HTMLElement>("[data-nav-item]");
  const naoVistas = new Set(attention.map((item) => item.conversationKey));

  const [pickerAberto, setPickerAberto] = useState(false);

  return (
    <aside className="glass glass-strong w-64 shrink-0 flex flex-col h-screen sticky top-0 rounded-none border-y-0 border-l-0">
      {/* Logo + sino. O sino é irmão do botão, não filho: <button> dentro de
          <button> é HTML inválido e o clique de um engole o do outro. */}
      <div className="px-4 py-5 border-b border-border-subtle/60 flex items-center gap-2">
        <button
          onClick={onHome}
          className="flex items-center gap-2.5 text-left min-w-0 flex-1 rounded-none focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
        >
          <OmniLogo className="neon-glow w-7 h-7 shrink-0" />
          <div className="min-w-0">
            <p className="pixel-text text-text-primary text-xs leading-tight truncate">OMNI AGENTS</p>
          </div>
        </button>

        <NotificationBell onOpenSettings={onOpenSettings} />
      </div>

      {/* Nav */}
      <nav ref={navRef} className="px-2 py-3 flex-1 min-h-0 overflow-y-auto overflow-x-hidden space-y-3">
        <AttentionPanel items={attention} onFocusSession={onFocusSession} onDismiss={onDismissAttention} />

        <WorkspaceList
          workspaces={workspaces}
          activeWorkspaceId={activeWorkspaceId}
          attentionByWorkspace={attentionByWorkspace}
          onSelect={onSelectWorkspace}
          onCreate={onCreateWorkspace}
          onClose={onCloseWorkspace}
          onRename={onRenameWorkspace}
        />

        <div className="space-y-0.5">
          <div className="flex items-center justify-between px-3 pt-1">
            <p className="text-text-muted text-[10px] font-medium uppercase tracking-wider">Projects</p>
            <button
              data-nav-item
              onClick={() => setPickerAberto(true)}
              aria-label="Adicionar projeto"
              className="text-text-muted hover:text-text-primary rounded-none p-0.5 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
            >
              <FolderPlus className="w-3.5 h-3.5" aria-hidden="true" />
            </button>
          </div>
          {projects.length === 0 ? (
            <p data-nav-item className="text-text-muted text-xs px-3 py-1.5">
              Nenhum projeto ainda.
            </p>
          ) : (
            projects.map((project) => (
              <div key={project.id} className="group flex items-stretch">
                <button
                  type="button"
                  data-nav-item
                  aria-current={project.id === activeProjectId ? "page" : undefined}
                  onClick={() => onSelectProject(project.id)}
                  className={[
                    "min-w-0 flex-1 px-3 py-2 text-left text-xs truncate border-l-2 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent",
                    project.id === activeProjectId
                      ? "border-accent bg-accent-muted text-text-primary"
                      : "border-transparent text-text-secondary hover:text-text-primary hover:bg-overlay/[0.07]",
                  ].join(" ")}
                  title={project.path}
                >
                  <FolderIcon className="mr-2 inline w-3 h-3 shrink-0 text-accent" aria-hidden />
                  {project.name}
                </button>
                <button
                  type="button"
                  aria-label={`Fechar projeto ${project.name}`}
                  title="Fechar projeto"
                  onClick={() => onCloseProject(project.id)}
                  className="grid w-8 shrink-0 place-items-center text-text-muted opacity-60 hover:text-danger hover:opacity-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
                >
                  <CloseIcon className="h-3.5 w-3.5" aria-hidden="true" />
                </button>
              </div>
            ))
          )}
        </div>

        {SECTIONS.map((section) => (
          <SidebarSection key={section} label={section} icon={SECTION_ICON[section]} disabled={!activeProjectId}>
            {/* CHATS = conversas com um agente de IA; TERMINALS = shells (npm run dev, git…). */}
            {(section === "CHATS" || section === "TERMINALS")
              && terminalSessions.some((session) => Boolean(session.provider) === (section === "CHATS"))
              ? terminalSessions.filter((session) => Boolean(session.provider) === (section === "CHATS")).map((session) => (
                  <div key={session.id} className="group flex items-stretch pl-6">
                    <SessionLabel
                      session={session}
                      naoVista={naoVistas.has(conversationKey(session))}
                      onAttach={onAttachTerminal}
                    />
                    {/* Ações só no hover/foco: fora dele o espaço é do status da conversa. */}
                    <div className="hidden shrink-0 items-stretch group-focus-within:flex group-hover:flex">
                    <button
                      type="button"
                      aria-label={session.provider ? `Bifurcar conversa ${session.name}` : `Duplicar terminal ${session.name}`}
                      title={session.provider
                        ? "Bifurcar: abre uma conversa nova com todo o histórico desta, que segue independente"
                        : "Duplicar: abre outro terminal na mesma pasta"}
                      onClick={() => onDuplicateTerminal(session.id)}
                      className="grid w-6 shrink-0 place-items-center text-text-muted opacity-60 hover:text-text-primary hover:opacity-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
                    >
                      {session.provider ? <GitBranchIcon className="h-3.5 w-3.5" aria-hidden="true" /> : <CopyIcon className="h-3.5 w-3.5" aria-hidden="true" />}
                    </button>
                    <button
                      type="button"
                      aria-label={`Reiniciar agente ${session.name}`}
                      title="Reiniciar sessão"
                      onClick={() => onRestartTerminal(session.id)}
                      className="grid w-6 shrink-0 place-items-center text-text-muted opacity-60 hover:text-text-primary hover:opacity-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
                    >
                      <ReloadIcon className="h-3.5 w-3.5" aria-hidden="true" />
                    </button>
                    <button
                      type="button"
                      aria-label={`Fechar agente ${session.name}`}
                      title="Encerrar e remover agente"
                      onClick={() => onCloseTerminal(session.id)}
                      className="grid w-6 shrink-0 place-items-center text-text-muted opacity-60 hover:text-danger hover:opacity-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
                    >
                      <CloseIcon className="h-3.5 w-3.5" aria-hidden="true" />
                    </button>
                    </div>
                  </div>
                ))
              : section === "FILES"
              ? (
                  <FileTree
                    projectPath={activeProjectPath}
                    onOpenFile={onOpenFile}
                    onFileRenamed={onFileRenamed}
                    onPathDeleted={onPathDeleted}
                  />
                )
              : section === "DOCKER"
              ? <DockerPanel onRunInTerminal={onRunInTerminal} />
              : section === "SKILLS"
              ? <SkillsList projectPath={activeProjectPath} onOpenFile={onOpenFile} />
              : section === "GIT"
              ? <GitPanel projectPath={activeProjectPath} onOpenDiff={onOpenGitDiff} onOpenGraph={onOpenGitGraph} />
              : section === "KANBAN"
              ? <KanbanPanel projectId={activeProjectId} kanban={kanban} onOpenBoard={onOpenKanban} />
              : undefined}
          </SidebarSection>
        ))}
      </nav>

      {/* Footer */}
      <div className="px-2 py-3 border-t border-border-subtle/60 space-y-0.5">
        <NavItem
          active={showHistory}
          onClick={onOpenHistory}
          icon={<ChatIcon className="w-4 h-4" aria-hidden="true" />}
          label="Histórico"
        />
        <NavItem
          active={showSettings}
          onClick={() => onOpenSettings()}
          icon={<Sliders className="w-4 h-4" aria-hidden="true" />}
          label="Configurações"
        />
      </div>

      {pickerAberto && (
        <ProjectPicker
          onClose={() => setPickerAberto(false)}
          onPick={(path) => {
            setPickerAberto(false);
            onAddProject(path);
          }}
        />
      )}
    </aside>
  );
}

/** Nome da sessão na lista, renomeável no duplo clique — é o nome da conversa, o mesmo que a aba e
 *  o celular mostram. */
function SessionLabel({
  session,
  naoVista,
  onAttach,
}: {
  session: TerminalSession;
  /** Há um aviso desta conversa que você ainda não viu — é o que separa "finalizado" de neutro. */
  naoVista: boolean;
  onAttach: (sessionId: string, title: string) => void;
}) {
  const status = statusDaConversa(session, naoVista);
  const [editando, setEditando] = useState(false);

  if (editando) {
    return (
      <span className="min-w-0 flex-1 py-1">
        <EditableLabel
          value={session.name}
          editing
          onEditingChange={setEditando}
          label={`Renomear conversa ${session.name}`}
          onCommit={(nome) => {
            if (session.conversation_id) void renameConversation(session.conversation_id, nome).catch(() => undefined);
          }}
        />
      </span>
    );
  }

  return (
    <button
      type="button"
      onClick={() => onAttach(session.id, session.name)}
      onDoubleClick={() => setEditando(true)}
      className={`flex min-w-0 flex-1 items-center gap-1.5 py-1.5 pr-1 text-left text-xs hover:text-text-primary ${
        status === "neutro" ? "text-text-muted" : "text-text-secondary"
      }`}
      title={`${session.name}${rotuloDoStatus(status) ? ` · ${rotuloDoStatus(status)}` : ""} — duplo clique renomeia`}
    >
      <IconeDoStatus status={status} />
      <AgentIcon provider={session.provider} size={12} className="shrink-0" />
      <span className="min-w-0 flex-1 truncate">{session.name}</span>
      <EtiquetaDoStatus status={status} />
    </button>
  );
}

function SidebarSection({
  label,
  icon: Icon,
  disabled,
  children,
}: {
  label: string;
  icon?: React.ComponentType<{ className?: string; "aria-hidden"?: boolean }>;
  disabled: boolean;
  children?: React.ReactNode;
}) {
  const [open, setOpen] = useState(true);
  return (
    <div className="space-y-0.5">
      <button
        type="button"
        data-nav-item
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((value) => !value)}
        className="w-full flex items-center gap-2 text-text-muted text-[10px] font-medium uppercase tracking-wider px-3 pt-1 disabled:opacity-40 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
      >
        <ChevronDownIcon
          className={`h-3 w-3 shrink-0 transition-transform ${open ? "" : "-rotate-90"}`}
          aria-hidden="true"
        />
        {Icon && <Icon className="w-3 h-3 shrink-0" aria-hidden />}
        {label}
      </button>
      {open && !disabled && (children ?? <p data-nav-item className="text-text-muted text-xs px-6 py-1.5">Vazio</p>)}
    </div>
  );
}

function NavItem({
  active,
  onClick,
  icon,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
}) {
  const barRef = useRef<HTMLSpanElement>(null);

  // Barra de acento cresce ao virar ativo; some ao sair.
  useEffect(() => {
    const bar = barRef.current;
    if (!bar) return;
    gsap.to(bar, {
      scaleY: active ? 1 : 0,
      opacity: active ? 1 : 0,
      duration: 0.32,
      ease: active ? "back.out(2)" : "power2.in",
    });
  }, [active]);

  return (
    <button
      data-nav-item
      onClick={onClick}
      className={[
        "relative w-full flex items-center gap-2.5 px-3 py-2 rounded-none text-sm transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent",
        active
          ? "glass text-text-primary font-medium"
          : "text-text-secondary hover:text-text-primary hover:bg-overlay/[0.07]",
      ].join(" ")}
    >
      <span
        ref={barRef}
        aria-hidden="true"
        className={[
          "absolute left-0 top-1.5 bottom-1.5 w-0.5 rounded-none origin-center opacity-0 scale-y-0",
          active ? "neon-bar" : "bg-border",
        ].join(" ")}
      />
      <span className={active ? "neon" : ""}>{icon}</span>
      <span className="truncate">{label}</span>
    </button>
  );
}
