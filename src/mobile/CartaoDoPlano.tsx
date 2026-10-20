import { useState, type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import { NotesIcon } from "../components/ui/PixelIcon";
import { AgentIcon } from "../components/ui/AgentIcon";
import { BlocoDeCodigo, horarioCurto, ImagemDoMarkdown } from "./Chat";
import { CodigoOuCaminho } from "./CaminhoClicavel";
import "./conversaRica.css";

/**
 * Fork: o plano que o Claude apresenta ao sair do plan mode (`ExitPlanMode`), num cartão destacado
 * em vez de uma linha de ação. Enquanto a CLI espera a aprovação, a pergunta dela (o mesmo menu que
 * o chat já responde pelo número, `CartaoDePergunta`) vem dentro do cartão: aprovar, aprovar
 * aceitando edições ou continuar planejando.
 */

/** Plano mais alto que isto nasce recolhido, com "Ver plano inteiro". */
const LINHAS_DO_RESUMO = 18;

export function CartaoDoPlano({ plano, provider, horario, pergunta }: {
  plano: string;
  provider?: string | null;
  horario?: string | null;
  /** A pergunta de aprovação da CLI, quando ela está esperando a resposta. */
  pergunta?: ReactNode;
}) {
  const longo = plano.split("\n").length > LINHAS_DO_RESUMO || plano.length > 1800;
  const [inteiro, setInteiro] = useState(false);
  const hora = horarioCurto(horario);
  return (
    <article className={`plano-cartao${pergunta ? " is-pendente" : ""}`} aria-label="Plano do agente">
      <header className="plano-topo">
        <span className="plano-icone" aria-hidden><NotesIcon className="h-3.5 w-3.5" /></span>
        <span className="plano-titulo">Plano</span>
        {provider && <AgentIcon provider={provider} size={12} className="shrink-0" />}
        {pergunta
          ? <span className="plano-selo">Esperando sua aprovação</span>
          : hora && <time className="bubble-time">{hora}</time>}
      </header>
      <div className={`plano-corpo bubble-markdown prose prose-sm max-w-none${longo && !inteiro ? " is-recolhido" : ""}`}>
        <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}
          components={{ pre: BlocoDeCodigo, img: ImagemDoMarkdown, code: CodigoOuCaminho }}>{plano}</ReactMarkdown>
      </div>
      {longo && (
        <button type="button" className="plano-expandir" aria-expanded={inteiro} onClick={() => setInteiro((v) => !v)}>
          {inteiro ? "Mostrar menos" : "Ver plano inteiro"}
        </button>
      )}
      {pergunta && <div className="plano-pergunta">{pergunta}</div>}
    </article>
  );
}
