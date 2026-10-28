import { useSession } from "../terminal/sessionProviders";
import { papelDaConversa, useResumoDaOrquestracao } from "./orquestracaoService";
import "./orquestracao.css";

/** Na aba do layout, diz se a conversa é orquestradora ("ORQ") ou subagente ("SUB"). */
export function MarcaDaAba({ sessionId }: { sessionId: string | undefined }) {
  const conversa = useSession(sessionId)?.conversation_id;
  const papel = papelDaConversa(useResumoDaOrquestracao(), conversa);
  if (papel.orquestradora || papel.teveFluxo) {
    return <span className="orq-marca-aba is-orq" title={`Orquestradora · ${papel.agentes} subagente(s)`}>ORQ</span>;
  }
  if (papel.dona) return <span className="orq-marca-aba is-sub" title="Subagente de uma orquestradora">SUB</span>;
  return null;
}
