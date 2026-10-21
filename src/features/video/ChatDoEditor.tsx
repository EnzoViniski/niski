// Fork (Editor de vídeo): o chat ao lado do editor é a conversa normal do app — a mesma das abas de
// agente (ferramentas, aprovações, diffs, modelo/esforço/conta, terminal por baixo) — rodando na
// pasta do projeto. Antes era um chat próprio, mais pobre, que travava o editor enquanto o agente
// trabalhava.
//
// - A sessão fica guardada no `projeto.json` (`chat`), e o mesmo vídeo reabre a mesma conversa (o
//   `TerminalPane` religa com `--resume` quando o engine só a conhece do histórico).
// - O projeto do engine é `video:<id>` (`ehConversaDoVideo`): a recuperação do workspace não
//   oferece adotar essas conversas como projeto solto.
// - "Nova conversa" volta para o lançador (escolher agente, conta e modelo).
import { useMemo, useState } from "react";
import { AbaDeAgente } from "../conversa/AbaDeAgente";
import { TerminalPane } from "../terminal/TerminalPane";
import { useSession } from "../terminal/sessionProviders";
import type { WorkspaceTab } from "../../types/workspace";
import type { AgentView } from "../../types/settings";
import { idDoProjetoDoVideo } from "./conversaDoVideo";

export interface ChatDoProjeto { sessao: string; provider?: string }

export function ChatDoEditor({ projeto, pasta, nome, chat, agentView, onChat, aviso }: {
  projeto: string;
  /** Pasta do projeto no disco (cwd do agente). */
  pasta: string;
  nome: string;
  chat: ChatDoProjeto | null;
  agentView: AgentView;
  onChat: (chat: ChatDoProjeto | null) => void;
  /** Faixa por cima da conversa (ex.: outra pessoa está com o agente, numa sessão a dois). */
  aviso?: string | null;
}) {
  // Muda a cada "Nova conversa": o TerminalPane volta do zero (lançador) em vez de religar a antiga.
  const [rodada, setRodada] = useState(0);
  const sessao = useSession(chat?.sessao);
  const tab = useMemo<WorkspaceTab>(() => ({
    id: `video-${projeto}-${rodada}`, kind: "agent", title: `Vídeo · ${nome}`,
    ...(chat?.sessao ? { resourceId: chat.sessao } : {}), ...(chat?.provider ? { provider: chat.provider } : {}),
  }), [chat?.provider, chat?.sessao, nome, projeto, rodada]);
  const trabalhando = sessao?.state === "working";

  return (
    <div className="video-chat-normal">
      <div className="video-chat-normal-topo">
        <span className="video-chat-normal-titulo">Conversa do vídeo</span>
        <span className="flex-1" />
        {chat && (
          <button type="button" className="video-mini-botao video-chat-nova" disabled={trabalhando}
            title={trabalhando ? "Espere o agente terminar" : "Começar outra conversa (a atual fica no Histórico)"}
            onClick={() => { onChat(null); setRodada((r) => r + 1); }}>
            Nova conversa
          </button>
        )}
      </div>
      {aviso && <p className="video-trava" role="status">{aviso}</p>}
      <div className="video-chat-normal-corpo">
        <AbaDeAgente agentView={agentView} sessionId={chat?.sessao ?? null}>
          <TerminalPane key={rodada} projectId={idDoProjetoDoVideo(projeto)} projectPath={pasta} paneId={`video-${projeto}`} tab={tab}
            onSessionCreated={(sessionId, _titulo, provider) => onChat({ sessao: sessionId, ...(provider ? { provider } : {}) })} />
        </AbaDeAgente>
      </div>
    </div>
  );
}
