// Fork (trocar agente): "Continuar com…" — outro agente (ou outra conta) na mesma conversa.
//
// Componente solto de propósito: o seletor de modelo/esforço está sendo refeito em paralelo, e quem
// encaixa isto no campo de mensagem só precisa das props abaixo. A lógica está no engine
// (`mobile/trocar_agente.rs`); aqui é escolher, confirmar e mostrar o estado.
import { useCallback, useEffect, useRef, useState } from "react";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { LoaderIcon } from "../../components/ui/PixelIcon";
import { useAcoesDaPaleta } from "../../features/paleta/acoes";
import { ApiError } from "../api";
import { destinosDaTroca, lerOpcoesDeTroca, trocarAgente, type Destino, type OpcoesDeTroca, type ResultadoDaTroca } from "./trocaDeAgente";
import "./trocarAgente.css";

export interface TrocarAgenteProps {
  conversaId: string;
  /** Agente e conta atuais (da `Conversation`): só para o rótulo do botão até as opções chegarem. */
  provider: string | null;
  profileId?: string | null;
  /** Agente parado (ou sem sessão): dá para trocar. Com ele trabalhando o botão fica desligado. */
  pronto: boolean;
  /** O que já está escrito no campo. Vai junto para o agente novo, depois da passagem de bastão. */
  mensagem?: string;
  /** Depois da troca: limpar o campo (se `mensagem_enviada`), recarregar a conversa. */
  onTrocado?: (resultado: ResultadoDaTroca) => void;
  /** Título da conversa, para a ação no ⌘K. */
  titulo?: string;
}

const nome = (d: Pick<Destino, "label" | "conta">) => d.conta ? `${d.label} · ${d.conta}` : d.label;

export function TrocarAgente({ conversaId, provider, profileId, pronto, mensagem, onTrocado, titulo }: TrocarAgenteProps) {
  const [aberto, setAberto] = useState(false);
  const [opcoes, setOpcoes] = useState<OpcoesDeTroca | null>(null);
  const [escolhido, setEscolhido] = useState<Destino | null>(null);
  const [trocando, setTrocando] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const raiz = useRef<HTMLDivElement>(null);

  const carregar = useCallback(async () => {
    setErro(null);
    try { setOpcoes(await lerOpcoesDeTroca(conversaId)); } catch (motivo) {
      setOpcoes(null);
      setErro(motivo instanceof ApiError && motivo.status === 404 ? "Reinicie o app para trocar de agente nesta conversa" : String(motivo instanceof Error ? motivo.message : motivo));
    }
  }, [conversaId]);
  const abrir = useCallback(() => { setAberto(true); setEscolhido(null); setAviso(null); void carregar(); }, [carregar]);
  useEffect(() => { setAberto(false); setEscolhido(null); setOpcoes(null); setAviso(null); setErro(null); }, [conversaId]);
  // Clicar fora fecha (menos no meio da troca).
  useEffect(() => {
    if (!aberto) return;
    const fora = (e: MouseEvent) => { if (!trocando && raiz.current && !raiz.current.contains(e.target as Node)) setAberto(false); };
    document.addEventListener("mousedown", fora);
    return () => document.removeEventListener("mousedown", fora);
  }, [aberto, trocando]);
  useAcoesDaPaleta("trocar-agente", [{ id: `trocar-agente-${conversaId}`, titulo: `Continuar com outro agente…${titulo ? ` (${titulo})` : ""}`, executar: abrir }]);

  async function confirmar() {
    if (!escolhido) return;
    setTrocando(true); setErro(null);
    try {
      const resultado = await trocarAgente(conversaId, { provider: escolhido.provider, profile_id: escolhido.profileId, mensagem });
      setAberto(false); setEscolhido(null);
      setAviso(resultado.modo === "passagem"
        ? resultado.conversa_no_chat === false
          ? `Continuando com o ${nome(escolhido)} no terminal. Ele lê a passagem de bastão sozinho.`
          : `Continuando com o ${nome(escolhido)}. Ele lê o resumo completo da conversa antes de responder.`
        : `Continuando na conta ${escolhido.conta ?? ""}, de onde parou.`);
      onTrocado?.(resultado);
    } catch (motivo) {
      setErro(motivo instanceof Error ? motivo.message : String(motivo));
    } finally {
      setTrocando(false);
    }
  }

  const destinos = opcoes ? destinosDaTroca({ ...opcoes, profile_id: opcoes.profile_id ?? profileId ?? null }) : [];
  // O engine reconhece a CLI presa na trava "aberta em outro app" como parada e pode trocá-la.
  const bloqueio = opcoes ? (!opcoes.pode_trocar ? opcoes.motivo ?? "Agora não dá para trocar" : null)
    : !pronto ? "Espere o agente terminar para trocar" : null;
  const mesmoAgente = escolhido && escolhido.provider === (opcoes?.provider ?? provider);

  return (
    <div className="trocar-agente" ref={raiz}>
      <button type="button" className="composer-botao trocar-agente-botao" aria-haspopup="menu" aria-expanded={aberto}
        disabled={trocando} title={!pronto ? "Espere o agente terminar para trocar" : "Continuar esta conversa com outro agente ou conta"}
        onClick={() => (aberto ? setAberto(false) : abrir())}>
        {trocando ? <LoaderIcon aria-hidden className="anexo-girando h-3.5 w-3.5" /> : <AgentIcon provider={provider} size={13} />}
        <span>{trocando ? "Trocando…" : "Continuar com…"}</span>
      </button>
      {aberto && (
        <div className="trocar-agente-painel" role="dialog" aria-label="Continuar com outro agente">
          {!escolhido ? <>
            <p className="trocar-agente-titulo">Continuar esta conversa com…</p>
            {!opcoes && !erro && <p className="m-dica">Carregando agentes…</p>}
            {opcoes && destinos.length === 0 && <p className="m-dica">Nenhum outro agente instalado ou conta cadastrada.</p>}
            {bloqueio && opcoes && <p className="m-dica" role="status">{bloqueio}</p>}
            <ul role="menu" className="trocar-agente-lista">
              {destinos.map((d) => (
                <li key={`${d.provider}:${d.profileId ?? ""}`} role="none">
                  <button type="button" role="menuitem" disabled={Boolean(bloqueio)} onClick={() => setEscolhido(d)}>
                    <AgentIcon provider={d.provider} size={14} className="shrink-0" />
                    <span className="trocar-agente-nome">{d.label}</span>
                    {d.conta && <span className="trocar-agente-conta">{d.conta}</span>}
                    {!d.conversa && <span className="trocar-agente-conta">só terminal</span>}
                  </button>
                </li>
              ))}
            </ul>
          </> : (
            <div className="trocar-agente-confirmar">
              <p className="trocar-agente-titulo"><AgentIcon provider={escolhido.provider} size={14} /> Continuar com o {nome(escolhido)}?</p>
              <p className="m-dica">
                {mesmoAgente
                  ? "Mesma IA, outra conta: a conversa vai junto e continua exatamente de onde parou."
                  : `A conversa continua aqui. O ${escolhido.label} recebe um resumo completo (pedidos, respostas, arquivos, comandos e tarefas) e segue de onde parou.`}
                {!mesmoAgente && mensagem?.trim() ? " O que você já escreveu vai junto." : ""}
              </p>
              <div className="trocar-agente-acoes">
                <button type="button" className="m-chip-aplicar" disabled={trocando} onClick={() => void confirmar()}>
                  {trocando ? <><LoaderIcon aria-hidden className="anexo-girando h-3.5 w-3.5" /> Trocando…</> : "Continuar"}
                </button>
                <button type="button" className="composer-botao" disabled={trocando} onClick={() => setEscolhido(null)}>Voltar</button>
              </div>
            </div>
          )}
          {erro && <p role="alert" className="m-alerta">{erro}</p>}
        </div>
      )}
      {!aberto && erro && <span role="alert" className="m-alerta">{erro}</span>}
      {!aberto && aviso && <span role="status" className="m-dica trocar-agente-aviso">{aviso}</span>}
    </div>
  );
}
