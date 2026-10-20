import { useState } from "react";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { corDoAgente, nomeDoAgente } from "../../lib/coresDosAgentes";
import { haQuanto } from "../../lib/tempo";
import type { HistoryEntry } from "../history/historyService";
import type { ConversaAnterior } from "./historicoDaPasta";

/** Quantas aparecem de cara e quantas cada "Mostrar mais" acrescenta. */
export const PRIMEIRAS = 5;
const MAIS = 10;

interface Props {
  nomeDoProjeto: string;
  anteriores: ConversaAnterior[];
  onRetomar: (entry: HistoryEntry) => Promise<void> | void;
  onVerTodas?: () => void;
}

function mensagemDoErro(erro: unknown) {
  return erro instanceof Error ? erro.message : String(erro);
}

/** Fork (histórico na lateral): conversas que já terminaram, embaixo das abertas. Clique retoma. */
export function ConversasAnteriores({ nomeDoProjeto, anteriores, onRetomar, onVerTodas }: Props) {
  const [quantas, setQuantas] = useState(PRIMEIRAS);
  const [abrindo, setAbrindo] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  if (anteriores.length === 0) return null;

  async function retomar(entry: HistoryEntry) {
    if (abrindo) return;
    setAbrindo(entry.session_id);
    setErro(null);
    try {
      await onRetomar(entry);
    } catch (motivo) {
      setErro(mensagemDoErro(motivo));
    } finally {
      setAbrindo(null);
    }
  }

  return (
    <>
      <li className="conversas-anteriores-rotulo" aria-hidden>Anteriores</li>
      {anteriores.slice(0, quantas).map(({ entry, titulo }) => (
        <li key={`${entry.provider}:${entry.profile_id}:${entry.session_id}`} className="conversa-linha is-anterior"
          style={{ "--cor-agente": corDoAgente(entry.provider) } as React.CSSProperties}>
          <button type="button" data-nav-item className="conversa-botao" disabled={abrindo !== null}
            aria-label={`Retomar conversa anterior ${titulo} de ${nomeDoProjeto}`}
            title={`${nomeDoAgente(entry.provider)} · ${entry.profile_name}\nClique para retomar esta conversa`}
            onClick={() => void retomar(entry)}>
            <AgentIcon provider={entry.provider} size={13} className="shrink-0" />
            <span className="min-w-0 flex-1 truncate">{abrindo === entry.session_id ? "Abrindo…" : titulo}</span>
            <span className="conversa-quando">{haQuanto(entry.updated_at_ms)}</span>
          </button>
        </li>
      ))}
      {erro && <li role="alert" className="px-2 py-1 text-[11px] text-danger">{erro}</li>}
      <li className="conversas-anteriores-mais">
        {anteriores.length > quantas && (
          <button type="button" data-nav-item onClick={() => setQuantas((atual) => atual + MAIS)}>
            Mostrar mais ({anteriores.length - quantas})
          </button>
        )}
        {onVerTodas && <button type="button" data-nav-item onClick={onVerTodas}>Histórico</button>}
      </li>
    </>
  );
}
