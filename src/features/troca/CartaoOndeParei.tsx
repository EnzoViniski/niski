/**
 * Fork (troca-de-contexto): o cartão "onde parei" no topo da conversa e a marca "novo desde a sua
 * última olhada" no meio dela. Lido em 3 segundos: onde está agora (e o que precisa de você), o que
 * você pediu, o que ele fez desde então, o que disse por último.
 */
import type { RefObject } from "react";
import type { OndePareiNaTela } from "./useOndeParei";
import { pedacosDosFeitos } from "./ondeParei";
import "./troca.css";

/** "agora", "há 4 min", "há 2 h", "há 3 dias". */
export function haQuanto(ms: number): string {
  const minutos = Math.round(ms / 60_000);
  if (minutos < 1) return "agora";
  if (minutos < 60) return `há ${minutos} min`;
  const horas = Math.round(minutos / 60);
  if (horas < 24) return `há ${horas} h`;
  const dias = Math.round(horas / 24);
  return `há ${dias} ${dias === 1 ? "dia" : "dias"}`;
}

function movimentoReduzido(): boolean {
  return document.documentElement.dataset.motion === "desligadas"
    || (typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
}

function rolarAte(elemento: Element | null | undefined, bloco: ScrollLogicalPosition) {
  elemento?.scrollIntoView?.({ block: bloco, behavior: movimentoReduzido() ? "auto" : "smooth" });
}

export function CartaoOndeParei({ onde, tela, agora = Date.now() }: {
  onde: OndePareiNaTela;
  tela: RefObject<HTMLElement | null>;
  agora?: number;
}) {
  const { cartao, aberto } = onde;
  if (!aberto) {
    if (!onde.podeReabrir) return null;
    return (
      <button type="button" className="onde-parei-reabrir" onClick={onde.abrir} title="Resumo de onde esta conversa está">
        Onde parei
      </button>
    );
  }
  const { estado, feitos, plano, extra } = cartao;
  const pedacos = pedacosDosFeitos(feitos);
  const novidades = cartao.janela === "olhada" && cartao.novas > 0
    ? `${cartao.novas}${cartao.truncado ? "+" : ""} ${cartao.novas === 1 && !cartao.truncado ? "novidade" : "novidades"}`
    : null;
  const ausencia = onde.olhadaEm ? `você saiu ${haQuanto(Math.max(0, agora - onde.olhadaEm))}` : null;
  const contexto = [novidades, ausencia].filter(Boolean).join(" · ");
  const precisa = estado.tom === "precisa";

  return (
    <section className="onde-parei" data-tom={estado.tom} aria-label="Onde parei" aria-live="polite">
      <div className="onde-parei-topo">
        <span className="onde-parei-ponto" aria-hidden />
        <strong className="onde-parei-estado">{estado.rotulo}</strong>
        {contexto && <span className="onde-parei-contexto">{contexto}</span>}
        <span className="flex-1" />
        <button type="button" className="onde-parei-fechar" onClick={onde.fechar} aria-label="Fechar o resumo" title="Fechar (Esc)">×</button>
      </div>
      {estado.detalhe && <p className="onde-parei-detalhe" title={estado.detalhe}>{estado.detalhe}</p>}
      <dl className="onde-parei-linhas">
        {cartao.pedido && (
          <div><dt>Você pediu</dt><dd title={cartao.pedido}>{cartao.pedido}</dd></div>
        )}
        {(pedacos.length > 0 || plano) && (
          <div>
            <dt>{cartao.janela === "olhada" ? "Desde então" : "Fez"}</dt>
            <dd className="onde-parei-feitos">
              {pedacos.map((pedaco) => (
                <span key={pedaco.texto} className="onde-parei-pedaco" data-tom={pedaco.tom}>
                  {pedaco.tom === "ok" && <span aria-hidden>✓ </span>}
                  {pedaco.tom === "falha" && <span aria-hidden>✗ </span>}
                  {pedaco.texto}
                </span>
              ))}
              {plano && (
                <span className="onde-parei-pedaco" title={plano.fazendo ?? undefined}>
                  Plano {plano.feitas}/{plano.total}{plano.fazendo ? ` · ${plano.fazendo}` : ""}
                </span>
              )}
            </dd>
          </div>
        )}
        {cartao.ultimaFala && !precisa && (
          <div><dt>Disse</dt><dd title={cartao.ultimaFala}>{cartao.ultimaFala}</dd></div>
        )}
        {extra?.bloqueio && <div><dt>Travado em</dt><dd>{extra.bloqueio}</dd></div>}
        {extra?.proximo && <div><dt>Próximo</dt><dd>{extra.proximo}</dd></div>}
      </dl>
      {(onde.marca !== null || precisa) && (
        <div className="onde-parei-acoes">
          {onde.marca !== null && (
            <button type="button" className="onde-parei-acao"
              onClick={() => rolarAte(tela.current?.querySelector(".onde-parei-marca"), "start")}>
              Ver o que chegou
            </button>
          )}
          {precisa && (
            <button type="button" className="onde-parei-acao is-principal"
              onClick={() => { rolarAte(tela.current?.querySelector(".pergunta, .bubble-aprovacao, .chat > :last-child"), "end"); onde.fechar(); }}>
              Responder
            </button>
          )}
        </div>
      )}
    </section>
  );
}

/** A linha "novo desde a sua última olhada", antes da primeira mensagem nova. */
export function MarcaDeNovo() {
  return (
    <p className="onde-parei-marca" role="separator" aria-label="Novo desde a sua última olhada">
      <span>Novo desde a sua última olhada</span>
    </p>
  );
}
