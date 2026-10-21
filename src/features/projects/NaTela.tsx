// Fork: "Na tela" — o que está aberto no painel da direita, agrupado por painel (pedido do Enzo,
// 21/10/2026). Conversas que dividem o mesmo painel (abas juntas) aparecem uma em cima da outra no
// mesmo cartão, mesmo sendo de pastas diferentes (aí a pasta aparece ao lado). Clicar foca a aba.
import { AgentIcon } from "../../components/ui/AgentIcon";
import { IconeDoStatus, statusDaConversa } from "../../components/ConversaStatus";
import { corDoAgente } from "../../lib/coresDosAgentes";
import type { TerminalSession } from "../terminal/terminalService";
import "./naTela.css";

export interface AbaNaTela { tabId: string; titulo: string; sessionId: string; provider?: string; projeto?: string; ativa: boolean }
export interface PainelNaTela { paneId: string; emFoco: boolean; abas: AbaNaTela[] }

/** Só aparece quando ajuda: dois painéis ou mais, ou um painel com duas conversas ou mais. */
export function deveMostrarNaTela(paineis: PainelNaTela[]): boolean {
  const comConversa = paineis.filter((painel) => painel.abas.length > 0);
  return comConversa.length > 1 || comConversa.some((painel) => painel.abas.length > 1);
}

export function NaTela({ paineis, sessoes, naoVistas, onFocar }: {
  paineis: PainelNaTela[];
  sessoes: TerminalSession[];
  naoVistas: Set<string>;
  onFocar: (paneId: string, tabId: string) => void;
}) {
  if (!deveMostrarNaTela(paineis)) return null;
  const comConversa = paineis.filter((painel) => painel.abas.length > 0);
  return (
    <section className="na-tela" aria-label="Na tela">
      <p className="na-tela-titulo text-text-muted text-[10px] font-medium uppercase tracking-wider">Na tela</p>
      {comConversa.map((painel, i) => (
        <ul key={painel.paneId} className={`na-tela-painel${painel.emFoco ? " is-em-foco" : ""}`}
          aria-label={`Painel ${i + 1}${painel.abas.length > 1 ? ` · ${painel.abas.length} conversas juntas` : ""}`}>
          {painel.abas.map((aba) => {
            const sessao = sessoes.find((item) => item.id === aba.sessionId);
            const status = sessao ? statusDaConversa(sessao, naoVistas.has(sessao.conversation_id || sessao.id)) : "neutro";
            return (
              <li key={aba.tabId} className={`na-tela-aba${aba.ativa ? " is-ativa" : ""}`}
                style={{ "--cor-agente": corDoAgente(aba.provider ?? sessao?.provider) } as React.CSSProperties}>
                <button type="button" onClick={() => onFocar(painel.paneId, aba.tabId)}
                  title={aba.ativa ? "Aberta neste painel" : "No mesmo painel, em outra aba — clique para mostrar"}>
                  <AgentIcon provider={aba.provider ?? sessao?.provider} size={13} className="shrink-0" />
                  <span className="na-tela-nome">{sessao?.name ?? aba.titulo}</span>
                  {aba.projeto && <span className="na-tela-projeto">{aba.projeto}</span>}
                  <IconeDoStatus status={status} className="h-3 w-3" />
                </button>
              </li>
            );
          })}
        </ul>
      ))}
    </section>
  );
}
