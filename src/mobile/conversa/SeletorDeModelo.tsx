// Modelo e esforço num botão só, no canto direito do campo, como no app do Codex (pedido do Enzo,
// 21/10/2026). Clicar abre um painel: em cima, o esforço em destaque e o modelo (o bloco inteiro é
// clicável e leva à lista de modelos); embaixo, uma barra para arrastar o esforço, que segue o dedo e
// encaixa no nível ao soltar. No nível mais alto a barra vira uma nebulosa com estrelas. O painel abre
// e fecha animado, e a troca para a lista anima a altura. Quem aplica (e reabre a conversa com o ajuste
// novo) é o `AjustesDaConversa`, ao fechar o painel (`onFechar`).
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { CheckIcon, ChevronDownIcon, LoaderIcon, ReloadIcon } from "../../components/ui/PixelIcon";
import { motionOn } from "../../lib/motion";
import "./seletorDeModelo.css";

export const ROTULO_DO_ESFORCO: Record<string, string> = {
  minimal: "Mínimo", low: "Baixo", medium: "Médio", high: "Alto", xhigh: "Muito alto", max: "Máximo", ultra: "Ultra",
};

export function rotuloDoEsforco(esforco: string): string {
  return ROTULO_DO_ESFORCO[esforco] ?? esforco;
}

/** Matiz do rótulo do esforço: do azul (pouco) ao violeta (máximo), pela posição na lista. A
 *  luminosidade vem do CSS (`--h`), clara no tema escuro e mais fechada no claro. */
export function matizDoEsforco(indice: number, total: number): number {
  const t = total <= 1 ? 1 : indice / (total - 1);
  return Math.round(222 + (272 - 222) * t);
}

/** Nível mais perto de uma posição da barra (0..1). */
export function nivelDaFracao(fracao: number, total: number): number {
  if (total <= 1) return 0;
  return Math.max(0, Math.min(total - 1, Math.round(fracao * (total - 1))));
}

export interface ModeloDaLista { id: string; label: string; efforts: string[] }

const SAIDA_MS = 140;

/** Estrelas da nebulosa do nível máximo: posições fixas (sem aleatório a cada render). */
const ESTRELAS = [
  [6, 30, 0], [12, 70, 1.1], [19, 45, 2.3], [27, 20, .6], [33, 78, 1.7], [41, 38, 2.9], [48, 62, .3],
  [55, 25, 1.4], [61, 72, 2.1], [68, 42, .9], [74, 18, 2.6], [80, 66, 1.2], [86, 35, .2], [92, 74, 1.9],
  [9, 58, 1.6], [23, 82, .8], [37, 15, 2.4], [52, 50, .5], [65, 85, 1.8], [77, 52, 2.2], [89, 22, 1],
] as const;

function BarraDeEsforco({ total, indice, padrao, onNivel }: {
  total: number;
  indice: number;
  /** Sem esforço escolhido: a barra fica no meio, apagada. */
  padrao: boolean;
  onNivel: (nivel: number) => void;
}) {
  const trilho = useRef<HTMLDivElement>(null);
  const [arrastando, setArrastando] = useState<number | null>(null);
  const fracao = arrastando ?? (total > 1 ? indice / (total - 1) : 1);
  const maximo = !padrao && total > 1 && nivelDaFracao(fracao, total) === total - 1;

  function fracaoDoPonteiro(x: number): number {
    const caixa = trilho.current?.getBoundingClientRect();
    if (!caixa || caixa.width <= 0) return fracao;
    const botao = 36;
    return Math.max(0, Math.min(1, (x - caixa.left - botao / 2) / (caixa.width - botao)));
  }
  function mover(x: number) {
    const f = fracaoDoPonteiro(x);
    setArrastando(f);
    const nivel = nivelDaFracao(f, total);
    if (padrao || nivel !== indice) onNivel(nivel);
  }
  function teclas(evento: KeyboardEvent<HTMLDivElement>) {
    const alvo = evento.key === "ArrowRight" || evento.key === "ArrowUp" ? indice + 1
      : evento.key === "ArrowLeft" || evento.key === "ArrowDown" ? indice - 1
      : evento.key === "Home" ? 0 : evento.key === "End" ? total - 1 : null;
    if (alvo === null) return;
    evento.preventDefault();
    onNivel(Math.max(0, Math.min(total - 1, alvo)));
  }

  return (
    <div ref={trilho} role="slider" tabIndex={0} aria-label="Esforço do raciocínio"
      aria-valuemin={0} aria-valuemax={total - 1} aria-valuenow={indice}
      className={`barra-esforco${maximo ? " is-maximo" : ""}${padrao ? " is-padrao" : ""}${arrastando !== null ? " is-arrastando" : ""}`}
      style={{ "--f": fracao } as React.CSSProperties}
      onKeyDown={teclas}
      onPointerDown={(evento: PointerEvent<HTMLDivElement>) => {
        evento.currentTarget.setPointerCapture?.(evento.pointerId);
        mover(evento.clientX);
      }}
      onPointerMove={(evento) => { if (arrastando !== null) mover(evento.clientX); }}
      onPointerUp={() => setArrastando(null)}
      onPointerCancel={() => setArrastando(null)}>
      <span className="barra-esforco-cheia" aria-hidden>
        {maximo && ESTRELAS.map(([x, y, atraso], i) => (
          <i key={i} className="barra-estrela" style={{ left: `${x}%`, top: `${y}%`, animationDelay: `${atraso}s` }} />
        ))}
      </span>
      <span className="barra-esforco-botao" aria-hidden />
    </div>
  );
}

export function SeletorDeModelo({ modelos, modelo, esforco, esforcos, desativado, motivo, aplicando, onModelo, onEsforco, onFechar }: {
  modelos: ModeloDaLista[];
  /** "" = padrão. */
  modelo: string;
  esforco: string;
  esforcos: string[];
  desativado?: boolean;
  motivo?: string;
  /** Reabrindo a conversa com o ajuste novo. */
  aplicando?: boolean;
  onModelo: (id: string) => void;
  onEsforco: (esforco: string) => void;
  /** O painel fechou: hora de aplicar o que mudou. */
  onFechar?: () => void;
}) {
  const [estado, setEstado] = useState<"fechado" | "aberto" | "fechando">("fechado");
  const [vista, setVista] = useState<"esforco" | "modelos">("esforco");
  const [paraBaixo, setParaBaixo] = useState(false);
  const caixa = useRef<HTMLSpanElement>(null);
  const corpo = useRef<HTMLDivElement>(null);
  const alturaAntes = useRef<number | null>(null);
  const onFecharRef = useRef(onFechar);
  onFecharRef.current = onFechar;
  const aberto = estado === "aberto";

  function fechar() {
    if (estado !== "aberto") return;
    onFecharRef.current?.();
    setEstado(motionOn() ? "fechando" : "fechado");
  }

  useEffect(() => {
    if (estado !== "fechando") return;
    const id = window.setTimeout(() => setEstado("fechado"), SAIDA_MS);
    return () => window.clearTimeout(id);
  }, [estado]);

  useEffect(() => {
    if (!aberto) return;
    const fora = (evento: globalThis.PointerEvent) => { if (!caixa.current?.contains(evento.target as Node)) fechar(); };
    const tecla = (evento: globalThis.KeyboardEvent) => { if (evento.key === "Escape") fechar(); };
    window.addEventListener("pointerdown", fora);
    window.addEventListener("keydown", tecla);
    return () => { window.removeEventListener("pointerdown", fora); window.removeEventListener("keydown", tecla); };
  }, [aberto]);

  // Troca esforço ↔ lista de modelos: a altura do painel desliza da antiga para a nova.
  function trocarVista(nova: "esforco" | "modelos") {
    alturaAntes.current = corpo.current?.getBoundingClientRect().height ?? null;
    setVista(nova);
  }
  useLayoutEffect(() => {
    const elemento = corpo.current;
    const antes = alturaAntes.current;
    alturaAntes.current = null;
    if (!elemento || antes === null || !motionOn() || typeof elemento.animate !== "function") return;
    const depois = elemento.getBoundingClientRect().height;
    if (Math.abs(depois - antes) < 1) return;
    elemento.animate([{ height: `${antes}px` }, { height: `${depois}px` }], { duration: 220, easing: "cubic-bezier(.2, .8, .2, 1)" });
  }, [vista]);

  const nomeDoModelo = modelos.find((m) => m.id === modelo)?.label ?? "Modelo padrão";
  const indice = esforcos.indexOf(esforco);
  const temEsforco = indice >= 0;
  // Sem esforço escolhido (padrão), a barra fica no meio e o rótulo diz "Padrão".
  const posicao = temEsforco ? indice : Math.floor((esforcos.length - 1) / 2);
  const cor = temEsforco ? ({ "--h": matizDoEsforco(indice, esforcos.length) } as React.CSSProperties) : undefined;

  function alternar() {
    if (desativado) return;
    if (aberto) { fechar(); return; }
    const topo = caixa.current?.getBoundingClientRect().top ?? 999;
    setParaBaixo(topo < 260);
    setVista("esforco");
    setEstado("aberto");
  }

  return (
    <span className="seletor-modelo" ref={caixa}>
      <button type="button" className={`seletor-gatilho${aberto ? " is-aberto" : ""}`} aria-haspopup="dialog" aria-expanded={aberto} disabled={desativado}
        title={motivo ?? "Modelo e esforço"} onClick={alternar}>
        <span className="seletor-gatilho-modelo">{nomeDoModelo}</span>
        {esforcos.length > 0 && (
          <span className="seletor-gatilho-esforco" style={cor} data-colorido={cor ? "" : undefined}>
            {temEsforco ? rotuloDoEsforco(esforco) : "Padrão"}
          </span>
        )}
        {aplicando ? <LoaderIcon aria-hidden className="anexo-girando h-3 w-3 shrink-0" /> : <ChevronDownIcon aria-hidden className="seletor-gatilho-seta h-3 w-3 shrink-0" />}
      </button>

      {estado !== "fechado" && (
        <div className={`seletor-pop${paraBaixo ? " is-baixo" : ""}${estado === "fechando" ? " is-saindo" : ""}`} role="dialog" aria-label="Modelo e esforço">
          <div ref={corpo} className="seletor-corpo">
            {vista === "esforco" ? (
              <div key="esforco" className="seletor-vista">
                <div className="seletor-topo">
                  <button type="button" className="seletor-cabecalho" onClick={() => trocarVista("modelos")} title="Trocar o modelo">
                    <span className="seletor-esforco" style={cor} data-colorido={cor ? "" : undefined}>
                      {esforcos.length === 0 ? "Esforço padrão" : temEsforco ? rotuloDoEsforco(esforco) : "Padrão"}
                    </span>
                    <span className="seletor-modelo-nome">{nomeDoModelo} <span aria-hidden>›</span></span>
                  </button>
                  <button type="button" className="seletor-reset" aria-label="Voltar ao padrão" title="Modelo e esforço padrão"
                    onClick={() => { onModelo(""); onEsforco(""); }}>
                    <ReloadIcon aria-hidden className="h-3.5 w-3.5" />
                  </button>
                </div>
                {esforcos.length > 1 && (
                  <BarraDeEsforco total={esforcos.length} indice={posicao} padrao={!temEsforco}
                    onNivel={(nivel) => onEsforco(esforcos[nivel] ?? "")} />
                )}
              </div>
            ) : (
              <div key="modelos" className="seletor-vista seletor-lista" role="listbox" aria-label="Selecionar modelo">
                <button type="button" className="seletor-voltar" onClick={() => trocarVista("esforco")}>
                  <span aria-hidden>‹</span> Selecionar modelo
                </button>
                <button type="button" role="option" aria-selected={modelo === ""} className="seletor-item"
                  onClick={() => { onModelo(""); trocarVista("esforco"); }}>
                  <span><span className="seletor-item-nome">Padrão</span><span className="seletor-item-desc">O que a CLI escolher</span></span>
                  {modelo === "" && <CheckIcon aria-hidden className="h-3.5 w-3.5" />}
                </button>
                {modelos.map((m) => (
                  <button key={m.id} type="button" role="option" aria-selected={modelo === m.id} className="seletor-item"
                    onClick={() => { onModelo(m.id); trocarVista("esforco"); }}>
                    <span className="seletor-item-nome">{m.label}</span>
                    {modelo === m.id && <CheckIcon aria-hidden className="h-3.5 w-3.5" />}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </span>
  );
}
