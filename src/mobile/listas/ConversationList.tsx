// Fork (Onda 2.0): saiu do `MobileApp.tsx` sem mudança de lógica.
import { EtiquetaDoStatus, statusDaConversa } from "../../components/ConversaStatus";
import type { TerminalSession } from "../../features/terminal/terminalService";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { PlayIcon } from "../../components/ui/PixelIcon";
import { nomeDaPlataforma } from "../../lib/plataformasDeAgente";
import { rotuloDoEstado, tomDoEstado } from "../../lib/estadosDaSessao";
import { haQuanto } from "../../lib/tempo";
import type { Conversation, Projects } from "../api";
import { Vazio } from "./Secao";

export function ConversationList({ conversations, perfis, projetos = [], mostrarProjeto = false, abrir, vazio, ativa = null, naoVistas }: {
  conversations: Conversation[]; perfis: Projects["profiles"]; projetos?: Projects["projects"];
  mostrarProjeto?: boolean; abrir: (id: string) => void; vazio: string;
  /** Conversa aberta ao lado (web): fica marcada na lista. */
  ativa?: string | null;
  /** Conversas com aviso que você ainda não viu — o que separa "finalizado" do neutro, como no PC. */
  naoVistas?: ReadonlySet<string>;
}) {
  if (conversations.length === 0) return <Vazio>{vazio}</Vazio>;
  return <ul className="m-lista">{conversations.map(c => {
    const aprovar = Boolean(c.capabilities?.approve);
    // Mesmo status da lateral do PC (trabalhando, aprovar, interrompido, finalizado) e mesmo selo.
    const status = statusDaConversa(
      { state: (aprovar ? "approval_required" : c.state) as TerminalSession["state"], notice: (c.notice ?? undefined) as TerminalSession["notice"] },
      Boolean(naoVistas?.has(c.id)),
    );
    const conta = perfis.find(p => p.id === c.profile_id)?.name;
    const projeto = mostrarProjeto ? projetos.find(p => p.id === c.project_id)?.name : null;
    const detalhe = [rotuloDoEstado(c.state) ?? "Histórico", nomeDaPlataforma(c.provider) ?? c.provider, conta, projeto]
      .filter(Boolean).join(" · ");
    return <li key={c.id}>
      <button type="button" className={`m-card glass glass-hover${c.id === ativa ? " m-card-ativo" : ""}`} data-status={status} onClick={() => abrir(c.id)}
        aria-label={`Abrir conversa ${c.title}`} aria-current={c.id === ativa ? "true" : undefined}>
        <span className="m-card-icone">
          {c.provider ? <AgentIcon provider={c.provider} size={20} /> : <PlayIcon aria-hidden className="h-5 w-5" />}
        </span>
        <span className="m-card-corpo">
          <span className="m-card-titulo">{c.title}</span>
          <span className="m-card-sub">
            <span className={`m-dot ${tomDoEstado(c.state, aprovar)}`} aria-hidden />
            {detalhe}
          </span>
        </span>
        <span className="m-card-lado">
          <EtiquetaDoStatus status={status} />
          {haQuanto(c.updated_at_ms) && <time className="m-card-tempo">{haQuanto(c.updated_at_ms)}</time>}
        </span>
      </button>
    </li>;
  })}</ul>;
}

/** A lista em duas partes: o que tem sessão aberta no PC agora (dá para responder na hora) e o
 *  histórico (fechadas — abrir uma permite retomar). Antes vinha tudo misturado, ordenado só por data. */
export function ConversasPorEstado(props: Parameters<typeof ConversationList>[0]) {
  if (props.conversations.length === 0) return <ConversationList {...props} />;
  const abertas = props.conversations.filter(c => c.live);
  const fechadas = props.conversations.filter(c => !c.live);
  return <>
    <h3 className="m-subtitulo"><span className="m-dot m-dot-ativo" aria-hidden />Abertas no PC<span className="m-subtitulo-total">{abertas.length}</span></h3>
    <ConversationList {...props} conversations={abertas} vazio="Nenhuma conversa aberta no PC agora." />
    {fechadas.length > 0 && <>
      <h3 className="m-subtitulo"><span className="m-dot m-dot-parado" aria-hidden />Histórico<span className="m-subtitulo-total">{fechadas.length}</span></h3>
      <ConversationList {...props} conversations={fechadas} vazio="" />
    </>}
  </>;
}
