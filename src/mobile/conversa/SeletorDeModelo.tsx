// Fork: modelo e esforço num botão só, no canto direito do campo, como no app do Codex (pedido do Enzo,
// 21/10/2026). Clicar abre um painel com o esforço em destaque e uma barra para arrastar (o nível mais
// alto ganha brilho animado); clicar no nome do modelo troca para a lista de modelos. Quem aplica (e
// reabre a conversa com o ajuste novo) continua sendo o `AjustesDaConversa`.
import { useEffect, useRef, useState } from "react";
import { CheckIcon, ChevronDownIcon, ReloadIcon } from "../../components/ui/PixelIcon";
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

export interface ModeloDaLista { id: string; label: string; efforts: string[] }

export function SeletorDeModelo({ modelos, modelo, esforco, esforcos, desativado, motivo, onModelo, onEsforco }: {
  modelos: ModeloDaLista[];
  /** "" = padrão. */
  modelo: string;
  esforco: string;
  esforcos: string[];
  desativado?: boolean;
  motivo?: string;
  onModelo: (id: string) => void;
  onEsforco: (esforco: string) => void;
}) {
  const [aberto, setAberto] = useState(false);
  const [vista, setVista] = useState<"esforco" | "modelos">("esforco");
  const [paraBaixo, setParaBaixo] = useState(false);
  const caixa = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!aberto) return;
    const fora = (evento: PointerEvent) => { if (!caixa.current?.contains(evento.target as Node)) setAberto(false); };
    const tecla = (evento: KeyboardEvent) => { if (evento.key === "Escape") setAberto(false); };
    window.addEventListener("pointerdown", fora);
    window.addEventListener("keydown", tecla);
    return () => { window.removeEventListener("pointerdown", fora); window.removeEventListener("keydown", tecla); };
  }, [aberto]);

  const nomeDoModelo = modelos.find((m) => m.id === modelo)?.label ?? "Modelo padrão";
  const indice = esforcos.indexOf(esforco);
  const temEsforco = indice >= 0;
  // Sem esforço escolhido (padrão), a barra fica no meio e o rótulo diz "Padrão".
  const posicao = temEsforco ? indice : Math.floor((esforcos.length - 1) / 2);
  const maximo = temEsforco && indice === esforcos.length - 1 && esforcos.length > 1;
  const cor = temEsforco ? ({ "--h": matizDoEsforco(indice, esforcos.length) } as React.CSSProperties) : undefined;
  const pct = esforcos.length > 1 ? (posicao / (esforcos.length - 1)) * 100 : 100;

  function abrir() {
    if (desativado) return;
    const topo = caixa.current?.getBoundingClientRect().top ?? 999;
    setParaBaixo(topo < 260);
    setVista("esforco");
    setAberto((atual) => !atual);
  }

  return (
    <span className="seletor-modelo" ref={caixa}>
      <button type="button" className="seletor-gatilho" aria-haspopup="dialog" aria-expanded={aberto} disabled={desativado}
        title={motivo ?? "Modelo e esforço"} onClick={abrir}>
        <span className="seletor-gatilho-modelo">{nomeDoModelo}</span>
        {esforcos.length > 0 && (
          <span className="seletor-gatilho-esforco" style={cor} data-colorido={cor ? "" : undefined}>
            {temEsforco ? rotuloDoEsforco(esforco) : "Padrão"}
          </span>
        )}
        <ChevronDownIcon aria-hidden className="h-3 w-3 shrink-0" />
      </button>

      {aberto && (
        <div className={`seletor-pop${paraBaixo ? " is-baixo" : ""}`} role="dialog" aria-label="Modelo e esforço">
          {vista === "esforco" ? (
            <>
              <div className="seletor-topo">
                <span className="seletor-esforco" style={cor} data-colorido={cor ? "" : undefined}>
                  {esforcos.length === 0 ? "Esforço padrão" : temEsforco ? rotuloDoEsforco(esforco) : "Padrão"}
                </span>
                <button type="button" className="seletor-reset" aria-label="Voltar ao padrão" title="Modelo e esforço padrão"
                  onClick={() => { onModelo(""); onEsforco(""); }}>
                  <ReloadIcon aria-hidden className="h-3.5 w-3.5" />
                </button>
              </div>
              <button type="button" className="seletor-modelo-link" onClick={() => setVista("modelos")}>
                {nomeDoModelo} <span aria-hidden>›</span>
              </button>
              {esforcos.length > 1 && (
                <div className={`seletor-faixa${maximo ? " is-maximo" : ""}${temEsforco ? "" : " is-padrao"}`}
                  style={{ "--p": `${pct}%` } as React.CSSProperties}>
                  <input type="range" min={0} max={esforcos.length - 1} step={1} value={posicao}
                    aria-label="Esforço do raciocínio" aria-valuetext={temEsforco ? rotuloDoEsforco(esforco) : "Padrão"}
                    onChange={(evento) => onEsforco(esforcos[Number(evento.target.value)] ?? "")} />
                </div>
              )}
              {esforcos.length > 1 && (
                <div className="seletor-marcas" aria-hidden>
                  <span>{rotuloDoEsforco(esforcos[0])}</span><span>{rotuloDoEsforco(esforcos[esforcos.length - 1])}</span>
                </div>
              )}
            </>
          ) : (
            <div className="seletor-lista" role="listbox" aria-label="Selecionar modelo">
              <p className="seletor-lista-titulo">Selecionar modelo</p>
              <button type="button" role="option" aria-selected={modelo === ""} className="seletor-item"
                onClick={() => { onModelo(""); setVista("esforco"); }}>
                <span><span className="seletor-item-nome">Padrão</span><span className="seletor-item-desc">O que a CLI escolher</span></span>
                {modelo === "" && <CheckIcon aria-hidden className="h-3.5 w-3.5" />}
              </button>
              {modelos.map((m) => (
                <button key={m.id} type="button" role="option" aria-selected={modelo === m.id} className="seletor-item"
                  onClick={() => { onModelo(m.id); setVista("esforco"); }}>
                  <span className="seletor-item-nome">{m.label}</span>
                  {modelo === m.id && <CheckIcon aria-hidden className="h-3.5 w-3.5" />}
                </button>
              ))}
            </div>
          )}
        </div>
      )}
    </span>
  );
}
