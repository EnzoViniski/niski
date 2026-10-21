import "@xterm/xterm/css/xterm.css";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect, useRef, useState } from "react";
import { Button } from "../../components/ui";
import type { WorkspaceTab } from "../../types/workspace";
import { joinPath, writeBinaryFile } from "../files/filesService";
import {
  attachTerminal,
  accountUsage,
  beginConversation,
  ensureAgentTrust,
  ensureEngine,
  exhaustedUntilMs,
  listAgentClis,
  listProfiles,
  MULTI_ACCOUNT_PROVIDERS,
  planSwitch,
  resizeTerminal,
  restartTerminal,
  selectFailoverProfile,
  spawnTerminal,
  engineApi,
  stopTerminal,
  terminalSnapshot,
  terminalSnapshotEspera,
  type AgentCliStatus,
  type AgentCliId,
  type AgentLaunch,
  type LaunchPlan,
  type Profile,
  type TerminalState,
  usageLimitResetAtMs,
  writeTerminal,
} from "./terminalService";
import type { TerminalSession } from "./terminalService";
import { handleTerminalKey } from "./keyBindings";
import { AgentLauncher } from "./AgentLauncher";
import { AgentSwitcher } from "./AgentSwitcher";
import { AgentRuntimeBadge } from "./AgentRuntimeBadge";
import { FONTE_MONO } from "../../lib/fontes";
import { ajustesDoAgente, opcoesDeModelo } from "../../lib/escolhasSalvas";
import { useSession } from "./sessionProviders";
// Fork (retomar-sempre): saídas quando a aba não consegue voltar (pasta sumida, conversa não achada).
import { ConversaNaoAchada, PastaSumida } from "../retomada/AvisoDeRetomada";
import { cliNaoAchouAConversa, ehPastaSumida } from "../retomada/retomadaService";
import { NOME_CURTO } from "../../lib/identidade";

/** Colar imagem numa CLI de agente: ESC v (meta+v) — o que o Alt+V já mandava e funcionava.
 *  Se alguma CLI passar a escutar ^V (0x16) em vez disso, é aqui que muda. */
const CLI_IMAGE_PASTE = "\x1bv";

/** Falhas seguidas do poll antes de mostrar o banner de erro. Com poll de 100ms, três falhas são
 *  ~0,3s de engine mudo — abaixo disso é soluço, não queda, e o banner só atrapalha. */
const FALHAS_ATE_AVISAR = 3;

/**
 * Ritmo de leitura da PTY.
 *
 * **Não existe eco local**: o que aparece na tela é o que a CLI redesenhou e o engine devolveu, então
 * a letra digitada só aparece na leitura seguinte. Com um ritmo fixo de 100ms, cada tecla esperava
 * de 0 a 100ms para aparecer — é isso que se sente como teclado lerdo.
 *
 * Enquanto há atividade (tecla digitada ou saída nova) a leitura vai a um quadro; parada a conversa,
 * volta ao ritmo econômico. O engine responde um snapshot em ~0,3ms, então o custo é a viagem de IPC
 * e só acontece enquanto alguém está usando aquela pane.
 */
const POLL_ATIVO_MS = 16;
const POLL_OCIOSO_MS = 100;
/** Quanto tempo depois da última atividade o ritmo rápido continua valendo. */
const JANELA_ATIVA_MS = 700;
/** Fork: janela oculta (minimizada, outro Space, coberta). Ninguém vê o quadro: lê 1x/s só para o
 *  estado (aviso de turno, failover) seguir vivo; ao voltar, lê na hora. */
const POLL_OCULTO_MS = 1_000;
/** Fork (energia): PTY calada há {@link VAZIAS_ATE_ESPACAR} leituras seguidas (~1 s) com a janela
 *  na tela: espaça para 250 ms (em foco) ou 500 ms (sem foco). Tecla digitada volta ao ritmo rápido na
 *  hora, e a primeira saída nova volta aos 100 ms. */
const VAZIAS_ATE_ESPACAR = 10;
const POLL_CALADO_MS = 250;
const POLL_CALADO_SEM_FOCO_MS = 500;
/** Fork (energia): com engine novo a leitura é long-poll. O engine segura o pedido até a PTY
 *  escrever ou o estado mudar; o prazo só garante uma resposta de tempos em tempos. Os ritmos acima
 *  ficam para um engine antigo ainda aberto (a troca do engine é adiada com conversa ocupada). */
const ESPERA_MS = 20_000;
/** Esperando a conta nova ficar pronta para o "continue": precisa olhar a cada {@link CONTINUAR_RITMO_MS}. */
const ESPERA_CURTA_MS = 1_500;
/** Entre duas leituras com saída: no máximo uma por quadro. */
const QUADRO_MS = 16;
/** Intervalo entre tentativas quando o failover não achou conta ou falhou. O aviso de limite
 *  continua na tela, então a próxima leitura reabre a busca — e uma conta conectada depois do
 *  limite passa a ser encontrada sem o usuário precisar reabrir a aba. */
const FAILOVER_RETRY_MS = 30_000;
/** Limite que chega até isso depois do agente ter sido visto trabalhando = caiu no meio de uma tarefa. */
const MEIO_DA_TAREFA_MS = 120_000;
/** Quanto esperar a conta nova ficar pronta para receber o "continue" antes de desistir. */
const CONTINUAR_ATE_MS = 120_000;
/** Entre uma consulta e outra da tela da conta nova, enquanto espera para mandar o "continue". */
const CONTINUAR_RITMO_MS = 1_500;
/** Mensagem que o OMNI manda sozinho depois de trocar de conta no meio de uma tarefa. Diz que é
 *  automática para ninguém (você nem o agente) achar que foi digitada. */
export const MENSAGEM_DE_CONTINUAR =
  `Continue de onde parou. (Mensagem automática do ${NOME_CURTO}: a conta anterior atingiu o limite de uso e esta conversa foi passada para outra conta.)`;

type FailoverStatus = {
  tone: "working" | "success" | "warning";
  text: string;
  detail: string;
};

interface TerminalPaneProps {
  projectId: string;
  projectPath: string;
  paneId: string;
  tab: WorkspaceTab;
  onSessionCreated: (sessionId: string, title: string, provider?: string) => void;
}

export function TerminalPane({ projectId, projectPath, paneId, tab, onSessionCreated }: TerminalPaneProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef(tab.resourceId);
  // Fork: pasta da sessão aberta (pode ser a worktree de outra branch) — a troca de conta relança nela.
  const pastaDaSessaoRef = useRef<string | undefined>(undefined);
  pastaDaSessaoRef.current = useSession(tab.resourceId)?.cwd ?? pastaDaSessaoRef.current;
  /** Criação de sessão em andamento. O efeito pode rodar de novo antes dela terminar (StrictMode no
   *  dev, remontagem): sem isto cada execução abria a sua PTY e "Novo terminal" virava dois. */
  const criandoRef = useRef<Promise<TerminalSession> | null>(null);
  const sequenceRef = useRef(0);
  const falhasRef = useRef(0);
  const [error, setError] = useState<string | null>(null);
  const [state, setState] = useState<TerminalState>("stopped");
  const [retry, setRetry] = useState(0);
  const [launch, setLaunch] = useState<AgentLaunch | null>(null);
  const [switching, setSwitching] = useState(false);
  const [inputLocked, setInputLocked] = useState(false);
  const [failoverStatus, setFailoverStatus] = useState<FailoverStatus | null>(null);
  const [conversaNaoAchada, setConversaNaoAchada] = useState(false);
  const conversationRef = useRef<string | null>(null);
  /** Conta → instante exato em que volta ao rodízio. O mapa sobrevive à remontagem do efeito
   * causada pela troca de perfil; o cache de uso em disco recompõe a informação após restart. */
  const exhaustedProfilesRef = useRef(new Map<string, number>());
  const availabilityTimersRef = useRef(new Map<string, number>());
  const failoverConversationRef = useRef<string | null>(null);
  /** Conta que acabou de receber a conversa por failover. Ao retomar, o CLI reexibe o aviso de
   *  limite da conta anterior, e o engine (que lê a tela) o reporta como se fosse desta — sem este
   *  filtro a aba tentava trocar de novo na hora. Vale até o aviso sair da tela uma vez. */
  const staleLimitProfileRef = useRef<string | null>(null);
  /** Troca feita no meio de uma tarefa: falta mandar o "continue" quando a conta nova ficar pronta.
   *  Ref porque a troca remonta o efeito (nova conta) e o pedido tem de sobreviver a isso. */
  const continuarRef = useRef<{ conversationId: string; ate: number; consultaEm: number } | null>(null);
  /** Última vez que o agente desta aba foi visto trabalhando. */
  const trabalhouEmRef = useRef(0);
  /** Título da aba num ref, **fora das dependências do efeito**: ele só é lido na hora de abrir a
   *  sessão, e tê-lo como dependência fazia renomear a aba derrubar o xterm e reproduzir o
   *  scrollback inteiro. Com o nome automático vindo do primeiro prompt, isso passou a acontecer em
   *  toda conversa. */
  const tituloRef = useRef(tab.title);
  tituloRef.current = tab.title;
  /** Quem está atendendo esta aba, do ponto de vista da SESSÃO (e não do launcher).
   *
   *  Tem de vir do snapshot: `launch` só existe quando foi esta montagem da pane que abriu o
   *  agente. Numa sessão restaurada (app reaberto, aba reatachada) ele é `null` para sempre — foi
   *  o que deixou a etiqueta de modelo invisível justamente no caso mais comum. */
  const [sessao, setSessao] = useState<{
    provider?: string;
    profileId?: string;
    externalSessionId?: string;
  }>({});
  const agent = launch?.agent ?? null;

  function clearAvailabilityTimers() {
    for (const timer of availabilityTimersRef.current.values()) window.clearTimeout(timer);
    availabilityTimersRef.current.clear();
  }

  function forgetExpiredProfiles(nowMs = Date.now()) {
    for (const [profileId, resetAtMs] of exhaustedProfilesRef.current) {
      if (resetAtMs > nowMs) continue;
      exhaustedProfilesRef.current.delete(profileId);
      const timer = availabilityTimersRef.current.get(profileId);
      if (timer) window.clearTimeout(timer);
      availabilityTimersRef.current.delete(profileId);
    }
  }

  /** Agenda a liberação sem depender de novo output do terminal. O callback se reagenda caso o
   * navegador limite timeouts muito longos ou o computador acorde antes do horário de reset. */
  function markExhaustedUntil(profileId: string, profileName: string, resetAtMs: number) {
    const previous = availabilityTimersRef.current.get(profileId);
    if (previous) window.clearTimeout(previous);
    exhaustedProfilesRef.current.set(profileId, resetAtMs);

    const release = () => {
      const remaining = resetAtMs - Date.now();
      if (remaining > 0) {
        const timer = window.setTimeout(release, Math.min(remaining, 2_147_483_647));
        availabilityTimersRef.current.set(profileId, timer);
        return;
      }
      if (exhaustedProfilesRef.current.get(profileId) === resetAtMs) {
        exhaustedProfilesRef.current.delete(profileId);
        setFailoverStatus({
          tone: "success",
          text: `${profileName} disponível`,
          detail: `A cota de ${profileName} foi liberada no horário de reset e a conta voltou ao rodízio automático.`,
        });
      }
      availabilityTimersRef.current.delete(profileId);
    };
    const timer = window.setTimeout(release, Math.min(Math.max(0, resetAtMs - Date.now()), 2_147_483_647));
    availabilityTimersRef.current.set(profileId, timer);
  }

  useEffect(() => () => clearAvailabilityTimers(), []);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let cancelled = false;
    let pollTimer: number | undefined;
    /** Fork: há um `pollTimer` pendente (nem em voo nem parado) — só aí dá para antecipar a leitura. */
    let agendado = false;
    const agendar = (ms: number) => {
      agendado = true;
      pollTimer = window.setTimeout(() => { agendado = false; void poll(); }, ms);
    };
    const aoVoltar = () => {
      if (cancelled || document.hidden || !agendado) return;
      window.clearTimeout(pollTimer);
      agendar(0);
    };
    document.addEventListener("visibilitychange", aoVoltar);
    let firstPoll = true;
    /** Fork (energia): o engine respondeu com `estado` = sabe esperar. `null` até a 1ª leitura. */
    let estadoConhecido: string | null = null;
    /** Escritas do histórico em andamento no xterm. Enquanto > 0, o que ele "responde" sai de
     *  perguntas antigas (posição do cursor, cor do fundo que o shell fez ao abrir) e não vai para
     *  a PTY: chegava ao prompt como letras soltas — "R", "1", ":". */
    let reproduzindo = 0;
    /** Esta montagem se liga a uma sessão que já existia e vai ler do começo: o que vier na primeira
     *  leitura é histórico. Sessão recém-criada aqui fica de fora — a saída dela é ao vivo e o shell
     *  pode estar esperando a resposta. */
    const reatando = Boolean(sessionRef.current) && sequenceRef.current === 0;
    let reviving = false;
    let failoverInProgress = false;
    /** Antes disso não tenta de novo; 0 = nenhuma tentativa frustrada ainda. */
    let failoverRetryAt = 0;
    /** Última tecla digitada ou saída recebida — decide o ritmo da leitura. */
    let atividadeEm = Date.now();
    /** Fork (energia): leituras seguidas sem saída nova. */
    let vazias = 0;
    const terminal = new Terminal({
      convertEol: true,
      cursorBlink: true,
      fontFamily: FONTE_MONO,
      fontSize: 13,
      scrollback: 10_000,
      // Fundo transparente no xterm; a cor sai do container, com a opacidade de Configurações → Fundo.
      allowTransparency: true,
      theme: {
        background: "#00000000",
        foreground: "#e2e2ec",
        cursor: "#f97316",
        selectionBackground: "#f973164d",
      },
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(host);
    fit.fit();

    terminal.attachCustomKeyEventHandler((event) =>
      handleTerminalKey(event, {
        hasSelection: () => terminal.hasSelection(),
        copySelection: () => {
          void navigator.clipboard.writeText(terminal.getSelection()).catch(() => undefined);
        },
        write: (data) => {
          const sessionId = sessionRef.current;
          if (sessionId) void writeTerminal(sessionId, data).catch(() => undefined);
        },
      })
    );

    async function start() {
      try {
        setError(null);
        await ensureEngine();
        if (!sessionRef.current) {
          criandoRef.current ??= criarSessao().finally(() => { criandoRef.current = null; });
          const session = await criandoRef.current;
          if (cancelled) return;
          setState(session.state);
        }
        poll();
      } catch (reason) {
        if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
      }
    }

    /** Abre a conversa (agente) e a PTY. Uma vez só por sessão, mesmo com o efeito rodando de novo:
     *  quem chega depois espera a mesma promessa em `criandoRef`. */
    async function criarSessao(): Promise<TerminalSession> {
          // Fork: branch escolhida no lançador roda na worktree dela (`launch.cwd`).
          const pastaDoAgente = launch?.cwd ?? projectPath;
          if (agent) await ensureAgentTrust(agent.id, pastaDoAgente, launch?.profile?.id);

          // A conversa é a entidade que sobrevive à troca de conta e de IA. Abri-la antes do
          // spawn é o que fixa o `--session-id` do Claude e, com ele, o caminho do transcript.
          let command = agent?.command;
          let externalSessionId: string | undefined;
          let planoTitulo: string | undefined;
          if (agent && !conversationRef.current) {
            const plan = launch?.conversationId
              ? null
              : await beginConversation({
                  projectId,
                  cwd: pastaDoAgente,
                  provider: agent.id,
                  profileId: launch?.profile?.id,
                  command: agent.command,
                  title: tituloRef.current,
                });
            conversationRef.current = plan?.conversation_id ?? launch?.conversationId ?? null;
            if (plan) {
              // Fork: a escolha do seletor acompanha esta aba; Codex local já traz seu modelo.
              command = plan.initial_command + opcoesDeModelo(agent.id, launch?.ajustes ?? ajustesDoAgente(agent.id));
              externalSessionId = plan.external_session_id ?? undefined;
              planoTitulo = plan.title;
            }
          }

          const session = await spawnTerminal({
            projectId,
            // Nome da **conversa**, não "Claude · agent": é ele que aparece na aba, na barra lateral
            // e na notificação, e o engine o atualiza sozinho quando o primeiro prompt der um nome.
            name: planoTitulo ?? (agent ? `${agent.label} · agent` : tituloRef.current),
            cwd: pastaDoAgente,
            rows: terminal.rows,
            cols: terminal.cols,
            initialCommand: command,
            provider: agent?.id,
            profileId: launch?.profile?.id,
            conversationId: conversationRef.current ?? undefined,
            externalSessionId,
          });
          // Grava mesmo se esta execução do efeito já foi cancelada: a próxima usa esta sessão.
          sessionRef.current = session.id;
          if (conversationRef.current) {
            await attachTerminal(conversationRef.current, session.id).catch(() => undefined);
          }
          onSessionCreated(session.id, session.name, agent?.id);
          return session;
    }

    /** Religa a sessão que o engine só conhece como histórica (engine reiniciado por update ou
     *  reabertura do app) ou cuja CLI saiu. Mesmo caminho do botão "reiniciar": mesmo id, e conversa
     *  Claude volta com `--resume`. Sem isto a aba ficava preta e digitar dava "session is not running". */
    async function revive(sessionId: string) {
      if (reviving) return;
      reviving = true;
      try {
        await restartTerminal(sessionId);
      } catch (reason) {
        if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
      } finally {
        reviving = false;
      }
    }

    /** Limite de uso é um evento do provider, não uma falha da PTY. Escolhe outra conta do mesmo
     * provider, prepara a continuação e só então encerra a sessão limitada. */
    async function autoFailover(input: {
      sessionId: string;
      conversationId: string;
      provider: AgentCliId;
      profileId: string;
      /** O limite chegou com o agente trabalhando: depois da troca, manda o "continue" sozinho. */
      meioDaTarefa: boolean;
    }) {
      if (failoverInProgress || Date.now() < failoverRetryAt) return;
      failoverInProgress = true;
      // Nova tentativa depois de "sem outra conta" não pisca o aviso: só troca se achar alguém.
      if (failoverRetryAt === 0) {
        setFailoverStatus({
          tone: "working",
          text: "trocando conta…",
          detail: "Limite de uso atingido; procurando outra conta conectada.",
        });
      }
      /** Libera nova tentativa mais tarde. Sem isto a flag ficava presa e a aba nunca mais trocava
       *  de conta, mesmo com o limite voltando à tela e outra conta já conectada. */
      const retryLater = () => {
        failoverRetryAt = Date.now() + FAILOVER_RETRY_MS;
        failoverInProgress = false;
      };

      try {
        const [profiles, agents] = await Promise.all([listProfiles(), listAgentClis(projectPath)]);
        const providerProfiles = profiles.filter(
          (profile) => profile.provider === input.provider && profile.authenticated
        );
        const usages = new Map(
          await Promise.all(providerProfiles.map(async (profile) => {
            const usage = await accountUsage(profile.id).catch(() => null);
            return [profile.id, usage] as const;
          }))
        );
        const nowMs = Date.now();
        forgetExpiredProfiles(nowMs);
        // Revalida todas as alternativas pelo cache local. Isso recompõe os bloqueios mesmo se o
        // app reiniciou no meio da janela e evita tentar uma segunda conta que já está em 100%.
        for (const profile of providerProfiles) {
          if (profile.id === input.profileId) continue;
          const resetAtMs = exhaustedUntilMs(usages.get(profile.id) ?? null, nowMs);
          if (resetAtMs) markExhaustedUntil(profile.id, profile.name, resetAtMs);
        }
        const current = profiles.find((profile) => profile.id === input.profileId);
        const currentResetAtMs = usageLimitResetAtMs(usages.get(input.profileId) ?? null, nowMs);
        markExhaustedUntil(input.profileId, current?.name ?? input.profileId, currentResetAtMs);
        const target = selectFailoverProfile(
          profiles,
          input.provider,
          input.profileId,
          exhaustedProfilesRef.current.keys()
        );
        if (!target) {
          setFailoverStatus({
            tone: "warning",
            text: "sem outra conta",
            detail: `O limite de ${current?.name ?? "esta conta"} foi atingido e não há outra conta conectada disponível. Ela volta ao rodízio em ${new Date(currentResetAtMs).toLocaleString("pt-BR")}. Uma conta conectada depois disso é usada automaticamente.`,
          });
          retryLater();
          return;
        }

        const nextAgent = agents.find((candidate) => candidate.id === input.provider && candidate.available);
        if (!nextAgent) throw new Error(`CLI do ${input.provider} não encontrada no PATH`);
        const plan = await planSwitch({
          conversationId: input.conversationId,
          targetProvider: input.provider,
          targetProfileId: target.id,
          targetCommand: nextAgent.command,
        });
        if (cancelled) return;

        await stopTerminal(input.sessionId).catch(() => undefined);
        sessionRef.current = undefined;
        sequenceRef.current = 0;
        conversationRef.current = plan.conversation_id;
        setFailoverStatus({
          tone: "success",
          text: `conta trocada: ${current?.name ?? "anterior"} → ${target.name}`,
          detail: `${plan.notice ? `${plan.notice} ` : ""}Limite de ${current?.name ?? "outra conta"} atingido; a conversa foi reaberta em ${target.name} e espera a sua próxima mensagem. ${current?.name ?? "A conta anterior"} volta ao rodízio em ${new Date(currentResetAtMs).toLocaleString("pt-BR")}.`,
        });
        // Retoma em silêncio: nenhum prompt é digitado no terminal. Um texto automático ("retome a
        // tarefa pendente") aparecia como se fosse do usuário e fazia o agente inventar trabalho —
        // com um simples "oi" sem resposta, ele saiu rodando comandos. Quem continua é o usuário.
        const resumedCommand = plan.initial_command;
        staleLimitProfileRef.current = target.id;
        if (input.meioDaTarefa) {
          continuarRef.current = { conversationId: plan.conversation_id, ate: Date.now() + CONTINUAR_ATE_MS, consultaEm: 0 };
        }
        setLaunch({
          agent: { ...nextAgent, command: resumedCommand },
          profile: target,
          conversationId: plan.conversation_id,
          // Fork: troca de conta continua na mesma pasta (worktree da branch, se for o caso).
          cwd: launch?.cwd ?? pastaDaSessaoRef.current,
        });
      } catch (reason) {
        if (!cancelled) {
          setFailoverStatus({
            tone: "warning",
            text: "troca automática falhou",
            detail: reason instanceof Error ? reason.message : String(reason),
          });
          retryLater();
        }
      }
    }

    /** Depois de trocar de conta no meio de uma tarefa: espera a CLI da conta nova mostrar o campo
     *  pronto e manda o "continue" pela fila do engine (a mesma porta do chat), uma vez só. */
    async function talvezContinuar() {
      const pedido = continuarRef.current;
      if (!pedido || cancelled) return;
      const agora = Date.now();
      if (agora > pedido.ate) { continuarRef.current = null; return; }
      if (agora < pedido.consultaEm) return;
      pedido.consultaEm = agora + CONTINUAR_RITMO_MS;
      try {
        const caminho = `/conversas/${encodeURIComponent(pedido.conversationId)}`;
        const leitura = await engineApi("GET", `${caminho}/timeline`, null);
        const capacidades = JSON.parse(leitura.body || "{}")?.conversation?.capabilities;
        if (!capacidades?.prompt || continuarRef.current !== pedido) return;
        continuarRef.current = null;
        const envio = await engineApi("POST", `${caminho}/prompt`, JSON.stringify({ texto: MENSAGEM_DE_CONTINUAR, fila: true }), {
          headers: [["Content-Type", "application/json"], ["Idempotency-Key", `continuar-${pedido.conversationId}-${pedido.ate}`]],
        });
        if (envio.status >= 400) throw new Error(JSON.parse(envio.body || "{}")?.error ?? `HTTP ${envio.status}`);
        setFailoverStatus((atual) => atual && { ...atual, text: `${atual.text} · continuando`, detail: `${atual.detail} O ${NOME_CURTO} mandou "continue" sozinho, porque a troca aconteceu no meio de uma tarefa.` });
      } catch (reason) {
        if (!cancelled) setFailoverStatus((atual) => atual && { ...atual, tone: "warning", detail: `${atual.detail} Não consegui mandar o "continue" sozinho (${reason instanceof Error ? reason.message : String(reason)}); mande a próxima mensagem.` });
      }
    }

    async function poll() {
      if (cancelled || !sessionRef.current) return;
      const pedidoEm = Date.now();
      try {
        const snapshot = estadoConhecido
          ? await terminalSnapshotEspera(sessionRef.current, sequenceRef.current, estadoConhecido,
            continuarRef.current ? ESPERA_CURTA_MS : ESPERA_MS)
          : await terminalSnapshot(sessionRef.current, sequenceRef.current);
        // A espera pode durar segundos: a aba pode ter fechado ou trocado de sessão nesse meio-tempo.
        if (cancelled) return;
        estadoConhecido = snapshot.estado ?? null;
        // Sessão histórica: o engine devolve `next_seq` 0 e nenhuma saída. Só na primeira leitura,
        // para não ressuscitar em loop quem o usuário parou nesta mesma montagem.
        // Fork (retomar-sempre): `orphan` (pasta sumiu) também tenta voltar: o erro do engine vira as
        // saídas de `PastaSumida` em vez de uma aba preta sem botão.
        const dead = snapshot.session.state === "stopped" || snapshot.session.state === "crashed" || snapshot.session.state === "orphan";
        const historico = firstPoll && reatando;
        if (firstPoll && dead && snapshot.next_seq === 0) void revive(sessionRef.current);
        firstPoll = false;
        if (snapshot.from_seq > sequenceRef.current) terminal.write("\r\n[scrollback anterior descartado]\r\n");
        vazias = snapshot.data ? 0 : vazias + 1;
        if (snapshot.data) {
          // Primeira leitura desta montagem = histórico. As respostas do xterm a ele são descartadas.
          if (historico || reproduzindo > 0) {
            reproduzindo += 1;
            terminal.write(snapshot.data, () => { reproduzindo -= 1; });
          } else {
            terminal.write(snapshot.data);
            if (cliNaoAchouAConversa(snapshot.data)) setConversaNaoAchada(true); // Fork (retomar-sempre)
          }
        }
        setState(snapshot.session.state);
        setInputLocked(Boolean(snapshot.session.input_locked));
        const conversationId = snapshot.session.conversation_id ?? conversationRef.current;
        if (conversationId && failoverConversationRef.current !== conversationId) {
          failoverConversationRef.current = conversationId;
          clearAvailabilityTimers();
          exhaustedProfilesRef.current.clear();
          setFailoverStatus(null);
        }
        if (conversationId) conversationRef.current = conversationId;
        // Mantém o que já sabia quando o campo vier vazio: o engine só preenche `external_session_id`
        // depois que o provider grava a sessão dele.
        setSessao((anterior) => {
          const proximo = {
            provider: snapshot.session.provider ?? anterior.provider,
            profileId: snapshot.session.profile_id ?? anterior.profileId,
            externalSessionId: snapshot.session.external_session_id ?? anterior.externalSessionId,
          };
          const igual =
            proximo.provider === anterior.provider &&
            proximo.profileId === anterior.profileId &&
            proximo.externalSessionId === anterior.externalSessionId;
          // Isto roda 10x por segundo: devolver o objeto anterior evita re-render a cada poll.
          return igual ? anterior : proximo;
        });
        terminal.options.disableStdin = Boolean(snapshot.session.input_locked);
        sequenceRef.current = snapshot.next_seq;
        const provider = snapshot.session.provider as AgentCliId | null | undefined;
        const profileId = snapshot.session.profile_id;
        const limitOnScreen = snapshot.session.notice === "usage_limit";
        if (snapshot.session.state === "working" && !limitOnScreen) trabalhouEmRef.current = Date.now();
        void talvezContinuar();
        if (profileId && profileId === staleLimitProfileRef.current && !limitOnScreen) {
          staleLimitProfileRef.current = null;
        }
        if (
          limitOnScreen &&
          profileId !== staleLimitProfileRef.current &&
          conversationId &&
          provider &&
          MULTI_ACCOUNT_PROVIDERS.includes(provider) &&
          profileId
        ) {
          void autoFailover({
            sessionId: snapshot.session.id,
            conversationId,
            provider,
            profileId,
            meioDaTarefa: Date.now() - trabalhouEmRef.current < MEIO_DA_TAREFA_MS,
          });
        }
        // Voltou a responder: derruba um aviso que tenha sobrado. Atualização funcional porque
        // isto roda 10x por segundo — devolver o mesmo valor faz o React nem re-renderizar.
        falhasRef.current = 0;
        setError((anterior) => (anterior === null ? anterior : null));
      } catch (reason) {
        falhasRef.current += 1;
        // Uma falha isolada não vira banner: o poll é a cada 100ms e qualquer soluço momentâneo
        // do engine deixava o erro colado na tela até trocar de projeto e voltar (o remount era a
        // única coisa que limpava).
        if (!cancelled && falhasRef.current >= FALHAS_ATE_AVISAR) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      }
      // Enquanto está falhando, espaça as tentativas: martelar de 100 em 100ms um engine que caiu
      // só gasta socket e atrasa a recuperação.
      if (!cancelled) {
        const ativo = Date.now() - atividadeEm < JANELA_ATIVA_MS;
        const intervalo = falhasRef.current >= FALHAS_ATE_AVISAR
          ? 1_000
          // Long-poll: a próxima espera sai já (no máximo uma leitura por quadro).
          : estadoConhecido ? Math.max(0, QUADRO_MS - (Date.now() - pedidoEm))
          : document.hidden ? POLL_OCULTO_MS
          : ativo ? POLL_ATIVO_MS
          : vazias >= VAZIAS_ATE_ESPACAR ? (document.hasFocus() ? POLL_CALADO_MS : POLL_CALADO_SEM_FOCO_MS)
          : POLL_OCIOSO_MS;
        agendar(intervalo);
      }
    }

    const dataSubscription = terminal.onData((data) => {
      if (reproduzindo > 0) return;
      // Digitou: a leitura acelera para o eco aparecer no quadro seguinte, não em até 100ms.
      atividadeEm = Date.now();
      // Fork (energia): com a leitura espaçada, antecipa a próxima em vez de esperar até 500 ms.
      if (agendado && vazias >= VAZIAS_ATE_ESPACAR) { window.clearTimeout(pollTimer); agendar(0); }
      const sessionId = sessionRef.current;
      if (sessionId) void writeTerminal(sessionId, data).catch((reason) => {
        if (String(reason).includes("Consulta de uso")) setInputLocked(true);
        // Digitar numa sessão morta é pedido para voltar a ela, não motivo de banner.
        else if (String(reason).includes("session is not running")) void revive(sessionId);
        else setError(String(reason));
      });
    });
    const resizeObserver = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(() => {
      fit.fit();
      if (sessionRef.current) {
        void resizeTerminal(sessionRef.current, terminal.rows, terminal.cols).catch(() => undefined);
      }
    });
    resizeObserver?.observe(host);

    // Captura na fase de captura (antes do próprio textarea do xterm ver o evento): xterm só lê
    // `clipboardData.getData("text/plain")`, então colar uma imagem (sem texto no clipboard) não
    // faria nada sozinho.
    //
    // Numa aba de agente quem sabe lidar com imagem é a própria CLI — ela lê o clipboard do
    // sistema e anexa como `[Image #N]`. O papel daqui é só entregar a tecla: manda a mesma
    // sequência que o Alt+V já mandava (ESC v, meta+v), que é o atalho de colar imagem que
    // funciona nessas CLIs. Salvar em disco e digitar o caminho continua sendo o caminho de
    // terminal puro, onde não há ninguém pra ler o clipboard.
    async function handleImagePaste(event: ClipboardEvent) {
      const files = event.clipboardData?.files;
      const imageFile = files && Array.from(files).find((file) => file.type.startsWith("image/"));
      if (!imageFile || !sessionRef.current) return;
      event.preventDefault();
      event.stopPropagation();
      if (agent) {
        void writeTerminal(sessionRef.current, CLI_IMAGE_PASTE).catch(() => undefined);
        return;
      }
      try {
        const buffer = new Uint8Array(await imageFile.arrayBuffer());
        const extension = imageFile.type.split("/")[1] || "png";
        // O caminho é digitado cru na PTY, sem aspas — espaço e vírgula no nome (o padrão de
        // print do ChatGPT, p.ex.) fariam a CLI ler só o primeiro pedaço. Troca tudo que não for
        // seguro por "-" em vez de tentar citar pra cada shell.
        const rawName = /\.\w+$/.test(imageFile.name) ? imageFile.name : `pasted-${Date.now()}.${extension}`;
        const fileName = rawName.replace(/[^\w.-]+/g, "-");
        const absolutePath = joinPath(projectPath, ".omni-agents", "pasted", fileName);
        await writeBinaryFile(projectPath, absolutePath, buffer);
        await writeTerminal(sessionRef.current, absolutePath);
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
      }
    }
    host.addEventListener("paste", handleImagePaste, { capture: true });

    void start();

    return () => {
      cancelled = true;
      if (pollTimer) window.clearTimeout(pollTimer);
      document.removeEventListener("visibilitychange", aoVoltar);
      resizeObserver?.disconnect();
      host.removeEventListener("paste", handleImagePaste, { capture: true });
      dataSubscription.dispose();
      terminal.dispose();
    };
  }, [agent, launch?.profile?.id, launch?.cwd, paneId, projectId, projectPath, retry, tab.id, onSessionCreated]);

  /** Aplica a troca preparada pelo backend: para a sessão atual e relança na conta/IA nova,
   *  mantendo a mesma conversa. Não abre aba nem conversa nova — é a mesma timeline. */
  async function applySwitch(plan: LaunchPlan, nextAgent: AgentCliStatus, profile: Profile | null) {
    const current = sessionRef.current;
    setSwitching(false);
    if (current) await stopTerminal(current).catch(() => undefined);
    sessionRef.current = undefined;
    sequenceRef.current = 0;
    conversationRef.current = plan.conversation_id;
    setFailoverStatus(null);
    if (plan.notice) setError(plan.notice);
    setLaunch({
      agent: { ...nextAgent, command: plan.initial_command },
      profile,
      conversationId: plan.conversation_id,
      cwd: launch?.cwd ?? pastaDaSessaoRef.current,
    });
  }

  // Aba de terminal puro não passa pelo seletor de CLI: sobe direto no shell do sistema.
  if (tab.kind !== "terminal" && !tab.resourceId && !agent) {
    const projectName = projectPath.split(/[\\/]/).filter(Boolean).pop() ?? projectPath;
    return <AgentLauncher projectName={projectName} projectPath={projectPath} onLaunch={setLaunch} iniciarDireto={tab.iniciarDireto} />;
  }

  return (
    <div className="relative h-full min-h-0" style={{ backgroundColor: "rgb(14 14 20 / var(--terminal-opacity, 1))" }}>
      <div
        ref={hostRef}
        className="absolute inset-0 p-2"
        aria-label={`Terminal ${tab.title}`}
        data-omni-context="terminal"
      />
      <div className="absolute right-3 top-2 z-10 flex items-center gap-2 bg-[#0e0e14]/90 px-2 py-1 text-[10px] uppercase text-text-muted">
        <span>{stateGlyph(state)} {state.replace(/_/g, " ")}</span>
        {failoverStatus && (
          <span
            role="status"
            title={failoverStatus.detail}
            className={
              failoverStatus.tone === "success"
                ? "text-success"
                : failoverStatus.tone === "warning"
                  ? "text-warning"
                  : "text-accent"
            }
          >
            {failoverStatus.text}
          </span>
        )}
        {inputLocked && <span className="text-xs text-warning">Consultando /usage…</span>}
        {state === "working" && sessionRef.current && (
          <button
            type="button"
            className="text-danger hover:underline"
            onClick={() => {
              const sessionId = sessionRef.current;
              if (!sessionId) return;
              void stopTerminal(sessionId).then(() => setState("stopped")).catch((reason) => setError(String(reason)));
            }}
          >
            parar
          </button>
        )}
        {sessionRef.current && state !== "orphan" && (
          <button
            type="button"
            className="hover:underline"
            onClick={() => {
              const sessionId = sessionRef.current;
              if (!sessionId) return;
              void restartTerminal(sessionId).catch((reason) => setError(String(reason)));
            }}
          >
            reiniciar
          </button>
        )}
        {conversationRef.current && agent && (
          <button type="button" className="hover:underline" onClick={() => setSwitching(true)}>
            trocar conta / IA
          </button>
        )}
      </div>
      {(sessao.provider ?? agent?.id) && (
        <AgentRuntimeBadge
          provider={(sessao.provider ?? agent?.id) as string}
          profileId={sessao.profileId ?? launch?.profile?.id}
          cwd={projectPath}
          externalSessionId={sessao.externalSessionId}
          className="absolute bottom-2 right-3 z-10"
        />
      )}
      {switching && conversationRef.current && agent && (
        <AgentSwitcher
          conversationId={conversationRef.current}
          currentProvider={agent.id}
          currentProfileId={launch?.profile?.id ?? null}
          onCancel={() => setSwitching(false)}
          onSwitch={(plan, nextAgent, profile) => void applySwitch(plan, nextAgent, profile)}
        />
      )}
      {conversaNaoAchada && sessionRef.current && (
        <ConversaNaoAchada
          sessao={{ sessionId: sessionRef.current, projectId, projectPath, cwd: pastaDaSessaoRef.current ?? projectPath,
            provider: sessao.provider, profileId: sessao.profileId, externalSessionId: sessao.externalSessionId, titulo: tab.title }}
          onNova={(sessionId, titulo) => { setConversaNaoAchada(false); onSessionCreated(sessionId, titulo, sessao.provider); }}
          onFechar={() => setConversaNaoAchada(false)}
        />
      )}
      {error && sessionRef.current && ehPastaSumida(error) && (
        <div role="alert" className="absolute inset-x-3 top-3 z-10 border-2 border-danger bg-bg-elevated p-3 text-xs text-danger">
          <PastaSumida
            erro={error}
            sessao={{ sessionId: sessionRef.current, projectId, projectPath, cwd: pastaDaSessaoRef.current ?? projectPath,
              provider: sessao.provider, profileId: sessao.profileId, externalSessionId: sessao.externalSessionId, titulo: tab.title }}
            onReaberta={() => { setError(null); setRetry((value) => value + 1); }}
          />
        </div>
      )}
      {error && !(sessionRef.current && ehPastaSumida(error)) && (
        <div role="alert" className="absolute inset-x-3 top-3 z-10 border-2 border-danger bg-bg-elevated p-3 text-xs text-danger">
          <p>{error}</p>
          <Button
            size="sm"
            variant="danger"
            className="mt-2"
            onClick={() => {
              const sessionId = sessionRef.current;
              if (sessionId && error.includes("session is not running")) {
                setError(null);
                void restartTerminal(sessionId).catch((reason) => setError(String(reason)));
                return;
              }
              if (isSessionMissing(error)) sessionRef.current = undefined;
              setRetry((value) => value + 1);
            }}
          >
            {isSessionMissing(error) ? "Iniciar nova sessão" : "Tentar reconectar"}
          </Button>
        </div>
      )}
    </div>
  );
}

function isSessionMissing(error: string) {
  return error.includes("session not found");
}

function stateGlyph(state: TerminalState) {
  switch (state) {
    case "working": return "▸";
    case "answered": return "●";
    case "approval_required": return "⏵";
    case "crashed": return "✖";
    case "stopped": return "○";
    case "orphan": return "⚠";
  }
}
