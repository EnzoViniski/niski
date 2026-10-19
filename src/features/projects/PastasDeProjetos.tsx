import { useEffect, useState, type KeyboardEvent } from "react";
import { ChevronDownIcon, CloseIcon, FolderIcon, GitBranchIcon, ReloadIcon } from "../../components/ui/PixelIcon";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { IconeDoStatus, rotuloDoStatus, statusDaConversa } from "../../components/ConversaStatus";
import { conversationKey } from "../terminal/useAttention";
// Fork: atalho para o fluxo na linha da conversa orquestradora.
import { IconeDaOrquestradora } from "../orquestracao/IconeDaOrquestradora";
import { corDoAgente, nomeDoAgente } from "../../lib/coresDosAgentes";
import type { Project } from "../../types/workspace";
import type { TerminalSession } from "../terminal/terminalService";
import { colocar, mover, ordenar, useOrdemDaLateral } from "./ordemDaLateral";
import { iniciarArraste } from "./arrastarLinha";
import { TAB_DRAG_EVENT, type TabDragState, type TabDropTarget } from "../workspace/tabPointerDrag";

/** A linha sendo arrastada: em qual lista (`projetos` ou o id do projeto), qual, e a posição prevista
 *  entre as outras (`null` = fora da lista, por exemplo sobre o layout). */
type Soltura = { lista: string; id: string; posicao: number | null } | null;

/** Durante o arraste a lista já aparece na ordem nova — a prévia de como vai ficar. */
function emPrevia(ids: string[], soltura: Soltura, lista: string): string[] {
  return soltura?.lista === lista && soltura.posicao !== null ? colocar(ids, soltura.id, soltura.posicao) : ids;
}

/** Os painéis do layout desenham a zona de soltar ouvindo o mesmo aviso do arraste de abas; a origem
 *  "lateral" não é painel nenhum, então nunca conta como "soltar no próprio painel". */
function avisarLayout(sessionId: string, alvo: TabDropTarget | null) {
  const estado: TabDragState = alvo ? { tab: { paneId: "lateral", tabId: sessionId }, target: alvo } : null;
  window.dispatchEvent(new CustomEvent(TAB_DRAG_EVENT, { detail: estado }));
}

/** ⌥↑/⌥↓ move a linha em foco: o mesmo que arrastar, pelo teclado. */
function setaComOption(evento: KeyboardEvent): -1 | 1 | 0 {
  if (!evento.altKey) return 0;
  return evento.key === "ArrowUp" ? -1 : evento.key === "ArrowDown" ? 1 : 0;
}

/** A linha arrastada fica no lugar previsto como um espaço vazio; quem segue o mouse é a cópia. */
function classeDeSoltura(soltura: Soltura, lista: string, id: string): string {
  return soltura?.lista === lista && soltura.id === id ? " is-arrastando" : "";
}

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
  /** Fork: conversa arrastada para um painel do layout (só do projeto aberto, que é o que aparece). */
  onSoltarNoLayout?: (projectId: string, session: TerminalSession, alvo: TabDropTarget) => void;
}

/**
 * Projetos como pastas: cada uma abre e mostra as conversas dela, e várias podem ficar abertas ao
 * mesmo tempo. A cor da barra e do ícone de cada conversa diz qual agente roda nela (Claude, Codex,
 * Antigravity…); o ícone à direita, o estado (trabalhando, pedindo aprovação…).
 */
export function PastasDeProjetos({
  projects, activeProjectId, activeSessionId, agentSessions, naoVistas,
  onSelectProject, onCloseProject, onAbrirConversa, onBifurcar, onReiniciar, onFechar, onSoltarNoLayout,
}: Props) {
  const { ordem, ordenarProjetos, ordenarConversas } = useOrdemDaLateral();
  const [soltura, setSoltura] = useState<Soltura>(null);
  // Fork: apagar conversa pede um segundo clique (o "×" vira "Apagar?" por alguns segundos).
  const [apagando, setApagando] = useState<string | null>(null);
  useEffect(() => {
    if (!apagando) return;
    const timer = window.setTimeout(() => setApagando(null), 3_000);
    return () => window.clearTimeout(timer);
  }, [apagando]);
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

  // Fork: projetos e conversas na ordem que a pessoa arrastou (`ordemDaLateral`).
  const projetosEmOrdem = ordenar(projects, (p) => p.id, ordem.projetos);
  const idsDosProjetos = projetosEmOrdem.map((p) => p.id);
  const projetosNaTela = emPrevia(idsDosProjetos, soltura, "projetos").map((id) => projetosEmOrdem.find((p) => p.id === id)!);

  return (
    <ul className="pastas space-y-0.5" aria-label="Projetos">
      {projetosNaTela.map((project) => {
        const indice = idsDosProjetos.indexOf(project.id);
        const aberta = abertas.has(project.id);
        const ativa = project.id === activeProjectId;
        const conversas = ordenar(agentSessions.filter((session) => session.project_id === project.id),
          conversationKey, ordem.conversas[project.id] ?? []);
        const idsDasConversas = conversas.map(conversationKey);
        const conversasNaTela = emPrevia(idsDasConversas, soltura, project.id).map((id) => conversas.find((c) => conversationKey(c) === id)!);
        // Fork: conversa de qualquer projeto pode cair no layout que está na tela; a aba mostra de
        // qual projeto ela é (`workspace/projetosDasAbas.tsx`).
        const soltaNoLayout = onSoltarNoLayout;
        return (
          <li key={project.id} data-reordenar className={classeDeSoltura(soltura, "projetos", project.id).trim() || undefined}>
            <div className={`pasta group${ativa ? " is-ativa" : ""}`}
              onPointerDown={(evento) => iniciarArraste(evento, evento.currentTarget.closest("ul"), {
                aoMudar: (posicao) => setSoltura({ lista: "projetos", id: project.id, posicao }),
                aoTerminar: () => setSoltura(null),
                aoSoltar: (posicao) => ordenarProjetos(colocar(idsDosProjetos, project.id, posicao)),
              })}>
              <button type="button" data-nav-item aria-expanded={aberta} aria-current={ativa ? "page" : undefined}
                title={`${project.path}\nArraste (ou ⌥↑/⌥↓) para mudar a ordem`}
                onKeyDown={(evento) => {
                  const passo = setaComOption(evento);
                  if (!passo) return;
                  evento.preventDefault();
                  ordenarProjetos(mover(idsDosProjetos, project.id, passo < 0 ? indice - 1 : indice + 2));
                }}
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
              <button type="button" data-nao-arrasta aria-label={`Fechar projeto ${project.name}`} title="Fechar projeto"
                onClick={() => onCloseProject(project.id)} className="pasta-acao hover:text-danger">
                <CloseIcon aria-hidden className="h-3 w-3" />
              </button>
            </div>

            {aberta && (
              <ul className="pasta-conversas" aria-label={`Conversas de ${project.name}`}>
                {conversas.length === 0 && <li className="px-2 py-1 text-[11px] text-text-muted">Nenhuma conversa aberta.</li>}
                {conversasNaTela.map((session) => {
                  const posicao = conversas.indexOf(session);
                  const status = statusDaConversa(session, naoVistas.has(conversationKey(session)));
                  const emFoco = session.id === activeSessionId;
                  const chave = conversationKey(session);
                  // Conversa só muda de lugar dentro do próprio projeto: o arraste fica preso a esta lista.
                  return (
                    <li key={session.id} data-reordenar
                      className={`conversa-linha group${emFoco ? " is-ativa" : ""}${classeDeSoltura(soltura, project.id, chave)}`}
                      style={{ "--cor-agente": corDoAgente(session.provider) } as React.CSSProperties}
                      onPointerDown={(evento) => iniciarArraste(evento, evento.currentTarget.closest("ul"), {
                        aoMudar: (posicao) => setSoltura({ lista: project.id, id: chave, posicao }),
                        aoTerminar: () => setSoltura(null),
                        aoSoltar: (posicao) => ordenarConversas(project.id, colocar(idsDasConversas, chave, posicao)),
                        layout: soltaNoLayout ? {
                          aoMudar: (alvo) => avisarLayout(session.id, alvo),
                          aoSoltar: (alvo) => onSoltarNoLayout(project.id, session, alvo),
                        } : undefined,
                      })}>
                      <button type="button" data-nav-item onClick={() => onAbrirConversa(project.id, session)}
                        title={`${nomeDoAgente(session.provider)} · ${rotuloDoStatus(status)}\nArraste (ou ⌥↑/⌥↓) para mudar a ordem${soltaNoLayout ? "; solte num painel para abrir lá" : ""}`}
                        onKeyDown={(evento) => {
                          const passo = setaComOption(evento);
                          if (!passo) return;
                          evento.preventDefault();
                          ordenarConversas(project.id, mover(idsDasConversas, chave, passo < 0 ? posicao - 1 : posicao + 2));
                        }}
                        className="conversa-botao">
                        <AgentIcon provider={session.provider} size={13} className="shrink-0" />
                        <span className="min-w-0 flex-1 truncate">{session.name}</span>
                        <IconeDoStatus status={status} className="h-3 w-3" />
                      </button>
                      <IconeDaOrquestradora conversa={chave} titulo={session.name} />
                      <span className="conversa-acoes" data-nao-arrasta>
                        <button type="button" aria-label={`Bifurcar conversa ${session.name}`} title="Bifurcar: conversa nova com todo o histórico desta"
                          onClick={() => onBifurcar(session.id)}><GitBranchIcon aria-hidden className="h-3 w-3" /></button>
                        <button type="button" aria-label={`Reiniciar agente ${session.name}`} title="Reiniciar sessão"
                          onClick={() => onReiniciar(session.id)}><ReloadIcon aria-hidden className="h-3 w-3" /></button>
                        <button type="button" aria-label={apagando === session.id ? `Confirmar: apagar conversa ${session.name}` : `Apagar conversa ${session.name}`}
                          title={apagando === session.id ? "Clique de novo para apagar" : "Apagar conversa (encerra o agente e tira da lista)"}
                          onClick={() => {
                            if (apagando !== session.id) { setApagando(session.id); return; }
                            setApagando(null);
                            onFechar(session.id);
                          }}
                          className={apagando === session.id ? "conversa-apagar-confirmar" : "hover:text-danger"}>
                          {apagando === session.id ? "Apagar?" : <CloseIcon aria-hidden className="h-3 w-3" />}</button>
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
