import { useEffect, useState } from "react";
import { BlocksIcon } from "../../components/ui/PixelIcon";
import { pedirAbrirFluxo } from "./abrirFluxo";
import { PODE_ORQUESTRAR, ativarOrquestracao, papelDaConversa, useResumoDaOrquestracao } from "./orquestracaoService";
import "./orquestracao.css";

/** Painel do layout de onde veio o clique: o fluxo abre ao lado dele. */
function painelDoClique(evento: React.MouseEvent) {
  return (evento.currentTarget as HTMLElement).closest<HTMLElement>("[data-drop-pane]")?.dataset.dropPane;
}

/**
 * Fork: na faixa da conversa, os botões do orquestrador.
 *
 * - "Tornar orquestradora" (só Claude e Codex): reabre a CLI com `--resume` e as ferramentas de criar
 *   e comandar subagentes. Pede um segundo clique, porque reabre a sessão.
 * - "Ver fluxo (N)": abre o fluxo ao vivo numa aba ao lado.
 * - "Deixar de orquestrar": reabre sem as ferramentas; o fluxo continua visível.
 * - Num subagente: "Subagente" leva ao fluxo da orquestradora dele.
 */
export function FaixaDoOrquestrador({ conversaId, provider, titulo }: { conversaId: string | null; provider: string | null | undefined; titulo?: string }) {
  const resumo = useResumoDaOrquestracao();
  const papel = papelDaConversa(resumo, conversaId);
  const [confirmar, setConfirmar] = useState<"ligar" | "desligar" | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  // O "tem certeza?" some sozinho se ninguém clicar.
  useEffect(() => {
    if (!confirmar) return;
    const t = window.setTimeout(() => setConfirmar(null), 4_000);
    return () => window.clearTimeout(t);
  }, [confirmar]);
  if (!conversaId) return null;

  async function trocar(ativo: boolean) {
    if (!conversaId) return;
    setOcupado(true);
    setErro(null);
    try {
      await ativarOrquestracao(conversaId, ativo);
    } catch (motivo) {
      setErro(motivo instanceof Error ? motivo.message : String(motivo));
    } finally {
      setOcupado(false);
      setConfirmar(null);
    }
  }

  const nome = `Fluxo · ${titulo ?? "orquestradora"}`.slice(0, 60);
  const podeLigar = PODE_ORQUESTRAR.includes(provider ?? "");
  return (
    <span className="orq-faixa">
      {erro && <span role="alert" className="orq-faixa-erro" title={erro}>{erro}</span>}
      {papel.dona && (
        <button type="button" className="orq-botao" title="Esta conversa é subagente de uma orquestradora"
          onClick={(e) => pedirAbrirFluxo(papel.dona!, "Fluxo do orquestrador", painelDoClique(e))}>
          <BlocksIcon aria-hidden className="h-3.5 w-3.5" />Subagente
        </button>
      )}
      {(papel.orquestradora || papel.teveFluxo) && (
        <button type="button" className="orq-botao is-destaque" title="Abrir o fluxo dos subagentes ao lado"
          onClick={(e) => pedirAbrirFluxo(conversaId, nome, painelDoClique(e))}>
          <BlocksIcon aria-hidden className="h-3.5 w-3.5" />Ver fluxo ({papel.agentes})
        </button>
      )}
      {papel.orquestradora ? (
        <button type="button" className="orq-botao is-discreto" disabled={ocupado}
          title="Reabre a conversa sem as ferramentas de orquestrar; o fluxo continua"
          onClick={() => (confirmar === "desligar" ? void trocar(false) : setConfirmar("desligar"))}>
          {confirmar === "desligar" ? "Reabrir sem ferramentas?" : "Deixar de orquestrar"}
        </button>
      ) : !papel.dona && podeLigar ? (
        <button type="button" className="orq-botao" disabled={ocupado}
          title="Reabre esta conversa (sem perder nada) com ferramentas para criar e comandar subagentes"
          onClick={() => (confirmar === "ligar" ? void trocar(true) : setConfirmar("ligar"))}>
          <BlocksIcon aria-hidden className="h-3.5 w-3.5" />
          {ocupado ? "Reabrindo…" : confirmar === "ligar" ? "Reabrir com ferramentas?" : "Tornar orquestradora"}
        </button>
      ) : null}
    </span>
  );
}
