// Fork (pergunta-da-cli): a CLI parada esperando algo no terminal que a conversa não sabe responder.
import { AgentIcon } from "../../components/ui/AgentIcon";
import { Button } from "../../components/ui/Button";
import { TerminalIcon } from "../../components/ui/PixelIcon";
import type { EsperaNoTerminal } from "../api";
import "./perguntaDaCli.css";

/**
 * "O agente está esperando algo no terminal": nunca silêncio. Mostra as últimas linhas da tela (o
 * diálogo como a CLI desenhou) e abre o terminal da conversa para responder por lá.
 */
export function CartaoDeEspera({ espera, provider, onAbrirTerminal }: {
  espera: EsperaNoTerminal; provider?: string | null; onAbrirTerminal: () => void;
}) {
  return (
    <section className="pergunta" role="group" aria-label="O agente está esperando algo no terminal">
      <header className="pergunta-topo">
        <span className="pergunta-avatar" aria-hidden><AgentIcon provider={provider} size={14} /></span>
        <span className="pergunta-quem">Esperando no terminal</span>
      </header>
      <h3 className="pergunta-titulo">O agente está esperando algo no terminal</h3>
      <p className="pergunta-dica">A CLI parou num diálogo que a conversa não consegue responder. Abra o terminal e responda por lá.</p>
      <pre className="pergunta-tela" aria-label="Últimas linhas do terminal">{espera.texto}</pre>
      {espera.dica && <p className="pergunta-dica">Teclas: {espera.dica}</p>}
      <footer className="pergunta-rodape">
        <span className="flex-1" />
        <Button variant="primary" size="sm" onClick={onAbrirTerminal}><TerminalIcon aria-hidden className="h-3.5 w-3.5" />Abrir no terminal</Button>
      </footer>
    </section>
  );
}
