import { useEffect, useMemo, useState } from "react";
import { Button, Select } from "../../components/ui";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { ChatIcon, FolderIcon } from "../../components/ui/PixelIcon";
import { HistoryView } from "../history/HistoryView";
import { targetLabel } from "../../lib/paths";
import { levarParaConta, resumeCommand, type HistoryEntry, type HistoryProvider } from "../history/historyService";
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
];

export function AgentLauncher({ projectName, projectPath, onLaunch }: AgentLauncherProps) {
  const [agents, setAgents] = useState(FALLBACK_AGENTS);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [selected, setSelected] = useState<AgentCliId>(FALLBACK_AGENTS[0].id);
  const [selectedProfile, setSelectedProfile] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([listAgentClis(projectPath), listProfiles()])
      .then(([statuses, accounts]) => {
        if (cancelled) return;
        setProfiles(accounts);
        if (statuses.length === 0) return;
        setAgents(statuses);
        const firstAvailable = statuses.find((item) => item.available);
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
    const preferred = accounts.find((item) => item.authenticated) ?? accounts[0];
    setSelectedProfile(preferred?.id ?? "");
  }, [accounts]);

  const profile = accounts.find((item) => item.id === selectedProfile) ?? null;

  const profileOptions = accounts.map((item) => ({
    value: item.id,
    label: item.name,
    hint: item.authenticated ? "conectado" : "sem login",
  }));

  function launch(command?: string) {
    if (!agent) return;
    onLaunch({ agent: command ? { ...agent, command } : agent, profile });
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
              <Button variant="ghost" size="sm" onClick={() => launch(`${agent.command} ${AGENT_RESUME_FLAG[agent.id]}`)}>
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

  return (
    <div className="launcher">
      <section className="launcher-card" aria-labelledby="launcher-titulo">
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
          {agents.map((item) => {
            const status = statusDe(item);
            const marcado = item.id === selected;
            return (
              <button
                key={item.id}
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
          <Button variant="primary" className="flex-1" disabled={loading || !agent?.available} onClick={() => launch()}>
            {loading ? (
              "Detectando CLIs…"
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
