// Fork: "Na tela" reúne todas as abas abertas, inclusive em outros projetos e workspaces.
import { AGENT_ICON, AgentIcon } from "../../components/ui/AgentIcon";
import { BlocksIcon, ChatIcon, GitBranchIcon, GlobeIcon, KanbanIcon, ListIcon, TerminalIcon } from "../../components/ui/PixelIcon";
import { IconeDoStatus, statusDaConversa } from "../../components/ConversaStatus";
import { corDoAgente } from "../../lib/coresDosAgentes";
import { iconForPath } from "../files/fileIcons";
import type { TerminalSession } from "../terminal/terminalService";
import type { AbaNaTela, ContextoDaTela, PainelNaTela } from "./telasAbertas";
export type { AbaNaTela, PainelNaTela } from "./telasAbertas";
import "./naTela.css";

/** Até uma única aba continua acessível quando a pessoa muda de projeto ou abre outra tela. */
export function deveMostrarNaTela(paineis: PainelNaTela[]): boolean {
  return paineis.some((painel) => painel.abas.length > 0);
}

export function NaTela({ paineis, sessoes, naoVistas, onFocar }: {
  paineis: PainelNaTela[];
  sessoes: TerminalSession[];
  naoVistas: Set<string>;
  onFocar: (paneId: string, tabId: string, contexto?: ContextoDaTela) => void;
}) {
  if (!deveMostrarNaTela(paineis)) return null;
  const comConversa = paineis.filter((painel) => painel.abas.length > 0);
  const variosWorkspaces = new Set(comConversa.map((painel) => painel.workspaceId)).size > 1;
  const variosProjetos = new Set(comConversa.map((painel) => `${painel.workspaceId}:${painel.projectId}`)).size > 1;
  const contexto = (painel: PainelNaTela, indice: number) => [
    variosWorkspaces ? painel.workspace : undefined,
    variosProjetos ? painel.projeto : undefined,
    `Painel ${painel.posicao ?? indice + 1}`,
  ].filter(Boolean).join(" · ");
  return (
    <section className="na-tela" aria-label="Na tela">
      <p className="na-tela-titulo text-text-muted text-[10px] font-medium uppercase tracking-wider">Na tela</p>
      <div className="na-tela-lista">
      {comConversa.map((painel, i) => (
        <div key={`${painel.workspaceId ?? ""}:${painel.projectId ?? ""}:${painel.paneId}`} className="na-tela-grupo">
        {(variosWorkspaces || variosProjetos) && <p className="na-tela-contexto" title={contexto(painel, i)}>{contexto(painel, i)}</p>}
        <ul className={`na-tela-painel${painel.emFoco ? " is-em-foco" : ""}`}
          aria-label={`${contexto(painel, i)}${painel.abas.length > 1 ? ` · ${painel.abas.length} ${painel.abas.every((aba) => !aba.kind || aba.kind === "agent") ? "conversas" : "abas"} juntas` : ""}`}>
          {painel.abas.map((aba) => {
            const sessao = sessoes.find((item) => item.id === aba.sessionId);
            const status = sessao ? statusDaConversa(sessao, naoVistas.has(sessao.conversation_id || sessao.id)) : "neutro";
            return (
              <li key={aba.tabId} className={`na-tela-aba${aba.ativa ? " is-ativa" : ""}`}
                // Fork: a sessão viva já pode ter trocado de agente desde que a aba foi salva.
                style={{ "--cor-agente": corDoAgente(sessao?.provider ?? aba.provider) } as React.CSSProperties}>
                <button type="button" data-nav-item aria-current={painel.emFoco && aba.ativa ? "page" : undefined}
                  onClick={() => painel.workspaceId && painel.projectId
                    ? onFocar(painel.paneId, aba.tabId, { workspaceId: painel.workspaceId, projectId: painel.projectId })
                    : onFocar(painel.paneId, aba.tabId)}
                  title={`${contexto(painel, i)} · ${aba.ativa ? "Aberta neste painel" : "Clique para mostrar"}`}>
                  <IconeDaTela aba={aba} provider={sessao?.provider ?? aba.provider} />
                  <span className="na-tela-nome">{sessao?.name ?? aba.titulo}</span>
                  {aba.projeto && <span className="na-tela-projeto">{aba.projeto}</span>}
                  <IconeDoStatus status={status} className="h-3 w-3" />
                </button>
              </li>
            );
          })}
        </ul>
        </div>
      ))}
      </div>
    </section>
  );
}

function IconeDaTela({ aba, provider }: { aba: AbaNaTela; provider?: string | null }) {
  if ((!aba.kind || aba.kind === "agent") && provider && AGENT_ICON[provider]) {
    return <AgentIcon provider={provider} size={13} className="shrink-0" style={{ color: corDoAgente(provider) }} />;
  }
  const Icone = !aba.kind || aba.kind === "agent" || aba.kind === "conversa-historica" ? ChatIcon
    : aba.kind === "terminal" ? TerminalIcon : aba.kind === "web" ? GlobeIcon
    : aba.kind === "kanban" ? KanbanIcon : aba.kind === "orquestracao" ? BlocksIcon
    : aba.kind === "command" ? ListIcon : aba.kind === "revisao" || aba.kind === "git-graph" || aba.kind === "git-diff" ? GitBranchIcon
    : iconForPath(aba.recurso ?? aba.titulo);
  return <Icone className="h-3.5 w-3.5 shrink-0 text-text-muted" aria-hidden />;
}
