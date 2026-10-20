import { useEffect, useMemo, useState } from "react";
import { Button, Select } from "../../components/ui";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { ChatIcon, FolderIcon } from "../../components/ui/PixelIcon";
import { HistoryView } from "../history/HistoryView";
import { Mascote } from "../workspace/PainelVazio";
import { targetLabel } from "../../lib/paths";
import { contaDoAgente, lembrarAgente, lembrarConta, ultimoAgente } from "../../lib/escolhasSalvas";
import { levarParaConta, resumeCommand, type HistoryEntry, type HistoryProvider } from "../history/historyService";
import { gitBranches, gitWorktreeDaBranch, type GitBranch } from "../git/gitService";
import { isRemoteProject } from "../projects/targetState";
import { criarWorktreeIsolada, nomeDaWorktree, PASTA_DAS_WORKTREES, PREFIXO_DA_BRANCH } from "./worktreeIsolada";
// Fork (3B): conta "Automática" pela Fila de contas, com o porquê.
import { CONTA_AUTOMATICA } from "../roteamento/roteamentoService";
import { PorQueEstaConta, useContaAutomatica } from "../roteamento/PorQueEstaConta";
import {
  AGENT_RESUME_FLAG,
  listAgentClis,
  listProfiles,
  type AgentCliId,
  type AgentCliStatus,
  type AgentLaunch,
  type Profile,
} from "./terminalService";

interface AgentLauncherProps {
  projectName: string;
  projectPath: string;
  onLaunch: (launch: AgentLaunch) => void;
}

const FALLBACK_AGENTS: AgentCliStatus[] = [
  { id: "claude", label: "Claude", command: "claude", path: null, available: false, authenticated: false },
  { id: "codex", label: "Codex", command: "codex", path: null, available: false, authenticated: false },
  { id: "cursor", label: "Cursor", command: "agent", path: null, available: false, authenticated: false },
  { id: "antigravity", label: "Antigravity", command: "agy", path: null, available: false, authenticated: false },
];

export function AgentLauncher({ projectName, projectPath, onLaunch }: AgentLauncherProps) {
  const [agents, setAgents] = useState(FALLBACK_AGENTS);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [selected, setSelected] = useState<AgentCliId>(FALLBACK_AGENTS[0].id);
  const [selectedProfile, setSelectedProfile] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);
  // Fork: branch em que o agente vai trabalhar. Outra que não a atual roda numa worktree própria.
  const [branches, setBranches] = useState<GitBranch[]>([]);
  const [branch, setBranch] = useState<string>("");
  const [branchNova, setBranchNova] = useState("");
  const [preparando, setPreparando] = useState(false);
  // Fork: "Isolar em worktree" — cópia do repo só do agente, fora da pasta, na branch medza/<nome>.
  const [isolar, setIsolar] = useState(false);
  const [nomeDaWt, setNomeDaWt] = useState("");
  const [erroDaWt, setErroDaWt] = useState<string | null>(null);
  useEffect(() => {
    if (isRemoteProject(projectPath)) return;
    let vivo = true;
    gitBranches(projectPath).then((lista) => {
      if (!vivo) return;
      setBranches(lista);
      setBranch(lista.find((item) => item.current)?.name ?? "");
    }).catch(() => undefined);
    return () => { vivo = false; };
  }, [projectPath]);
  const branchAtual = branches.find((item) => item.current)?.name ?? "";
  const NOVA = "__nova__";

  useEffect(() => {
    let cancelled = false;
    void Promise.all([listAgentClis(projectPath), listProfiles()])
      .then(([statuses, accounts]) => {
        if (cancelled) return;
        setProfiles(accounts);
        if (statuses.length === 0) return;
        setAgents(statuses);
        // Fork: volta no último agente usado, se ele ainda estiver instalado.
        const lembrado = statuses.find((item) => item.available && item.id === ultimoAgente());
        const firstAvailable = lembrado ?? statuses.find((item) => item.available);
        if (firstAvailable) setSelected(firstAvailable.id);
      })
      // Engolir esta falha em silêncio deixava os quatro providers em `available: false` sem
      // nenhum sinal — metade do sintoma de "nenhum agente aparece conectado".
      .catch((reason) => {
        if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, [projectPath]);

  const agent = agents.find((item) => item.id === selected) ?? agents[0];

  // O agente é sempre o desta máquina (conta, histórico e /usage daqui). O que muda em projeto
  // remoto é onde os comandos dele rodam — quem desvia é o omni-shim, no engine.
  const alvo = targetLabel(projectPath);
  const accounts = useMemo(
    () => profiles.filter((profile) => profile.provider === selected),
    [profiles, selected]
  );

  // Prefere uma conta já autenticada: abrir no perfil sem login só porque veio primeiro na lista
  // joga o usuário numa tela de autenticação que ele não pediu.
  useEffect(() => {
    // Fork: a conta escolhida da última vez neste agente vem antes (some se a conta sumir).
    if (contaDoAgente(selected) === CONTA_AUTOMATICA && accounts.length > 1) { setSelectedProfile(CONTA_AUTOMATICA); return; }
    const preferred = accounts.find((item) => item.id === contaDoAgente(selected))
      ?? accounts.find((item) => item.authenticated) ?? accounts[0];
    setSelectedProfile(preferred?.id ?? "");
  }, [accounts, selected]);

  // Fork (3B): "Automática" pergunta à fila; a conta escolhida vale para abrir e o porquê aparece abaixo.
  const automatica = selectedProfile === CONTA_AUTOMATICA;
  const contaAutomatica = useContaAutomatica(selected, projectPath, automatica);
  const profile = automatica
    ? profiles.find((item) => item.id === contaAutomatica.escolha?.perfil_id) ?? null
    : accounts.find((item) => item.id === selectedProfile) ?? null;
  const semContaAutomatica = automatica && !profile;

  const profileOptions = [
    ...(accounts.length > 1 ? [{ value: CONTA_AUTOMATICA, label: "Automática (fila)", hint: "pela Fila de contas" }] : []),
    ...accounts.map((item) => ({
      value: item.id,
      label: item.name,
      hint: item.authenticated ? "conectado" : "sem login",
    })),
  ];

  async function launch(command?: string) {
    if (!agent) return;
    lembrarAgente(agent.id);
    if (profile) lembrarConta(agent.id, automatica ? CONTA_AUTOMATICA : profile.id);
    let cwd: string | undefined;
    const alvoBranch = branch === NOVA ? branchNova.trim() : branch;
    if (isolar) {
      setPreparando(true);
      setErroDaWt(null);
      try { cwd = (await criarWorktreeIsolada(projectPath, nomeDaWt.trim() || agent.id)).pasta_de_trabalho; }
      catch (reason) { setErroDaWt(reason instanceof Error ? reason.message : String(reason)); setPreparando(false); return; }
      setPreparando(false);
    } else if (alvoBranch && alvoBranch !== branchAtual) {
      setPreparando(true);
      setError(null);
      try { cwd = await gitWorktreeDaBranch(projectPath, alvoBranch, branch === NOVA); }
      catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); setPreparando(false); return; }
      setPreparando(false);
    }
    onLaunch({ agent: command ? { ...agent, command } : agent, profile, cwd });
  }

  // Reabre na conta escolhida no histórico (por padrão, a que gravou a conversa). Outra conta
  // recebe uma cópia do transcript antes, senão o `--resume` não acha a conversa.
  async function resume(entry: HistoryEntry, profileId: string) {
    if (!agent) return;
    await levarParaConta(entry, profileId);
    onLaunch({
      agent: { ...agent, command: resumeCommand(agent.command, entry) },
      profile: profiles.find((item) => item.id === profileId) ?? profile,
    });
  }

  if (picking && agent) {
    return (
      <div className="h-full overflow-hidden bg-bg-surface/80">
        <HistoryView
          lock={{ provider: agent.id as HistoryProvider, cwd: projectPath }}
          onResume={resume}
          actions={
            <>
              <Button variant="ghost" size="sm" onClick={() => void launch(`${agent.command} ${AGENT_RESUME_FLAG[agent.id]}`)}>
                Continuar a última
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setPicking(false)}>
                Voltar
              </Button>
            </>
          }
        />
      </div>
    );
  }

  // Status de cada CLI em uma palavra, com a cor que o cartão usa.
  const statusDe = (item: AgentCliStatus) =>
    !item.available
      ? { texto: "Não instalada", tom: "off" }
      : item.authenticated
        ? { texto: "Conectada", tom: "ok" }
        : { texto: "Sem login", tom: "aviso" };

  // O que a janelinha da cena "digita": o comando de verdade que vai abrir.
  const comando = agent?.command ?? "agent";
  const faiscas = [8, 19, 33, 47, 61, 74, 88];

  return (
    <div className="launcher">
      <section className="launcher-card lz-card" aria-labelledby="launcher-titulo">
        {/* Cena: remonta a cada troca de agente (key) — varredura, comando digitado e o herói chegando. */}
        <div key={agent?.id} className="lz-cena" aria-hidden>
          <span className="lz-varredura" />
          {faiscas.map((x, i) => (
            <span key={x} className="lz-faisca" style={{ left: `${x}%`, animationDelay: `${(i * 0.53) % 2.6}s` }} />
          ))}
          <div className="lz-janela">
            <div className="lz-janela-barra"><i /><i /><i /><span>{projectName}</span></div>
            <div className="lz-janela-corpo" style={{ "--n": comando.length } as React.CSSProperties}>
              <p className="lz-linha"><span className="lz-prompt">$</span> <span className="lz-digitando">{comando}</span><span className="lz-cursor" /></p>
              <p className="lz-linha lz-saida lz-saida-1">✓ {agent?.label ?? "Agente"} pronto em {projectName}</p>
              <p className="lz-linha lz-saida lz-saida-2">{profile ? `conta ${profile.name}` : "conta padrão"} · continua rodando em segundo plano</p>
            </div>
          </div>
          <div className="lz-heroi">
            {agent?.id === "claude" ? <Mascote /> : <AgentIcon provider={agent?.id} size={46} className="lz-heroi-icone" />}
            <span className="lz-sombra" />
          </div>
          <div className="lz-chao" />
        </div>

        <header className="launcher-topo">
          <span className="launcher-topo-icone" aria-hidden>
            <FolderIcon className="h-5 w-5" />
          </span>
          <div className="min-w-0">
            <p className="launcher-sobretitulo">Novo agente</p>
            <h2 id="launcher-titulo" className="launcher-titulo">{projectName}</h2>
            <p className="launcher-caminho" title={projectPath}>{projectPath}</p>
          </div>
        </header>

        {alvo && (
          <p className="launcher-nota">
            O agente roda neste PC, com a sua conta; os comandos dele executam em {alvo}.
          </p>
        )}

        <div role="radiogroup" aria-label="CLI do agente" className="launcher-agentes">
          {agents.map((item, indice) => {
            const status = statusDe(item);
            const marcado = item.id === selected;
            return (
              <button
                key={item.id}
                style={{ "--i": indice } as React.CSSProperties}
                type="button"
                role="radio"
                aria-checked={marcado}
                disabled={loading || !item.available}
                onClick={() => setSelected(item.id)}
                className="launcher-agente"
                title={item.available ? `${item.command} · ${status.texto.toLowerCase()}` : `${item.command} não foi encontrado no PATH`}
              >
                <AgentIcon provider={item.id} size={26} className="shrink-0" />
                <span className="launcher-agente-nome">{item.label}</span>
                <span className={`launcher-agente-status is-${status.tom}`}>
                  <span className="launcher-ponto" aria-hidden />
                  {loading ? "Verificando…" : status.texto}
                </span>
              </button>
            );
          })}
        </div>

        {profileOptions.length > 1 && (
          <div className="launcher-campo">
            <label htmlFor="agent-profile" className="launcher-rotulo">Conta</label>
            <Select
              id="agent-profile"
              value={selectedProfile}
              options={profileOptions}
              onChange={setSelectedProfile}
              disabled={loading}
            />
          </div>
        )}
        {automatica && (
          <div className="launcher-nota" aria-live="polite">
            {contaAutomatica.carregando ? "Consultando a fila de contas…"
              : contaAutomatica.erro ? `Não deu para consultar a fila: ${contaAutomatica.erro}`
              : contaAutomatica.escolha && (
                <>
                  <p>{profile ? `Vai abrir em ${profile.name}.` : "Nenhuma conta da fila serve agora: escolha uma conta."}</p>
                  <PorQueEstaConta escolha={contaAutomatica.escolha} className="mt-1" />
                </>
              )}
          </div>
        )}
        {branches.length > 0 && (
          <div className="launcher-campo">
            <label htmlFor="agent-branch" className="launcher-rotulo">Branch</label>
            <Select
              id="agent-branch"
              value={branch}
              searchable={branches.length > 6}
              options={[
                ...branches.map((item) => ({ value: item.name, label: item.name, hint: item.current ? "atual da pasta" : "pasta separada" })),
                { value: NOVA, label: "Nova branch…", hint: `a partir de ${branchAtual}` },
              ]}
              onChange={setBranch}
              disabled={loading || preparando || isolar}
            />
            {branch === NOVA && (
              <input className="launcher-branch-nova" autoFocus value={branchNova} onChange={(e) => setBranchNova(e.target.value)}
                placeholder="nome-da-branch" aria-label="Nome da branch nova" spellCheck={false} />
            )}
          </div>
        )}
        {branches.length > 0 && (
          <div className="launcher-campo">
            <label className="flex cursor-pointer items-center gap-2 text-xs text-text-secondary">
              <input type="checkbox" checked={isolar} disabled={loading || preparando}
                onChange={(event) => { setIsolar(event.target.checked); setErroDaWt(null); }} className="accent-[rgb(var(--c-accent))]" />
              Isolar em worktree
            </label>
            {isolar && (
              <input className="launcher-branch-nova" value={nomeDaWt} onChange={(e) => setNomeDaWt(e.target.value)}
                placeholder={agent?.id ?? "agente"} aria-label="Nome da worktree" spellCheck={false} />
            )}
          </div>
        )}
        {isolar && (
          <p className="launcher-nota">
            O agente trabalha numa cópia do repositório só dele, fora da pasta do projeto
            (../{PASTA_DAS_WORKTREES}/…/{nomeDaWorktree(nomeDaWt) || agent?.id || "agente"}), na branch nova{" "}
            {PREFIXO_DA_BRANCH}{nomeDaWorktree(nomeDaWt) || agent?.id || "agente"} a partir de {branchAtual}. Os arquivos do
            .worktreeinclude (ex.: .env) são copiados. Ao encerrar o agente, você escolhe manter ou apagar a worktree.
          </p>
        )}
        {erroDaWt && (
          <p role="alert" className="launcher-alerta is-erro">Não deu para criar a worktree: {erroDaWt}</p>
        )}
        {!isolar && branch && branch !== branchAtual && (
          <p className="launcher-nota">
            O agente trabalha numa cópia da pasta só dele (worktree do git) na branch {branch === NOVA ? (branchNova.trim() || "nova") : branch}.
            Os agentes desta pasta continuam em {branchAtual}, sem nada mudar debaixo deles.
          </p>
        )}
        {profile && !profile.authenticated && (
          <p className="launcher-alerta is-aviso">
            A conta “{profile.name}” ainda não tem login. O CLI vai pedir autenticação ao abrir.
          </p>
        )}
        {error && (
          <p role="alert" className="launcher-alerta is-erro">
            Falha ao consultar as CLIs instaladas: {error}
          </p>
        )}
        {!loading && !error && !agents.some((item) => item.available) && (
          <p role="alert" className="launcher-alerta is-aviso">
            Nenhuma CLI compatível foi encontrada no PATH. Instale uma delas e reinicie o OMNI Agents.
          </p>
        )}

        <div className="launcher-acoes">
          <Button variant="primary" className="flex-1 lz-abrir" disabled={loading || preparando || !agent?.available || (branch === NOVA && !branchNova.trim()) || semContaAutomatica} onClick={() => void launch()}>
            {loading ? (
              "Detectando CLIs…"
            ) : preparando ? (
              isolar ? "Criando a worktree…" : "Preparando a branch…"
            ) : (
              <span className="inline-flex items-center gap-1.5">
                {agent && <AgentIcon provider={agent.id} size={14} className="shrink-0" />}
                {`Abrir ${agent?.label ?? "agente"}`}
              </span>
            )}
          </Button>
          {agent && AGENT_RESUME_FLAG[agent.id] && (
            <Button
              variant="ghost"
              className="flex-1"
              disabled={loading || !agent.available}
              title="Escolhe uma conversa anterior deste projeto, com prévia das mensagens"
              onClick={() => setPicking(true)}
            >
              <ChatIcon className="h-3.5 w-3.5" aria-hidden="true" />
              Retomar conversa
            </Button>
          )}
        </div>
        <p className="launcher-rodape">O agente continua rodando mesmo se esta janela fechar.</p>
      </section>
    </div>
  );
}
