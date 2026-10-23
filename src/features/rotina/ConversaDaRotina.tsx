// Fork (Rotina): a conversa do agente da rotina ao lado da tela — a mesma conversa das abas de agente
// (aprovações, troca de modelo/conta, terminal por baixo). A sessão é aberta pelo engine
// (`POST /rotina/conversa`), que escolhe plataforma/conta/modelo dos Ajustes da Rotina.
import { useMemo } from "react";
import { AbaDeAgente } from "../conversa/AbaDeAgente";
import { TerminalPane } from "../terminal/TerminalPane";
import { useSession } from "../terminal/sessionProviders";
import type { WorkspaceTab } from "../../types/workspace";
import type { AgentView } from "../../types/settings";

/** Mesmo valor de `PROJETO_DA_ROTINA` no engine (`mobile/orquestracao_rotina.rs`). */
export const PROJETO_DA_ROTINA = "rotina";
export const ehConversaDaRotina = (projectId: string | null | undefined) => projectId === PROJETO_DA_ROTINA;

export function ConversaDaRotina({ sessao, agentView, abrindo, erro, onFechar }: {
  sessao: string | null; agentView: AgentView; abrindo: boolean; erro: string | null; onFechar: () => void;
}) {
  const viva = useSession(sessao ?? undefined);
  const tab = useMemo<WorkspaceTab>(() => ({
    id: `rotina-${sessao ?? "nova"}`, kind: "agent", title: "Rotina",
    ...(sessao ? { resourceId: sessao } : {}), ...(viva?.provider ? { provider: viva.provider } : {}),
  }), [sessao, viva?.provider]);
  return (
    <aside className="rotina-conversa" aria-label="Conversa com a rotina">
      <header className="rotina-conversa-topo">
        <strong className="flex-1">Agente da rotina</strong>
        <button type="button" className="rotina-mini" onClick={onFechar} aria-label="Fechar a conversa (ela continua no Histórico)">×</button>
      </header>
      <div className="rotina-conversa-corpo">
        {sessao ? (
          <AbaDeAgente agentView={agentView} sessionId={sessao}>
            <TerminalPane key={sessao} projectId={PROJETO_DA_ROTINA} projectPath={viva?.cwd ?? ""} paneId="rotina" tab={tab} onSessionCreated={() => undefined} />
          </AbaDeAgente>
        ) : (
          <p className="rotina-vazio" role="status">{erro ?? (abrindo ? "Abrindo a conversa da rotina…" : "Sem conversa aberta.")}</p>
        )}
      </div>
    </aside>
  );
}
