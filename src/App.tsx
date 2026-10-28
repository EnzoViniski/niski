// Paleta e registro dos handlers já usados na interface.
import { AcoesDaPaleta, useAcoesDaPaleta } from "./features/paleta/acoes";
import { acoesParaAbrirConversa } from "./features/paleta/abrirConversa";
import { BotaoDoJarvis } from "./features/jarvis/BotaoDoJarvis";
import { ConversaDoHistorico } from "./features/paleta/ConversaDoHistorico";
import { Paleta } from "./features/paleta/Paleta";
import type { Conversation } from "./mobile/api";
import { TitleBar } from "./components/TitleBar";
import { LayoutPresetPicker } from "./features/workspace/LayoutPresetPicker";
import { useCallback, useEffect, useMemo, useState } from "react";
import { AppBackground } from "./components/backgrounds/AppBackground";
import { SettingsView } from "./components/SettingsView";
import { Sidebar } from "./components/Sidebar";
import { fileKind } from "./features/files/filesService";
import { QuickOpen } from "./features/files/QuickOpen";
import { useWorkspace } from "./features/workspace/useWorkspace";
// Recuperar workspace (backups, conversas recentes) e aviso de agentes fora do workspace.
import { RecuperacaoDoWorkspace } from "./features/workspace/RecuperacaoDoWorkspace";
import { collectResourceIds, findPane } from "./features/workspace/workspaceReducer";
import { RodapeDoProjeto, WorkspaceView } from "./features/workspace/WorkspaceView";
import { useKanban } from "./features/kanban/useKanban";
import { useKanbanDispatcher } from "./features/kanban/useKanbanDispatcher";
import { SessionProvidersContext } from "./features/terminal/sessionProviders";
import { ProjetosDasAbasContext, montarProjetosDasAbas } from "./features/workspace/projetosDasAbas";
import { useLinksEmAba } from "./features/navegador/useLinksEmAba";
import { useAberturasDoOrquestrador } from "./features/orquestracao/abrirFluxo";
import { useTerminalSessions } from "./features/terminal/useTerminalSessions";
// Manter ou apagar a worktree isolada ao encerrar o agente.
import { DialogoDaWorktree, worktreeParaPerguntar } from "./features/terminal/DialogoDaWorktree";
import { latestPerConversation, useAttention, type AttentionItem } from "./features/terminal/useAttention";
import { useAttentionNotifier } from "./features/terminal/useAttentionNotifier";
import { useJanelaEmFoco } from "./hooks/useJanelaEmFoco";
// Pausa animações e fundos WebGL com a janela oculta, sem foco ou parada (energia).
import { useEconomiaDeEnergia } from "./hooks/useEconomiaDeEnergia";
import { sessoesNaTela } from "./features/workspace/abrirEmPainelProprio"; // Lateral clareia o que está na tela.
import { todasAsTelasAbertas } from "./features/projects/telasAbertas"; // Navegação por todos os layouts abertos.
import { useAtenderPedidoDeAtualizacao } from "./lib/pedidoDeAtualizacao"; // "Atualizar o PC" pelo celular.
import {
  closeTerminal,
  duplicateTerminal,
  ensureEngine,
  restartTerminal,
  spawnTerminal,
} from "./features/terminal/terminalService";
import { HistoryView } from "./features/history/HistoryView";
// React Flow só ao abrir a tela de Fluxos.
import { FluxosView } from "./features/fluxos/FluxosViewTardio";
// Roadmaps de estudo, carregados só ao abrir a tela.
import { RoadmapsView } from "./features/roadmaps/RoadmapsViewTardio";
import type { SecaoDosRoadmaps } from "./features/caderno/tipos"; // Fork (Caderno)
// Tarefas (Meu dia, quadro, semana, lista) e aquecimento da manhã por cima de tudo.
import { PainelDeTarefas } from "./features/tarefas/PainelDeTarefas";
import { BotaoDasTarefas, ListaDoDia } from "./features/tarefas/Flutuantes";
import { abrirPainelDeTarefas, alternarListaDoDia, usePainel } from "./features/tarefas/painel";
// Fork (Copiloto): conversa do app numa janela flutuante ou presa à direita, com as ferramentas do app.
import { JanelaDoCopiloto } from "./features/copiloto/JanelaDoCopiloto";
import { abrirCopiloto, novaConversaDoCopiloto } from "./features/copiloto/copiloto";
import { RotinaDaManha } from "./features/manha/RotinaDaManha";
// Fork (Rotina): tela da rotina semanal e os avisos dos lembretes.
import { RotinaView } from "./features/rotina/RotinaView";
import { AvisosDaRotina } from "./features/rotina/AvisosDaRotina";
import { guardarPedidoPendente, ouvirPedidosDaRotina, pedirAbrirRotina } from "./features/rotina/abrirRotina";
import { abrirRotinaAgora, useRotinaDaManha } from "./features/manha/useRotinaDaManha";
// Fork (Bíblia do dia): a Palavra do dia depois do aquecimento, ou a qualquer hora.
import { LeituraDoDia } from "./features/biblia/LeituraDoDia";
import { abrirLeituraAgora, useLeituraDoDia } from "./features/biblia/useLeituraDoDia";
// Edição de vídeo (HyperFrames), carregada só ao abrir a tela.
import { VideoView } from "./features/video/VideoViewTardio";
import { definirEstudoEmTelaCheia, definirModoFoco, useEstudoEmTelaCheia, useModoFoco } from "./lib/modoFoco";
import type { HistoryEntry } from "./features/history/historyService";
// Fork (retomar-sempre): retomar do Histórico sem beco sem saída.
import { retomarDoHistorico } from "./features/retomada/retomarDoHistorico";
import type { OpcoesDeRetomada } from "./features/retomada/retomadaService";
import { rememberProject } from "./services/recentProjectsService";
import { primeSshDrives } from "./features/projects/targetState";
import type { SettingsSection } from "./hooks/useNotifications";
import { useSettings } from "./hooks/useSettings";
import { matchesShortcut } from "./lib/shortcuts";
// Caixa do engine usa a mesma navegação da paleta e registra o que está de fato na tela.
import { useAberturasDaAtividade } from "./features/atividade/navegacao";
import { usaFormaMinimalista } from "./lib/estiloDaInterface"; // Minimalista e Vidro sem fundo animado.
import { useAtividadeDoDesktop } from "./features/atividade/vistas";

type View = "workspace" | "settings" | "history" | "fluxos" | "conversa" | "roadmaps" | "video" | "rotina"; // Fork (Rotina).

export function App() { return <AcoesDaPaleta><AppComPaleta /></AcoesDaPaleta>; }

function AppComPaleta() {
  const [conversaSemProjeto, setConversaSemProjeto] = useState<string | null>(null);
  const [lateralOculta, setLateralOculta] = useState(false);
  useAcoesDaPaleta("app", [{ id: "alternar-lateral", titulo: "Alternar barra lateral", executar: () => setLateralOculta((atual) => !atual) }]);
  const [view, setView] = useState<View>("workspace");
  // Fork (Roadmaps): cada abertura remonta a tela; `continuar` vai direto ao último roadmap.
  const [aberturaDosRoadmaps, setAberturaDosRoadmaps] = useState({ chave: 0, continuar: true });
  // Fork (Caderno): qual parte dos Roadmaps está na tela (o Caderno mora lá dentro).
  const [secaoDosRoadmaps, setSecaoDosRoadmaps] = useState<SecaoDosRoadmaps>("roadmaps");
  const abrirRoadmaps = useCallback((continuar: boolean, secao: SecaoDosRoadmaps = "roadmaps") => {
    setAberturaDosRoadmaps((atual) => ({ chave: atual.chave + 1, continuar }));
    setSecaoDosRoadmaps(secao);
    setView("roadmaps");
  }, []);
  // Fork (Roadmaps): foco e estudo em tela cheia escondem a lateral e só valem dentro dos Roadmaps.
  const modoFoco = useModoFoco();
  const estudoEmTelaCheia = useEstudoEmTelaCheia();
  useEffect(() => { if (view !== "roadmaps") { definirModoFoco(false); definirEstudoEmTelaCheia(false); } }, [view]);
  useAcoesDaPaleta("roadmaps", [
    { id: "roadmaps-continuar", titulo: "Continuar estudo (Roadmaps)", executar: () => abrirRoadmaps(true) },
    { id: "roadmaps-lista", titulo: "Roadmaps de estudo", executar: () => abrirRoadmaps(false) },
    { id: "caderno-abrir", titulo: "Caderno: anotações de livros, estudos e vida", executar: () => abrirRoadmaps(true, "caderno") }, // Fork (Caderno)
  ]);
  // Fork (Edição de vídeo): cada abertura remonta a tela; `continuar` volta ao último vídeo aberto.
  const [aberturaDoVideo, setAberturaDoVideo] = useState({ chave: 0, continuar: true });
  const abrirVideo = useCallback((continuar: boolean) => { setAberturaDoVideo((a) => ({ chave: a.chave + 1, continuar })); setView("video"); }, []);
  useAcoesDaPaleta("video", [
    { id: "video-continuar", titulo: "Continuar edição de vídeo", executar: () => abrirVideo(true) },
    { id: "video-lista", titulo: "Edição de vídeo", executar: () => abrirVideo(false) },
  ]);
  // Fork (Rotina): a tela abre por pedido (lateral, paleta, aviso); o pedido fica guardado até ela montar.
  useEffect(() => ouvirPedidosDaRotina((pedido) => { guardarPedidoPendente(pedido); setView("rotina"); }), []);
  useAcoesDaPaleta("rotina", [
    { id: "rotina-hoje", titulo: "Rotina: hoje", executar: () => pedirAbrirRotina({ aba: "hoje" }) },
    { id: "rotina-semana", titulo: "Rotina: semana", executar: () => pedirAbrirRotina({ aba: "semana" }) },
    { id: "rotina-planejar", titulo: "Rotina: planejar a semana", executar: () => pedirAbrirRotina({ aba: "semana", conversa: "planejar" }) },
    { id: "rotina-conversar", titulo: "Rotina: conversar com a rotina", executar: () => pedirAbrirRotina({ conversa: "conversar" }) },
    { id: "rotina-treino", titulo: "Rotina: registrar treino", executar: () => pedirAbrirRotina({ aba: "treino" }) },
    { id: "rotina-alimentacao", titulo: "Rotina: alimentação e água", executar: () => pedirAbrirRotina({ aba: "alimentacao" }) },
  ]);
  // Fork (Tarefas e Manhã): painel de tarefas por cima do app e o aquecimento que cobre a tela no horário escolhido.
  const manha = useRotinaDaManha();
  const leitura = useLeituraDoDia(manha.aberta); // Fork (Bíblia do dia)
  const painelDeTarefas = usePainel();
  useAcoesDaPaleta("tarefas", [
    { id: "tarefas-abrir", titulo: "Tarefas: abrir o painel", executar: abrirPainelDeTarefas },
    { id: "tarefas-lista", titulo: "Tarefas: lista do dia flutuante", executar: alternarListaDoDia },
    { id: "copiloto-abrir", titulo: "Copiloto: abrir", executar: abrirCopiloto },
    { id: "copiloto-nova", titulo: "Copiloto: nova conversa", executar: () => { void novaConversaDoCopiloto(); abrirCopiloto(); } },
    { id: "manha-agora", titulo: "Aquecimento da manhã: fazer agora", executar: abrirRotinaAgora },
    { id: "biblia-ler", titulo: "Palavra do dia: ler a Bíblia agora", executar: abrirLeituraAgora }, // Fork (Bíblia do dia)
  ]);
  // Pasta do agente em worktree isolada que acabou de ser encerrado (`DialogoDaWorktree`).
  const [worktreeAoFechar, setWorktreeAoFechar] = useState<string | null>(null);
  const fecharDialogoDaWorktree = useCallback(() => setWorktreeAoFechar(null), []);
  const [settingsSection, setSettingsSection] = useState<SettingsSection | undefined>();
  // Para onde o "fechar" das Configurações volta: a tela de antes, com o workspace intacto no reducer.
  const [viewBeforeSettings, setViewBeforeSettings] = useState<Exclude<View, "settings">>("workspace");
  // Mesma ideia para o Histórico. Aberto por cima das Configurações, volta para a tela de antes
  // delas — senão fechar o histórico reabriria as Configurações, que o usuário já tinha deixado.
  const [viewBeforeHistory, setViewBeforeHistory] = useState<Exclude<View, "history">>("workspace");
  const { settings, updateSettings, resetSettings } = useSettings();
  // Memo: o objeto novo a cada render faria o `useWorkspace` recalcular a projeção à toa.
  const temaPublicado = useMemo(
    // O estilo (Clássico/Minimalista) vai junto, e o celular segue o PC.
    () => ({ accent: settings.accent, theme: settings.theme, background: settings.background, glass: settings.glass, estilo: settings.estilo }),
    [settings.accent, settings.theme, settings.background, settings.glass, settings.estilo]
  );
  const { workspace, activeProject, dispatch, workspaces, activeWorkspaceId } = useWorkspace(temaPublicado);
  const { sessions, online: engineOnline } = useTerminalSessions();
  const { kanban, dispatch: kanbanDispatch } = useKanban();
  useKanbanDispatcher(kanban, kanbanDispatch, workspace.projects, sessions);
  // Com as Configurações abertas nenhuma pane está na tela, e com a janela fora de foco ninguém
  // está olhando para ela — nos dois casos nada pode ser marcado como visto.
  const janelaEmFoco = useJanelaEmFoco();
  useEconomiaDeEnergia();
  useAtenderPedidoDeAtualizacao();
  const { items: attention, countByWorkspace, dismiss: dismissAttention } = useAttention(
    sessions,
    workspaces,
    activeWorkspaceId,
    view === "workspace" && janelaEmFoco
  );

  useAttentionNotifier(settings.notifyAttention, settings.avisos);
  useAtividadeDoDesktop(activeProject, sessions, (view === "workspace" || view === "conversa") && janelaEmFoco, view === "conversa" ? conversaSemProjeto : null);
  useAberturasDaAtividade(abrirConversaDaPaleta);

  // Sessões por id: a barra de abas troca o ícone de conversa pela marca do agente, e o rodapé
  // descobre provider, conta e transcript da aba em foco.
  const sessionsById = useMemo(
    () => Object.fromEntries(sessions.map((session) => [session.id, session])),
    [sessions]
  );

  // Nomes dos projetos e quantas abas cada sessão tem, para as abas (`projetosDasAbas.tsx`).
  const projetosDasAbas = useMemo(() => montarProjetosDasAbas(workspace), [workspace]);
  // Link clicado abre numa aba de navegador ao lado, não na janela do app.
  useLinksEmAba(workspaces, dispatch, () => setView("workspace"));
  // "Ver fluxo" e o clique num nó do fluxo do orquestrador.
  useAberturasDoOrquestrador(workspace, dispatch, () => setView("workspace"));

  function closeSettings() {
    setView(viewBeforeSettings);
  }

  function openHistory() {
    if (view === "history") return;
    const origem = view === "settings" ? viewBeforeSettings : view;
    // Histórico → Configurações → Histórico: mantém a volta que o primeiro Histórico já guardou.
    if (origem !== "history") setViewBeforeHistory(origem);
    setView("history");
  }

  function closeHistory() {
    setView(viewBeforeHistory);
  }

  // O engine devolve em `session.name` o nome da **conversa** (o primeiro prompt, ou o que o usuário
  // renomeou). Espelha também o provider; uma troca de agente não pode deixar a marca antiga
  // salva nas abas. O reducer percorre todos os workspaces e ignora polls sem mudança.
  useEffect(() => {
    dispatch({ type: "SYNC_SESSION_IDENTITIES", sessions });
  }, [sessions, activeProject?.id, dispatch]);

  // Qual unidade é qual máquina remota: lido uma vez, para os polls saberem que um projeto em
  // `X:\...` é SSH e não disco local (ver `targetState`).
  useEffect(() => {
    void primeSshDrives();
  }, []);

  useEffect(() => {
    if (view !== "settings" && view !== "history") return;
    function onKeyDown(event: KeyboardEvent) {
      // Esc dentro de campo/menu é do campo; só fecha a tela se ninguém mais tratou a tecla.
      if (event.defaultPrevented || (event.target as HTMLElement | null)?.closest?.("input, textarea, [role=listbox]")) return;
      if (!matchesShortcut(event, "closeSettings")) return;
      event.preventDefault();
      setView(view === "settings" ? viewBeforeSettings : viewBeforeHistory);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [view, viewBeforeSettings, viewBeforeHistory]);

  function focusSession(item: AttentionItem) {
    dispatch({
      type: "FOCUS_SESSION",
      workspaceId: item.workspaceId,
      projectId: item.projectId,
      sessionId: item.sessionId,
      title: item.sessionName,
    });
    setView("workspace");
  }

  async function runInTerminal(title: string, command: string) {
    if (!activeProject) return;
    await ensureEngine();
    const session = await spawnTerminal({
      projectId: activeProject.id,
      name: title,
      cwd: activeProject.path,
      rows: 30,
      cols: 120,
      initialCommand: command,
    });
    dispatch({ type: "ATTACH_TERMINAL", sessionId: session.id, title: session.name });
    setView("workspace");
  }

  function openFile(path: string, kind: "file" | "markdown") {
    if (!activeProject) return;
    const title = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
    dispatch({ type: "CREATE_TAB", paneId: activeProject.activePaneId, kind, title, resourceId: path });
    setView("workspace");
  }

  // Mesma ação da lateral, incluindo seleção do workspace dono da conversa.
  function abrirConversaDaPaleta(conversa: Conversation) {
    const acoes = acoesParaAbrirConversa(conversa, workspaces, sessions, activeProject);
    if (acoes.length) { acoes.forEach(dispatch); setView("workspace"); }
    else { setConversaSemProjeto(conversa.id); setView("conversa"); }
  }

  /** Reabre uma conversa do Histórico num terminal do projeto dela — adicionando o projeto ao
   *  workspace ativo se ele ainda não estiver aberto em nenhum. Fork (retomar-sempre): a lógica
   *  (sessão já viva, pasta apagada, conta, conversa nova com histórico) mora em `retomarDoHistorico`. */
  async function resumeHistory(entry: HistoryEntry, profileId: string = entry.profile_id, opcoes: OpcoesDeRetomada = {}) {
    await retomarDoHistorico(entry, profileId, opcoes, {
      workspaces,
      activeWorkspaceId,
      projetoAtual: activeProject ? { id: activeProject.id, path: activeProject.path } : null,
      dispatch,
      lembrarProjeto: rememberProject,
      aoAbrir: () => setView("workspace"),
    });
  }

  return (
    <SessionProvidersContext.Provider value={sessionsById}>
    <ProjetosDasAbasContext.Provider value={projetosDasAbas}>
    <div className="flex h-screen flex-col overflow-hidden">
      {/* Minimalista e Vidro não têm fundo animado (não gastam GPU desenhando por baixo). */}
      {!usaFormaMinimalista(settings.estilo) && <AppBackground settings={settings} />}
      <TitleBar
        trilha={
          view === "settings" ? ["Configurações"]
            : view === "history" ? ["Histórico"]
            : view === "fluxos" ? ["Fluxos de agentes"]
            : view === "roadmaps" ? ["Roadmaps"]
            : view === "video" ? ["Edição de vídeo"]
            : view === "rotina" ? ["Rotina"]
            : [workspaces.find((item) => item.id === activeWorkspaceId)?.name, activeProject?.name].filter((parte): parte is string => Boolean(parte))
        }
        dica={view === "workspace" ? activeProject?.path : undefined}
        acoes={<>{view === "workspace" && activeProject ? <LayoutPresetPicker dispatch={dispatch} /> : null}<BotaoDoJarvis /></>} /* Jarvis ao lado de Layouts. */
        onHome={() => setView("workspace")}
        onOpenSettings={(section) => {
          setSettingsSection(section);
          if (view !== "settings") setViewBeforeSettings(view);
          setView("settings");
        }}
      />
      <div className="flex min-h-0 flex-1">
      <Paleta workspaces={workspaces} projectPath={activeProject?.path ?? null}
        onProjeto={(workspaceId, projectId) => { dispatch({ type: "SELECT_WORKSPACE", workspaceId }); dispatch({ type: "SELECT_PROJECT", projectId }); setView("workspace"); }}
        onConversa={abrirConversaDaPaleta} onArquivo={(path) => openFile(path, fileKind(path))} />
      <QuickOpen projectPath={activeProject?.path ?? null} onOpenFile={(path) => openFile(path, fileKind(path))} />
      <div className={lateralOculta || (view === "roadmaps" && (modoFoco || estudoEmTelaCheia)) ? "hidden" : "contents"}>
      <Sidebar
        workspaces={workspaces}
        activeWorkspaceId={activeWorkspaceId}
        onSelectWorkspace={(workspaceId) => dispatch({ type: "SELECT_WORKSPACE", workspaceId })}
        onCreateWorkspace={() => dispatch({ type: "ADD_WORKSPACE" })}
        onCloseWorkspace={(workspaceId) => {
          const closingWorkspace = workspaces.find((item) => item.id === workspaceId);
          const closing = (closingWorkspace?.projects ?? [])
            .flatMap((project) => collectResourceIds(project.layout))
            .map((id) => closeTerminal(id));
          void Promise.allSettled(closing).then(() => dispatch({ type: "CLOSE_WORKSPACE", workspaceId }));
        }}
        onRenameWorkspace={(workspaceId, name) => dispatch({ type: "RENAME_WORKSPACE", workspaceId, name })}
        attention={attention}
        attentionByWorkspace={countByWorkspace}
        onFocusSession={focusSession}
        onDismissAttention={(item) => dismissAttention(item.sessionId)}
        onRunInTerminal={(title, command) => void runInTerminal(title, command).catch(() => undefined)}
        projects={workspace.projects}
        activeProjectId={workspace.activeProjectId}
        activeProjectPath={activeProject?.path ?? null}
        onOpenFile={openFile}
        onFileRenamed={(fromPath, toPath, title) => {
          dispatch({ type: "RENAME_TAB_RESOURCE", fromResourceId: fromPath, toResourceId: toPath, title });
        }}
        onPathDeleted={(path) => {
          dispatch({ type: "CLOSE_TABS_BY_RESOURCE_PREFIX", prefix: path });
        }}
        // Uma linha por conversa: as tentativas antigas do failover (uma sessão parada por troca
        // de conta) repetiam a mesma conversa dez vezes na lista.
        activeSessionId={(() => {
          const pane = activeProject ? findPane(activeProject.layout, activeProject.activePaneId) : null;
          return pane?.tabs.find((tab) => tab.id === pane.activeTabId)?.resourceId ?? null;
        })()}
        terminalSessions={latestPerConversation(
          sessions.filter((session) => session.project_id === workspace.activeProjectId),
        )}
        agentSessions={latestPerConversation(sessions.filter((session) => session.provider))}
        sessoesNaTela={sessoesNaTela(activeProject?.layout)}
        paineisNaTela={todasAsTelasAbertas(workspaces, activeWorkspaceId)}
        sessoesDasTelas={sessions}
        onFocarAba={(paneId, tabId, contexto) => {
          if (contexto) dispatch({ type: "FOCUS_OPEN_TAB", destino: { ...contexto, paneId, tabId } });
          else {
            dispatch({ type: "FOCUS_PANE", paneId });
            dispatch({ type: "SELECT_TAB", paneId, tabId });
          }
          setView("workspace");
        }}
        showSettings={view === "settings"}
        showHistory={view === "history"}
        showFluxos={view === "fluxos"}
        showTarefas={painelDeTarefas.aberto}
        onOpenTarefas={abrirPainelDeTarefas}
        showRotina={view === "rotina"} /* Fork (Rotina). */
        onOpenRotina={() => setView("rotina")}
        showRoadmaps={view === "roadmaps"}
        onOpenRoadmaps={() => abrirRoadmaps(true)}
        showCaderno={view === "roadmaps" && secaoDosRoadmaps === "caderno"} /* Fork (Caderno) */
        onOpenCaderno={() => abrirRoadmaps(true, "caderno")}
        showVideo={view === "video"}
        onOpenVideo={() => abrirVideo(true)}
        onOpenFluxos={() => setView("fluxos")}
        onOpenHistory={openHistory}
        onHome={() => setView("workspace")}
        onAddProject={(path) => {
          rememberProject(path);
          dispatch({ type: "ADD_PROJECT", path });
          setView("workspace");
        }}
        onSelectProject={(projectId) => {
          dispatch({ type: "SELECT_PROJECT", projectId });
          setView("workspace");
        }}
        onCloseProject={(projectId) => {
          const project = workspace.projects.find((item) => item.id === projectId);
          const closing = project ? collectResourceIds(project.layout).map((id) => closeTerminal(id)) : [];
          void Promise.allSettled(closing).then(() => dispatch({ type: "CLOSE_PROJECT", projectId }));
        }}
        onNovaConversaNoProjeto={(projectId) => {
          // "+" da pasta — conversa nova do projeto num painel próprio, sem trocar o layout da direita
          // e aguardando a escolha de agente/conta/modelo (`abrirEmPainelProprio.ts`).
          dispatch({ type: "NEW_CONVERSATION_OWN_PANE", projectId });
          setView("workspace");
        }}
        onAbrirConversa={(projectId, sessionId, title) => {
          if (!activeWorkspaceId) return;
          // Conversa que não está na tela abre num painel próprio ao lado (`abrirEmPainelProprio.ts`).
          dispatch({ type: "OPEN_SESSION_OWN_PANE", workspaceId: activeWorkspaceId, projectId, sessionId, title });
          setView("workspace");
        }}
        // Fork (histórico na lateral): mesmo caminho do "Retomar" da tela Histórico.
        onRetomarConversaAnterior={(entry) => resumeHistory(entry)}
        onSoltarConversaNoLayout={(sessionId, title, targetPaneId, zone) => {
          // Arrastar da lateral para o layout (`features/workspace/colocarConversa.ts`).
          dispatch({ type: "PLACE_SESSION", sessionId, title, targetPaneId, zone });
          setView("workspace");
        }}
        onAttachTerminal={(sessionId, title) => {
          dispatch({ type: "ATTACH_TERMINAL", sessionId, title });
          setView("workspace");
        }}
        onCloseTerminal={(sessionId) => {
          // Agente em worktree isolada — pergunta se mantém ou apaga a worktree.
          setWorktreeAoFechar(worktreeParaPerguntar(sessions, sessionId));
          void closeTerminal(sessionId).catch(() => undefined);
          // Fecha (ou reseta, se for a última tab) qualquer tab presa nessa sessão — senão ela
          // fica apontando pra um resourceId morto e o poll do TerminalPane vira "session not
          // found" pra sempre.
          dispatch({ type: "CLOSE_TABS_BY_RESOURCE_PREFIX", prefix: sessionId });
        }}
        onDuplicateTerminal={(sessionId) => {
          void duplicateTerminal(sessionId)
            .then((session) => dispatch({ type: "ATTACH_TERMINAL", sessionId: session.id, title: session.name }))
            .catch(() => undefined);
        }}
        onRestartTerminal={(sessionId) => { void restartTerminal(sessionId).catch(() => undefined); }}
        onOpenGitDiff={(file, staged) => {
          if (!activeProject) return;
          const title = file.split(/[\\/]/).filter(Boolean).pop() ?? file;
          dispatch({
            type: "CREATE_TAB",
            paneId: activeProject.activePaneId,
            kind: "git-diff",
            title: `${title} (diff)`,
            resourceId: `${staged ? "staged" : "unstaged"}::${file}`,
          });
          setView("workspace");
        }}
        onOpenGitGraph={() => {
          if (!activeProject) return;
          dispatch({ type: "CREATE_TAB", paneId: activeProject.activePaneId, kind: "git-graph", title: "Git Graph" });
          setView("workspace");
        }}
        kanban={kanban}
        onOpenKanban={() => {
          if (!activeProject) return;
          dispatch({ type: "CREATE_TAB", paneId: activeProject.activePaneId, kind: "kanban", title: "Kanban" });
          setView("workspace");
        }}
        onOpenSettings={(section) => {
          setSettingsSection(section);
          if (view !== "settings") setViewBeforeSettings(view);
          setView("settings");
        }}
      />

      </div>

      <div className="flex-1 flex flex-col min-w-0 min-h-0">
        <main className="flex-1 min-h-0">
          {view === "history" ? (
            <HistoryView onResume={resumeHistory} onClose={closeHistory} />
          ) : view === "rotina" ? ( /* Fork (Rotina). */
            <RotinaView agentView={settings.agentView} onClose={() => setView("workspace")} />
          ) : view === "roadmaps" ? (
            <RoadmapsView key={aberturaDosRoadmaps.chave} continuar={aberturaDosRoadmaps.continuar} onClose={() => setView("workspace")} secao={secaoDosRoadmaps} onSecao={setSecaoDosRoadmaps} />
          ) : view === "video" ? (
            <VideoView key={aberturaDoVideo.chave} continuar={aberturaDoVideo.continuar} agentView={settings.agentView} onClose={() => setView("workspace")} />
          ) : view === "fluxos" ? (
            <div className="h-full min-h-0 flex flex-col">
              <div className="flex-1 min-h-0">
                <FluxosView projectId={activeProject?.id ?? null} projectPath={activeProject?.path ?? null}
                  onAbrirSessao={(sessionId, title) => { dispatch({ type: "ATTACH_TERMINAL", sessionId, title }); setView("workspace"); }}
                  onClose={() => setView("workspace")} />
              </div>
              <RodapeDoProjeto projectPath={activeProject?.path ?? null} engineOnline={engineOnline} activeSessionId={null} />
            </div>
          ) : view === "settings" ? (
            <div className="h-full overflow-y-auto px-6 py-5 [scrollbar-gutter:stable]">
              <div className="w-full mx-auto max-w-[1180px]">
                <SettingsView
                  settings={settings}
                  onChange={updateSettings}
                  onReset={resetSettings}
                  initialSection={settingsSection}
                  projectPath={activeProject?.path ?? null}
                  projectPaths={[...new Set(workspaces.flatMap((item) => item.projects.map((project) => project.path)))]}
                  onClose={closeSettings}
                />
              </div>
            </div>
          ) : view === "conversa" && conversaSemProjeto ? (
            <div className="flex h-full flex-col">
              <button className="px-3 py-2 text-left text-xs text-accent" onClick={() => setView("workspace")}>Voltar ao projeto</button>
              <div className="min-h-0 flex-1"><ConversaDoHistorico id={conversaSemProjeto} aoRetomar={setConversaSemProjeto} /></div>
            </div>
          ) : (
            <WorkspaceView
              project={activeProject}
              engineOnline={engineOnline}
              attention={attention}
              fileSaveMode={settings.fileSaveMode}
              agentView={settings.agentView}
              kanban={kanban}
              kanbanDispatch={kanbanDispatch}
              dispatch={dispatch}
            />
          )}
        </main>
      </div>
      <JanelaDoCopiloto /> {/* Fork (Copiloto): coluna à direita ou janela flutuante. */}
      <AvisosDaRotina /> {/* Fork (Rotina): lembretes vencidos, discretos no canto. */}
      </div>
      <BotaoDasTarefas />
      <ListaDoDia />
      <PainelDeTarefas onAbrirAjustes={() => { setSettingsSection("tarefas"); if (view !== "settings") setViewBeforeSettings(view); setView("settings"); }} />
      {manha.aberta && <RotinaDaManha onFechar={manha.fechar} onAbrirTarefas={abrirPainelDeTarefas} />}
      {leitura.aberta && !manha.aberta && <LeituraDoDia onFechar={leitura.fechar} onAjustes={() => { setSettingsSection("manha"); if (view !== "settings") setViewBeforeSettings(view); setView("settings"); }} />}
      {worktreeAoFechar && <DialogoDaWorktree caminho={worktreeAoFechar} onFechar={fecharDialogoDaWorktree} />}
      <RecuperacaoDoWorkspace workspaces={workspaces} sessions={sessions} dispatch={dispatch} mostrarAviso={view === "workspace"} />
    </div>
    </ProjetosDasAbasContext.Provider>
    </SessionProvidersContext.Provider>
  );
}
