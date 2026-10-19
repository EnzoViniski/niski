import { BlocksIcon } from "../../components/ui/PixelIcon";
import { pedirAbrirFluxo } from "./abrirFluxo";
import { papelDaConversa, useResumoDaOrquestracao } from "./orquestracaoService";
import "./orquestracao.css";

/** Fork: na linha da conversa na lateral, o atalho para o fluxo de uma orquestradora (só aparece nela). */
export function IconeDaOrquestradora({ conversa, titulo }: { conversa: string; titulo: string }) {
  const papel = papelDaConversa(useResumoDaOrquestracao(), conversa);
  if (!papel.orquestradora && !papel.teveFluxo) return null;
  return (
    <button type="button" data-nao-arrasta className="orq-lateral" aria-label={`Ver fluxo de ${titulo}`}
      title={`Orquestradora · ${papel.agentes} subagente(s). Abrir o fluxo`}
      onClick={() => pedirAbrirFluxo(conversa, `Fluxo · ${titulo}`.slice(0, 60))}>
      <BlocksIcon aria-hidden className="h-3 w-3" />{papel.agentes > 0 && <span>{papel.agentes}</span>}
    </button>
  );
}
