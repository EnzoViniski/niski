import { useEffect, useState } from "react";
import { ChevronDownIcon, CloseIcon, FolderIcon, GitBranchIcon, ReloadIcon } from "../../components/ui/PixelIcon";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { IconeDoStatus, rotuloDoStatus, statusDaConversa } from "../../components/ConversaStatus";
import { conversationKey } from "../terminal/useAttention";
import { corDoAgente, nomeDoAgente } from "../../lib/coresDosAgentes";
import type { Project } from "../../types/workspace";
import type { TerminalSession } from "../terminal/terminalService";

const CHAVE_ABERTAS = "omni-pastas-abertas";

function lerAbertas(): Set<string> | null {
  try {
    const salvo = JSON.parse(localStorage.getItem(CHAVE_ABERTAS) ?? "null");
    return Array.isArray(salvo) ? new Set(salvo.filter((id): id is string => typeof id === "string")) : null;
  } catch {
    return null;
  }
}

interface Props {
  projects: Project[];
  activeProjectId: string | null;
  activeSessionId: string | null;
  /** Conversas de agente de todos os projetos (uma por conversa). */
  agentSessions: TerminalSession[];
  naoVistas: Set<string>;
  onSelectProject: (projectId: string) => void;
  onCloseProject: (projectId: string) => void;
  onAbrirConversa: (projectId: string, session: TerminalSession) => void;
  onBifurcar: (sessionId: string) => void;
  onReiniciar: (sessionId: string) => void;
  onFechar: (sessionId: string) => void;
}

/**
 * Projetos como pastas: cada uma abre e mostra as conversas dela, e várias podem ficar abertas ao
 * mesmo tempo. A cor da barra e do ícone de cada conversa diz qual agente roda nela (Claude, Codex,
 * Antigravity…); o ícone à direita, o estado (trabalhando, pedindo aprovação…).
 */
export function PastasDeProjetos({
  projects, activeProjectId, activeSessionId, agentSessions, naoVistas,
  onSelectProject, onCloseProject, onAbrirConversa, onBifurcar, onReiniciar, onFechar,
}: Props) {
  const [abertas, setAbertas] = useState<Set<string>>(() => lerAbertas() ?? new Set(activeProjectId ? [activeProjectId] : []));

  useEffect(() => {
    try { localStorage.setItem(CHAVE_ABERTAS, JSON.stringify([...abertas])); } catch { /* sem armazenamento: vale só agora */ }
  }, [abertas]);

  const alternar = (id: string) => setAbertas((atual) => {
    const nova = new Set(atual);
    if (nova.has(id)) nova.delete(id); else nova.add(id);
    return nova;
  });

  if (projects.length === 0) {
    return <p data-nav-item className="px-3 py-1.5 text-xs text-text-muted">Nenhum projeto ainda.</p>;
  }

  return (
    <ul className="pastas space-y-0.5" aria-label="Projetos">
      {projects.map((project) => {
        const aberta = abertas.has(project.id);
        const ativa = project.id === activeProjectId;
        const conversas = agentSessions.filter((session) => session.project_id === project.id);
        return (
          <li key={project.id}>
            <div className={`pasta group${ativa ? " is-ativa" : ""}`}>
              <button type="button" data-nav-item aria-expanded={aberta} aria-current={ativa ? "page" : undefined}
                title={project.path}
                onClick={() => {
                  // Clicar na pasta já ativa só abre/fecha; numa outra, seleciona e garante aberta.
                  if (ativa) alternar(project.id);
                  else {
                    onSelectProject(project.id);
                    setAbertas((atual) => new Set(atual).add(project.id));
                  }
                }}
                className="pasta-botao">
                <ChevronDownIcon aria-hidden className={`h-3 w-3 shrink-0 transition-transform ${aberta ? "" : "-rotate-90"}`} />
                <FolderIcon aria-hidden className="h-3.5 w-3.5 shrink-0 text-accent" />
                <span className="min-w-0 flex-1 truncate">{project.name}</span>
                {conversas.length > 0 && <span className="pasta-contagem">{conversas.length}</span>}
              </button>
              <button type="button" aria-label={`Fechar projeto ${project.name}`} title="Fechar projeto"
                onClick={() => onCloseProject(project.id)} className="pasta-acao hover:text-danger">
                <CloseIcon aria-hidden className="h-3 w-3" />
              </button>
            </div>

            {aberta && (
              <ul className="pasta-conversas" aria-label={`Conversas de ${project.name}`}>
                {conversas.length === 0 && <li className="px-2 py-1 text-[11px] text-text-muted">Nenhuma conversa aberta.</li>}
                {conversas.map((session) => {
                  const status = statusDaConversa(session, naoVistas.has(conversationKey(session)));
                  const emFoco = session.id === activeSessionId;
                  return (
                    <li key={session.id} className={`conversa-linha group${emFoco ? " is-ativa" : ""}`}
                      style={{ "--cor-agente": corDoAgente(session.provider) } as React.CSSProperties}>
                      <button type="button" data-nav-item onClick={() => onAbrirConversa(project.id, session)}
                        title={`${nomeDoAgente(session.provider)} · ${rotuloDoStatus(status)}`}
                        className="conversa-botao">
                        <AgentIcon provider={session.provider} size={13} className="shrink-0" />
                        <span className="min-w-0 flex-1 truncate">{session.name}</span>
                        <IconeDoStatus status={status} className="h-3 w-3" />
                      </button>
                      <span className="conversa-acoes">
                        <button type="button" aria-label={`Bifurcar conversa ${session.name}`} title="Bifurcar: conversa nova com todo o histórico desta"
                          onClick={() => onBifurcar(session.id)}><GitBranchIcon aria-hidden className="h-3 w-3" /></button>
                        <button type="button" aria-label={`Reiniciar agente ${session.name}`} title="Reiniciar sessão"
                          onClick={() => onReiniciar(session.id)}><ReloadIcon aria-hidden className="h-3 w-3" /></button>
                        <button type="button" aria-label={`Fechar agente ${session.name}`} title="Encerrar e remover agente"
                          onClick={() => onFechar(session.id)} className="hover:text-danger"><CloseIcon aria-hidden className="h-3 w-3" /></button>
                      </span>
                    </li>
                  );
                })}
              </ul>
            )}
          </li>
        );
      })}
    </ul>
  );
}
