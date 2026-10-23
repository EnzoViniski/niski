// Fork: publica os mesmos handlers dos botões para a paleta.
import { useAcoesDaPaleta } from "../paleta/acoes";
import { ConversaDoHistorico } from "../paleta/ConversaDoHistorico";
import { GlifoColunas, GlifoExpandir, GlifoFechar, GlifoLinhas, GlifoRecolher } from "../../components/ui/PixelGlyph";
import { Button } from "../../components/ui";
import type { AgentView } from "../../types/settings";
import { Component, useCallback, useEffect, useRef, useState } from "react";
import type { ErrorInfo, ReactNode } from "react";
import type { PaneNode, SplitNode, WorkspaceAction } from "../../types/workspace";
import type { KanbanAction, KanbanState } from "../../types/kanban";
import { isImagePath } from "../files/filesService";
// Fork: editor (Monaco) carregado só quando abre a primeira aba de arquivo.
import { FilePane } from "../files/FilePaneTardio";
import { ImagePane } from "../files/ImagePane";
import { MarkdownPane } from "../files/MarkdownPane";
import { GitDiffPane } from "../git/GitDiffPane";
// Fork: revisão ligada ao projeto/worktree da sessão.
import { RevisaoPane } from "../revisao/RevisaoPane";
import { GitGraphPane } from "../git/GitGraphPane";
import { KanbanBoard } from "../kanban/KanbanBoard";
import { TerminalPane } from "../terminal/TerminalPane";
import { AbaDeAgente } from "../conversa/AbaDeAgente";
// Fork: aba de navegador (link clicado).
import { NavegadorPane } from "../navegador/NavegadorPane";
// Fork: fluxo ao vivo do orquestrador (React Flow carregado tardio).
import { FluxoAoVivo } from "../orquestracao/FluxoAoVivoTardio";
import { closeTerminal } from "../terminal/terminalService";
import { PANE_DRAG_TYPE } from "./tabDrag";
import { TAB_DRAG_EVENT, type DropZone, type TabDragState } from "./tabPointerDrag";
import { TabDropOverlay } from "./TabDropOverlay";
import { WorkspaceTabBar } from "./WorkspaceTabBar";
import { PainelVazio } from "./PainelVazio";
import { ehKindConhecido } from "../../services/consertarWorkspace";
import { EtiquetaDoProjeto, useProjetoDeFora, useProjetosDasAbas } from "./projetosDasAbas";
// Fork (0.9.4): a conversa que aparece no painel entra animada (Web Animations, sem View Transitions).
import { useAnimarAoTrocar } from "./animarConversa";

interface PaneViewProps {
  pane: PaneNode;
  active: boolean;
  onlyPane: boolean;
  maximized: boolean;
  projectId: string;
  projectPath: string;
  fileSaveMode: "auto" | "manual";
  /** Aba de agente: o chat (como na web) ou o terminal cru. Ver `AgentView` em `types/settings`. */
  agentView: AgentView;
  kanban: KanbanState;
  kanbanDispatch: React.Dispatch<KanbanAction>;
  dispatch: React.Dispatch<WorkspaceAction>;
}

export function PaneView({
  pane,
  active,
  onlyPane,
  maximized,
  projectId,
  projectPath,
  fileSaveMode,
  agentView,
  kanban,
  kanbanDispatch,
  dispatch,
}: PaneViewProps) {
  const [dragActive, setDragActive] = useState(false);
  const [tabDrag, setTabDrag] = useState<TabDragState>(null);
  useEffect(() => {
    const update = (event: Event) => setTabDrag((event as CustomEvent<TabDragState>).detail);
    const finish = () => setDragActive(false);
    window.addEventListener(TAB_DRAG_EVENT, update);
    window.addEventListener("dragend", finish);
    window.addEventListener("drop", finish);
    return () => {
      window.removeEventListener(TAB_DRAG_EVENT, update);
      window.removeEventListener("dragend", finish);
      window.removeEventListener("drop", finish);
    };
  }, []);
  const [dirtyTabIds, setDirtyTabIds] = useState<Set<string>>(new Set());
  const tab = pane.tabs.find((item) => item.id === pane.activeTabId) ?? pane.tabs[0];
  const tabId = tab?.id;
  // Fork (0.9.4): entrada animada da conversa ao abrir/trocar/fechar aba (ver animarConversa.ts).
  const conteudoRef = useRef<HTMLDivElement>(null);
  useAnimarAoTrocar(conteudoRef, tabId ?? "vazio");
  const deFora = useProjetoDeFora(tab?.kind === "agent" || tab?.kind === "terminal" ? tab.resourceId : null);
  // Fork: de que projeto é este painel. Aba com projeto próprio (criada pelo "+" aqui) ou conversa de
  // outro projeto: o "+" abre agente **nesse** projeto, não no dono do layout.
  const { projetos } = useProjetosDasAbas();
  const projetoDoPainel = tab?.projectId ?? deFora?.id ?? projectId;
  const projetoDaAba = tab?.projectId && projetos[tab.projectId] ? { id: tab.projectId, path: projetos[tab.projectId].path } : { id: projectId, path: projectPath };
  const criarAba = (kind: PaneNode["tabs"][number]["kind"], title: string) =>
    dispatch({ type: "CREATE_TAB", paneId: pane.id, kind, title, projectId: projetoDoPainel !== projectId ? projetoDoPainel : undefined });
  const dropZone = tabDrag?.target?.paneId === pane.id ? tabDrag.target.zone : null;
  // Soltar no próprio painel só faz algo quando divide e sobra aba para trás.
  const showDrop = dropZone !== null
    && !(tabDrag!.tab.paneId === pane.id && (dropZone === "center" || pane.tabs.length === 1));

  const handleDirtyChange = useCallback((changedTabId: string, dirty: boolean) => {
    setDirtyTabIds((previous) => {
      if (dirty === previous.has(changedTabId)) return previous;
      const next = new Set(previous);
      if (dirty) next.add(changedTabId);
      else next.delete(changedTabId);
      return next;
    });
  }, []);

  function split(direction: SplitNode["direction"]) {
    dispatch({ type: "SPLIT_PANE", paneId: pane.id, direction });
  }

  // Fork: fechar aba ou painel **não apaga conversa**. Aba de agente só sai da tela: a sessão segue
  // viva e continua na lateral, de onde reabre. Apagar é só pelo "Encerrar e remover" da lateral.
  // Terminal puro (shell) ainda é encerrado, mas só quando nenhuma outra aba (em qualquer painel ou
  // projeto) continua presa a ele (`projetosDasAbas.tsx`).
  const { abasDaSessao } = useProjetosDasAbas();
  const ehSessao = (item: PaneNode["tabs"][number]) => Boolean(item.resourceId && item.kind === "terminal");
  const nestePainel = (resourceId: string) => pane.tabs.filter((item) => item.resourceId === resourceId).length;

  function closeTab(item: PaneNode["tabs"][number]) {
    const close = ehSessao(item) && abasDaSessao(item.resourceId!) <= 1
      ? closeTerminal(item.resourceId!).catch(() => undefined) : Promise.resolve();
    void close.then(() => dispatch({ type: "CLOSE_TAB", paneId: pane.id, tabId: item.id }));
  }

  function closePane() {
    const sessions = [...new Set(pane.tabs.filter(ehSessao).map((item) => item.resourceId!))]
      .filter((id) => abasDaSessao(id) <= nestePainel(id))
      .map((id) => closeTerminal(id));
    void Promise.allSettled(sessions).then(() => dispatch({ type: "CLOSE_PANE", paneId: pane.id }));
  }

  useAcoesDaPaleta(`painel-${pane.id}`, active ? [
    { id: "novo-agente", titulo: "Novo agente", executar: () => criarAba("agent", "Novo agente") },
    { id: "novo-terminal", titulo: "Novo terminal", executar: () => criarAba("terminal", "Novo terminal") },
    { id: "dividir-horizontal", titulo: "Dividir painel lado a lado", executar: () => split("horizontal") },
    { id: "dividir-vertical", titulo: "Dividir painel em cima e embaixo", executar: () => split("vertical") },
    { id: "fechar-painel", titulo: "Fechar painel", executar: closePane },
    ...(tab ? [{ id: "fechar-aba", titulo: "Fechar aba atual", executar: () => closeTab(tab) }] : []),
    { id: "maximizar-painel", titulo: "Maximizar ou restaurar painel", executar: () => dispatch({ type: "TOGGLE_MAXIMIZE", paneId: pane.id }) },
  ] : []);

  const bindSession = useCallback(
    (sessionId: string, title: string, provider?: string) => {
      if (tabId) dispatch({ type: "BIND_TAB_RESOURCE", paneId: pane.id, tabId, resourceId: sessionId, title, provider });
    },
    [dispatch, pane.id, tabId]
  );

  return (
    <section
      className={[
        // Terminal pinta o próprio fundo (com a opacidade configurável); o do painel por baixo
        // cobriria o fundo do app mesmo com o terminal transparente.
        // Fork: `painel-moldura` (e `painel-cabecalho` abaixo) são ganchos do estilo Minimalista.
        "painel-moldura relative flex-1 min-w-0 min-h-0 flex flex-col border-2",
        tab?.kind === "terminal" || tab?.kind === "agent" ? "" : "bg-bg-surface/90",
        // Com um painel só, a moldura de foco não informa nada e só pesava a tela.
        active && !onlyPane ? "border-accent/80" : "border-border-subtle",
      ].join(" ")}
      aria-label={`Painel ${tab?.title ?? pane.id}`}
      data-drop-pane={pane.id}
      onPointerDown={() => dispatch({ type: "FOCUS_PANE", paneId: pane.id })}
      onDragEnter={(event) => {
        if (!Array.from(event.dataTransfer.types).includes(PANE_DRAG_TYPE)) return;
        event.preventDefault();
        setDragActive(true);
      }}
      onDragLeave={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setDragActive(false);
      }}
      onDragEnd={() => setDragActive(false)}
    >
      <header
        className="painel-cabecalho h-10 shrink-0 flex items-stretch border-b-2 border-border-subtle bg-bg-elevated/90"
        draggable
        onDragStart={(event) => {
          if ((event.target as HTMLElement).closest('[role="tablist"]')) {
            event.preventDefault();
            return;
          }
          event.dataTransfer.effectAllowed = "move";
          event.dataTransfer.setData(PANE_DRAG_TYPE, JSON.stringify({ paneId: pane.id }));
        }}
      >
        <WorkspaceTabBar pane={pane} dispatch={dispatch} onCloseTab={closeTab} dirtyTabIds={dirtyTabIds} onCriarAba={criarAba} />

        <div className="pane-acoes" aria-label="Ações do painel">
          <PaneAction label="Dividir lado a lado" glyph={<GlifoColunas />} onClick={() => split("horizontal")} />
          <PaneAction label="Dividir em cima e embaixo" glyph={<GlifoLinhas />} onClick={() => split("vertical")} />
          {!onlyPane && (
            <>
              <PaneAction
                label={maximized ? "Restaurar painel" : "Maximizar painel"}
                glyph={maximized ? <GlifoRecolher /> : <GlifoExpandir />}
                onClick={() => dispatch({ type: "TOGGLE_MAXIMIZE", paneId: pane.id })}
              />
              <PaneAction label="Fechar painel" glyph={<GlifoFechar />} perigo onClick={closePane} />
            </>
          )}
        </div>
      </header>

      {/* Fork: conversa de outro projeto solta neste layout — deixa claro de onde ela é. */}
      {deFora && (
        <div className="faixa-outro-projeto" style={{ "--cor-projeto": deFora.cor } as React.CSSProperties}>
          <span>Conversa de outro projeto</span>
          <EtiquetaDoProjeto projeto={deFora} />
        </div>
      )}
      {/* Fork: `painel-conteudo` evita a animação dupla (esta + a do `.chat-tela`), ver `vidro.css`. */}
      <div ref={conteudoRef} className="painel-conteudo flex-1 min-h-0">
        <PaneErrorBoundary key={tab?.id ?? pane.id}>
        {!tab ? (
          <PainelVazio onCriar={criarAba} />
        ) : tab.kind === "agent" ? (
          // Fork: a faixa abre revisão ao lado e mantém o destinatário da aba.
          <AbaDeAgente agentView={agentView} sessionId={tab.resourceId ?? null} emFoco={active} /* Fork (troca-de-contexto) */
            onRevisao={tab.resourceId ? () => dispatch({ type: "ABRIR_REVISAO", sessionId: tab.resourceId!, paneId: pane.id, projectId: projetoDoPainel !== projectId ? projetoDoPainel : undefined }) : undefined}>
            <TerminalPane
              projectId={projetoDaAba.id}
              projectPath={projetoDaAba.path}
              paneId={pane.id}
              tab={tab}
              onSessionCreated={bindSession}
            />
          </AbaDeAgente>
        ) : tab.kind === "terminal" ? (
          <TerminalPane
            projectId={projetoDaAba.id}
            projectPath={projetoDaAba.path}
            paneId={pane.id}
            tab={tab}
            onSessionCreated={bindSession}
          />
        ) : tab.kind === "conversa-historica" && tab.resourceId ? (
          <ConversaDoHistorico id={tab.resourceId} aoRetomar={(id) => dispatch({ type: "RENAME_TAB_RESOURCE", fromResourceId: tab.resourceId!, toResourceId: id, title: tab.title })} />
        ) : tab.kind === "web" ? (
          <NavegadorPane tab={tab} onTitulo={(titulo) => dispatch({ type: "RENAME_TAB_RESOURCE", fromResourceId: tab.resourceId ?? "", toResourceId: tab.resourceId ?? "", title: titulo.slice(0, 60) })} />
        ) : tab.kind === "orquestracao" && tab.resourceId ? (
          <FluxoAoVivo orquestrador={tab.resourceId} paneId={pane.id} />
        ) : tab?.kind === "file" && tab.resourceId && isImagePath(tab.resourceId) ? (
          <ImagePane tab={tab} />
        ) : tab?.kind === "file" ? (
          <FilePane projectPath={projectPath} tab={tab} saveMode={fileSaveMode} onDirtyChange={handleDirtyChange} />
        ) : tab?.kind === "markdown" ? (
          <MarkdownPane
            projectPath={projectPath}
            tab={tab}
            fileSaveMode={fileSaveMode}
            onDirtyChange={handleDirtyChange}
          />
        ) : tab?.kind === "git-diff" ? (
          <GitDiffPane projectPath={projectPath} tab={tab} />
        // Fork: revisão usa a pasta da sessão, inclusive worktrees.
        ) : tab?.kind === "revisao" ? (
          <RevisaoPane projectPath={projetoDaAba.path} sessionId={tab.resourceId} onAbrirSessao={(sessionId, title) => dispatch({ type: "ATTACH_TERMINAL", sessionId, title })} />
        ) : tab?.kind === "git-graph" ? (
          <GitGraphPane projectPath={projectPath} />
        ) : tab?.kind === "kanban" ? (
          <KanbanBoard projectId={projectId} kanban={kanban} dispatch={kanbanDispatch} />
        // Fork: aba salva por uma versão mais nova do app (kind que esta não conhece) fica
        // preservada no workspace e mostra só um aviso, sem quebrar (`services/consertarWorkspace.ts`).
        ) : !ehKindConhecido(tab.kind) ? (
          <div className="h-full p-4 text-xs overflow-auto">
            <p className="text-text-primary">Aba de uma versão mais nova do app.</p>
            <p className="mt-1 text-text-muted">Ela continua guardada no workspace e volta a abrir quando o app for atualizado.</p>
          </div>
        ) : (
          <div className="h-full p-4 font-mono text-xs overflow-auto">
            <div className="text-text-muted mb-4">{tab?.kind ?? "painel"}://{pane.id}</div>
            <p className="text-text-primary">Este painel será conectado numa fase posterior.</p>
          </div>
        )}
        </PaneErrorBoundary>
      </div>
      {dragActive && (
        <TabDropOverlay targetPaneId={pane.id} dispatch={dispatch} onFinished={() => setDragActive(false)} />
      )}
      {showDrop && (
        <div
          role="status"
          aria-label="Prévia do destino da aba"
          data-zone={dropZone}
          className={`pointer-events-none absolute z-30 flex items-center justify-center border-2 border-accent bg-accent/25 transition-all duration-150 motion-reduce:transition-none ${DROP_PREVIEW[dropZone].box}`}
        >
          <span className="bg-bg-elevated/90 px-2 py-1 text-[11px] text-text-primary">{/* Fork: conversa vinda da lateral (`projects/arrastarLinha.ts`) abre, não "move". */}
            {dropZone === "center" && tabDrag?.tab.paneId === "lateral" ? "Abrir neste painel" : DROP_PREVIEW[dropZone].label}</span>
        </div>
      )}
    </section>
  );
}

/** Área que a aba vai ocupar: metade do painel nas bordas, o painel inteiro no centro. */
const DROP_PREVIEW: Record<DropZone, { box: string; label: string }> = {
  center: { box: "inset-0", label: "Mover para este painel" },
  left: { box: "inset-y-0 left-0 right-1/2", label: "Dividir à esquerda" },
  right: { box: "inset-y-0 left-1/2 right-0", label: "Dividir à direita" },
  top: { box: "inset-x-0 top-0 bottom-1/2", label: "Dividir acima" },
  bottom: { box: "inset-x-0 top-1/2 bottom-0", label: "Dividir abaixo" },
};

function PaneAction({
  label,
  glyph,
  perigo,
  onClick,
}: {
  label: string;
  glyph: React.ReactNode;
  perigo?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={`pane-acao${perigo ? " pane-acao-perigo" : ""}`}
      aria-label={label}
      title={label}
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
    >
      {glyph}
    </button>
  );
}

/** Uma pane travada nunca pode derrubar o app inteiro — mesmo padrão do `EffectBoundary` do fundo
 * animado (`AppBackground.tsx`), aqui com um retry visível em vez de sumir silenciosamente.
 * `key={tab.id}` no call site garante que trocar de aba reseta o estado de erro. */
class PaneErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // eslint-disable-next-line no-console
    console.error("painel travou:", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div role="alert" className="h-full overflow-auto p-4 text-xs text-danger">
        <p className="mb-2">Este painel travou: {this.state.error.message}</p>
        <Button size="sm" variant="danger" onClick={() => this.setState({ error: null })}>
          Tentar de novo
        </Button>
      </div>
    );
  }
}
