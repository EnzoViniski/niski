// Fork (Onda 2.0): o subtítulo da conversa no topo do celular, antes inline no `MobileApp`, sem mudança.
import { AgentIcon } from "../../components/ui/AgentIcon";
import { nomeDaPlataforma } from "../../lib/plataformasDeAgente";
import { rotuloDoEstado, tomDoEstado } from "../../lib/estadosDaSessao";
import type { Conversation } from "../api";
// Fork (Onda 2, item 3): contexto e custo estimado vindos da statusline.
import { ChipDeContexto } from "./ChipDeContexto";
import { ChipDeTokens } from "./TokensDaSessao"; // Total de tokens da conversa.

/** Ponto de estado, agente e "Claude · Trabalhando · projeto" abaixo do título. */
export function CabecalhoDaConversa({ conversa, projeto }: { conversa: Conversation; projeto?: { name: string } | null }) {
  const aprovar = Boolean(conversa.capabilities?.approve);
  return <>
    <span className={`m-dot ${tomDoEstado(conversa.state, aprovar)}`} aria-hidden />
    {conversa.provider && <AgentIcon provider={conversa.provider} size={12} className="shrink-0" />}
    <span className="truncate">
      {[nomeDaPlataforma(conversa.provider) ?? conversa.provider, rotuloDoEstado(conversa.state) ?? "Sem sessão ativa", projeto?.name]
        .filter(Boolean).join(" · ")}
    </span>
    <ChipDeContexto ganchos={conversa.ganchos} />
    <ChipDeTokens tokens={conversa.tokens_da_sessao} />
    {conversa.codex_app_server?.contexto_pct != null && <span className="ml-2 shrink-0 text-[11px] text-text-muted">Contexto {Math.round(conversa.codex_app_server.contexto_pct)}%</span> /* 3A. */}
  </>;
}
