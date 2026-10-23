// Fork: "Na tela" reúne todas as abas abertas, inclusive em outros projetos e workspaces.
import { AGENT_ICON, AgentIcon } from "../../components/ui/AgentIcon";
import { BlocksIcon, ChatIcon, GitBranchIcon, GlobeIcon, KanbanIcon, ListIcon, TerminalIcon } from "../../components/ui/PixelIcon";
import { IconeDoStatus, statusDaConversa } from "../../components/ConversaStatus";
import { corDoAgente } from "../../lib/coresDosAgentes";
import { iconForPath } from "../files/fileIcons";
import type { TerminalSession } from "../terminal/terminalService";
import type { AbaNaTela, AreaDoPainel, ContextoDaTela, PainelNaTela } from "./telasAbertas";
export type { AbaNaTela, PainelNaTela } from "./telasAbertas";
import "./naTela.css";
// Fork (troca-de-contexto): o essencial de cada agente embaixo do nome e as novidades desde a sua olhada.
import { EssencialNaTela, NovasNaTela } from "../troca/EssencialNaTela";

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
  // Fork: painéis do mesmo layout ficam juntos num cartão de "tela", com o mini mapa da divisão.
  const telas: { chave: string; paineis: { painel: PainelNaTela; i: number }[] }[] = [];
  comConversa.forEach((painel, i) => {
    const chave = `${painel.workspaceId ?? ""}:${painel.projectId ?? ""}`;
    const ultima = telas[telas.length - 1];
    if (ultima?.chave === chave) ultima.paineis.push({ painel, i }); else telas.push({ chave, paineis: [{ painel, i }] });
  });
  return (
    <section className="na-tela" aria-label="Na tela">
      <p className="na-tela-titulo text-text-muted text-[10px] font-medium uppercase tracking-wider">Na tela</p>
      <div className="na-tela-lista">
      {telas.map((tela) => {
        const primeiro = tela.paineis[0].painel;
        const juntos = tela.paineis.length > 1;
        const conversas = tela.paineis.reduce((n, { painel }) => n + painel.abas.length, 0);
        return (
          <div key={tela.chave} className={`na-tela-tela${juntos ? " is-dividida" : ""}${primeiro.telaVisivel ? " is-visivel" : ""}`}
            role="group" aria-label={`Tela ${[variosWorkspaces ? primeiro.workspace : undefined, primeiro.projeto].filter(Boolean).join(" · ")}${juntos ? ` · ${tela.paineis.length} painéis` : ""}${primeiro.telaVisivel ? " · à vista agora" : ""}`}>
            {juntos && (
              <div className="na-tela-cabecalho">
                <MapaDaTela mapa={primeiro.mapaDaTela ?? []} paineis={tela.paineis.map(({ painel }) => painel)} sessoes={sessoes} />
                <span className="na-tela-cabecalho-texto">
                  <span className="na-tela-cabecalho-nome">{variosProjetos || variosWorkspaces ? primeiro.projeto : "Mesma tela"}</span>
                  <span className="na-tela-cabecalho-info">{conversas} {conversas === 1 ? "aba" : "abas"} {arranjo(primeiro.mapaDaTela ?? [])}</span>
                </span>
                {primeiro.telaVisivel && <span className="na-tela-agora" title="Esta tela está à vista agora">agora</span>}
              </div>
            )}
      {tela.paineis.map(({ painel, i }, j) => (
        <div key={`${painel.workspaceId ?? ""}:${painel.projectId ?? ""}:${painel.paneId}`}
          className={`na-tela-grupo${j === 0 ? " is-primeiro" : ""}${j === tela.paineis.length - 1 ? " is-ultimo" : ""}`}>
        {(variosWorkspaces || variosProjetos) && !juntos && <p className="na-tela-contexto" title={contexto(painel, i)}>{contexto(painel, i)}</p>}
        <ul className={`na-tela-painel${painel.emFoco ? " is-em-foco" : ""}`}
          aria-label={`${contexto(painel, i)}${painel.abas.length > 1 ? ` · ${painel.abas.length} ${painel.abas.every((aba) => !aba.kind || aba.kind === "agent") ? "conversas" : "abas"} juntas` : ""}`}>
          {painel.abas.map((aba) => {
            const sessao = sessoes.find((item) => item.id === aba.sessionId);
            const status = sessao ? statusDaConversa(sessao, naoVistas.has(sessao.conversation_id || sessao.id)) : "neutro";
            return (
              <li key={aba.tabId} className={`na-tela-aba${aba.ativa ? " is-ativa" : ""}`}
                // Fork: a sessão viva já pode ter trocado de agente desde que a aba foi salva.
                style={{ "--cor-agente": corDoAgente(sessao?.provider ?? aba.provider) } as React.CSSProperties}>
                <button type="button" data-nav-item aria-describedby={sessao ? `essencial-${aba.tabId}` : undefined} aria-current={painel.emFoco && aba.ativa ? "page" : undefined}
                  onClick={() => painel.workspaceId && painel.projectId
                    ? onFocar(painel.paneId, aba.tabId, { workspaceId: painel.workspaceId, projectId: painel.projectId })
                    : onFocar(painel.paneId, aba.tabId)}
                  title={`${contexto(painel, i)} · ${aba.ativa ? "Aberta neste painel" : "Clique para mostrar"}`}>
                  <IconeDaTela aba={aba} provider={sessao?.provider ?? aba.provider} />
                  <span className="na-tela-texto">
                    <span className="na-tela-nome">{sessao?.name ?? aba.titulo}</span>
                    {sessao && (!aba.kind || aba.kind === "agent") && <EssencialNaTela sessao={sessao} id={`essencial-${aba.tabId}`} />}
                  </span>
                  {aba.projeto && <span className="na-tela-projeto">{aba.projeto}</span>}
                  {sessao && <NovasNaTela sessao={sessao} />}
                  <IconeDoStatus status={status} className="h-3 w-3" />
                </button>
              </li>
            );
          })}
        </ul>
        {juntos && <span className="na-tela-numero" aria-hidden="true">{painel.posicao ?? i + 1}</span>}
        </div>
      ))}
          </div>
        );
      })}
      </div>
    </section>
  );
}

/** "lado a lado", "uma em cima da outra" ou "na mesma tela" (divisão mista). */
export function arranjo(mapa: AreaDoPainel[]): string {
  if (mapa.length > 1 && mapa.every((a) => a.y === 0 && a.h === 1)) return "lado a lado";
  if (mapa.length > 1 && mapa.every((a) => a.x === 0 && a.w === 1)) return "uma em cima da outra";
  return "na mesma tela";
}

/** Mini mapa da divisão da tela: cada painel com o número e a cor do agente da aba à vista. */
function MapaDaTela({ mapa, paineis, sessoes }: { mapa: AreaDoPainel[]; paineis: PainelNaTela[]; sessoes: TerminalSession[] }) {
  if (mapa.length < 2) return null;
  const W = 30, H = 20;
  return (
    <svg className="na-tela-mapa" width={W} height={H} viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
      {mapa.map((area) => {
        const painel = paineis.find((p) => p.paneId === area.paneId);
        const aba = painel?.abas.find((a) => a.ativa) ?? painel?.abas[0];
        const provider = aba ? (sessoes.find((x) => x.id === aba.sessionId)?.provider ?? aba.provider) : undefined;
        return (
          <rect key={area.paneId} x={area.x * W + 0.75} y={area.y * H + 0.75} width={Math.max(1, area.w * W - 1.5)} height={Math.max(1, area.h * H - 1.5)} rx={1.5}
            className={painel?.emFoco ? "is-em-foco" : painel ? "" : "is-vazio"}
            style={{ fill: painel ? `color-mix(in srgb, ${corDoAgente(provider)} 38%, transparent)` : undefined }} />
        );
      })}
    </svg>
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
