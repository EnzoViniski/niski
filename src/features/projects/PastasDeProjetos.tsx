import { useEffect, useState, type KeyboardEvent } from "react";
import { ChevronDownIcon, CloseIcon, FolderIcon, GitBranchIcon, ReloadIcon } from "../../components/ui/PixelIcon";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { IconeDoStatus, rotuloDoStatus, statusDaConversa } from "../../components/ConversaStatus";
import { conversationKey } from "../terminal/useAttention";
// Fork: atalho para o fluxo na linha da conversa orquestradora.
import { IconeDaOrquestradora } from "../orquestracao/IconeDaOrquestradora";
import { useResumoDaOrquestracao } from "../orquestracao/orquestracaoService";
import { corDoAgente, nomeDoAgente } from "../../lib/coresDosAgentes";
import type { Project } from "../../types/workspace";
import type { TerminalSession } from "../terminal/terminalService";
import { colocar, mover, ordenar, useOrdemDaLateral } from "./ordemDaLateral";
import { iniciarArraste } from "./arrastarLinha";
import { TAB_DRAG_EVENT, type TabDragState, type TabDropTarget } from "../workspace/tabPointerDrag";
// Fork (histórico na lateral): conversas anteriores de cada pasta, como nos apps do Claude/Codex.
import { anterioresDaPasta, relerConversasAnteriores, useConversasAnteriores } from "./historicoDaPasta";
import { ConversasAnteriores } from "./ConversasAnteriores";
import type { HistoryEntry } from "../history/historyService";

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
/** Fork: orquestradoras com os subagentes recolhidos na lateral. */
const CHAVE_RECOLHIDOS = "medza-orquestradoras-recolhidas";

function lerAbertas(): Set<string> | null {
  return lerConjunto(CHAVE_ABERTAS);
}

function lerConjunto(chave: string): Set<string> | null {
  try {
    const salvo = JSON.parse(localStorage.getItem(chave) ?? "null");
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
  /** Fork: retoma uma conversa anterior (do transcript da CLI) num terminal do projeto. */
  onRetomarAnterior?: (entry: HistoryEntry) => Promise<void> | void;
  /** Fork: abre a tela Histórico (todas as conversas, com busca). */
  onVerHistorico?: () => void;
}

/**
 * Projetos como pastas: cada uma abre e mostra as conversas dela, e várias podem ficar abertas ao
 * mesmo tempo. A cor da barra e do ícone de cada conversa diz qual agente roda nela (Claude, Codex,
 * Antigravity…); o ícone à direita, o estado (trabalhando, pedindo aprovação…).
 */
export function PastasDeProjetos({
  projects, activeProjectId, activeSessionId, agentSessions, naoVistas,
  onSelectProject, onCloseProject, onAbrirConversa, onBifurcar, onReiniciar, onFechar, onSoltarNoLayout,
  onRetomarAnterior, onVerHistorico,
}: Props) {
  const historico = useConversasAnteriores();
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

  // Fork: quem é subagente de quem (orquestrador), para aninhar na lateral.
  const resumo = useResumoDaOrquestracao();
  const donoDe = new Map<string, string>();
  for (const o of resumo.orquestradores) for (const sub of o.subagentes) donoDe.set(sub, o.orquestrador);
  const filhosDe = (chave: string) => agentSessions.filter((session) => donoDe.get(conversationKey(session)) === chave);
  const [recolhidos, setRecolhidos] = useState<Set<string>>(() => lerConjunto(CHAVE_RECOLHIDOS) ?? new Set());
  const alternarRecolhido = (chave: string) => setRecolhidos((atual) => {
    const novo = new Set(atual);
    if (novo.has(chave)) novo.delete(chave); else novo.add(chave);
    try { localStorage.setItem(CHAVE_RECOLHIDOS, JSON.stringify([...novo])); } catch { /* vale só agora */ }
    return novo;
  });

  if (projects.length === 0) {
    return <p data-nav-item className="px-3 py-1.5 text-xs text-text-muted">Nenhum projeto ainda.</p>;
  }

  // Fork: projetos e conversas na ordem que a pessoa arrastou (`ordemDaLateral`).
  const projetosEmOrdem = ordenar(projects, (p) => p.id, ordem.projetos);
  const idsDosProjetos = projetosEmOrdem.map((p) => p.id);
  const projetosNaTela = emPrevia(idsDosProjetos, soltura, "projetos").map((id) => projetosEmOrdem.find((p) => p.id === id)!);

  /** Uma conversa da lateral. `sub`: subagente embaixo da orquestradora (sem reordenar nem arrastar). */
  function linha(project: Project, session: TerminalSession, idsDasConversas: string[], conversas: TerminalSession[],
    soltaNoLayout: Props["onSoltarNoLayout"], extra: { sub?: boolean; ultimo?: boolean; filhos?: number; recolhido?: boolean }) {
    const sub = Boolean(extra.sub);
    const posicao = conversas.indexOf(session);
    const status = statusDaConversa(session, naoVistas.has(conversationKey(session)));
    const emFoco = session.id === activeSessionId;
    const chave = conversationKey(session);
    return (
                    <li key={session.id} data-reordenar={sub ? undefined : ""}
                      className={`conversa-linha group${emFoco ? " is-ativa" : ""}${sub ? ` is-subagente${extra.ultimo ? " is-ultimo" : ""}` : ""}${classeDeSoltura(soltura, project.id, chave)}`}
                      style={{ "--cor-agente": corDoAgente(session.provider) } as React.CSSProperties}
                      onPointerDown={sub ? undefined : (evento) => iniciarArraste(evento, evento.currentTarget.closest("ul"), {
                        aoMudar: (posicao) => setSoltura({ lista: project.id, id: chave, posicao }),
                        aoTerminar: () => setSoltura(null),
                        aoSoltar: (posicao) => ordenarConversas(project.id, colocar(idsDasConversas, chave, posicao)),
                        layout: soltaNoLayout ? {
                          aoMudar: (alvo) => avisarLayout(session.id, alvo),
                          aoSoltar: (alvo) => soltaNoLayout(project.id, session, alvo),
                        } : undefined,
                      })}>
                      {Boolean(extra.filhos) && (
                        <button type="button" data-nao-arrasta className="orq-recolher" aria-expanded={!extra.recolhido}
                          aria-label={`${extra.recolhido ? "Mostrar" : "Esconder"} os subagentes de ${session.name}`} onClick={() => alternarRecolhido(chave)}>
                          <ChevronDownIcon aria-hidden className={`h-3 w-3 transition-transform ${extra.recolhido ? "-rotate-90" : ""}`} />
                        </button>
                      )}
                      <button type="button" data-nav-item onClick={() => onAbrirConversa(project.id, session)}
                        title={sub ? `Subagente · ${nomeDoAgente(session.provider)} · ${rotuloDoStatus(status)}`
                          : `${nomeDoAgente(session.provider)} · ${rotuloDoStatus(status)}\nArraste (ou ⌥↑/⌥↓) para mudar a ordem${soltaNoLayout ? "; solte num painel para abrir lá" : ""}`}
                        onKeyDown={(evento) => {
                          if (sub) return;
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
  }

  return (
    <ul className="pastas space-y-0.5" aria-label="Projetos">
      {projetosNaTela.map((project) => {
        const indice = idsDosProjetos.indexOf(project.id);
        const aberta = abertas.has(project.id);
        const ativa = project.id === activeProjectId;
        const todas = ordenar(agentSessions.filter((session) => session.project_id === project.id),
          conversationKey, ordem.conversas[project.id] ?? []);
        // Fork: subagente cuja orquestradora está nesta pasta não entra solto na lista (vai embaixo dela).
        const chavesDaPasta = new Set(todas.map(conversationKey));
        const conversas = todas.filter((session) => !chavesDaPasta.has(donoDe.get(conversationKey(session)) ?? ""));
        const idsDasConversas = conversas.map(conversationKey);
        const conversasNaTela = emPrevia(idsDasConversas, soltura, project.id).map((id) => conversas.find((c) => conversationKey(c) === id)!);
        // Fork: conversa de qualquer projeto pode cair no layout que está na tela; a aba mostra de
        // qual projeto ela é (`workspace/projetosDasAbas.tsx`).
        const soltaNoLayout = onSoltarNoLayout;
        const anteriores = onRetomarAnterior && aberta ? anterioresDaPasta(project.path, historico, agentSessions) : [];
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
                {todas.length > 0 && <span className="pasta-contagem">{todas.length}</span>}
              </button>
              <button type="button" data-nao-arrasta aria-label={`Fechar projeto ${project.name}`} title="Fechar projeto"
                onClick={() => onCloseProject(project.id)} className="pasta-acao hover:text-danger">
                <CloseIcon aria-hidden className="h-3 w-3" />
              </button>
            </div>

            {aberta && (
              <ul className="pasta-conversas" aria-label={`Conversas de ${project.name}`}>
                {todas.length === 0 && anteriores.length === 0 && <li className="px-2 py-1 text-[11px] text-text-muted">Nenhuma conversa ainda.</li>}
                {conversasNaTela.flatMap((session) => {
                  // Fork: subagentes logo abaixo da orquestradora, como subpasta (recolhível).
                  const filhos = filhosDe(conversationKey(session));
                  const recolhido = recolhidos.has(conversationKey(session));
                  return [linha(project, session, idsDasConversas, conversas, soltaNoLayout, { filhos: filhos.length, recolhido }),
                    ...(recolhido ? [] : filhos.map((filho, i) => linha(project, filho, idsDasConversas, conversas, undefined, { sub: true, ultimo: i === filhos.length - 1 })))];
                })}
                {onRetomarAnterior && (
                  <ConversasAnteriores nomeDoProjeto={project.name} anteriores={anteriores} onVerTodas={onVerHistorico}
                    onRetomar={async (entry) => {
                      await onRetomarAnterior(entry);
                      void relerConversasAnteriores();
                    }} />
                )}
              </ul>
            )}
          </li>
        );
      })}
    </ul>
  );
}
