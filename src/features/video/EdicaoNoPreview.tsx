// Fork (Editor de vídeo): edição na mão em cima do preview. Clique escolhe o elemento (a ponte dentro
// do iframe diz qual), arrastar move, o canto aumenta ou diminui e dois cliques editam o texto.
//
// - Ao vivo pela ponte (`paraPonte`, sem recarregar o preview); ao soltar, grava no arquivo só o
//   `style` daquele elemento (`translate`/`scale`, propriedades soltas que somam com as animações GSAP,
//   que usam `transform`).
// - A camada não pega clique fora da caixa: o clique vai para o vídeo e a ponte escolhe outro elemento.
import { useEffect, useRef, useState, type PointerEvent as EventoDePonteiro } from "react";
import type { ElementoNoPreview } from "./Preview";

export interface Retangulo { x: number; y: number; w: number; h: number }

/** `translate: 10px -4px` → [10, -4] (outras unidades contam como 0). */
export function lerTranslate(v: string): [number, number] {
  const [a, b] = v.trim().split(/\s+/);
  const px = (s?: string) => (s && /^-?[\d.]+(px)?$/.test(s) ? Number.parseFloat(s) : 0);
  return [px(a), px(b)];
}
export function lerScale(v: string) { const n = Number.parseFloat(v.trim().split(/\s+/)[0] ?? ""); return Number.isFinite(n) && n > 0 ? n : 1; }
export const px = (n: number) => `${Math.round(n * 10) / 10}px`;

type Arraste = { tipo: "mover" | "escala"; x0: number; y0: number; t0: [number, number]; s0: number; larguraNaTela: number; props?: Record<string, string> };

export function EdicaoNoPreview({ escala, elemento, passando, editandoTexto, onAoVivo, onGravar, onTexto, onEditarTexto, onFecharTexto }: {
  escala: number;
  elemento: ElementoNoPreview | null;
  passando: Retangulo | null;
  editandoTexto: boolean;
  onAoVivo: (ref: string, props: Record<string, string>) => void;
  onGravar: (ref: string, props: Record<string, string>, motivo: string) => void;
  onTexto: (ref: string, texto: string) => void;
  onEditarTexto: () => void;
  onFecharTexto: () => void;
}) {
  const arraste = useRef<Arraste | null>(null);
  const [rascunho, setRascunho] = useState("");
  useEffect(() => { if (editandoTexto) setRascunho(elemento?.texto ?? ""); }, [editandoTexto, elemento?.ref]); // eslint-disable-line react-hooks/exhaustive-deps

  const caixa = (r: Retangulo) => ({ left: r.x * escala, top: r.y * escala, width: Math.max(4, r.w * escala), height: Math.max(4, r.h * escala) });

  const comecar = (tipo: Arraste["tipo"]) => (e: EventoDePonteiro<HTMLDivElement>) => {
    if (!elemento || e.button > 0) return;
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.setPointerCapture?.(e.pointerId);
    arraste.current = { tipo, x0: e.clientX, y0: e.clientY, t0: lerTranslate(elemento.translate), s0: lerScale(elemento.scale), larguraNaTela: Math.max(8, elemento.rect.w * escala) };
  };
  const mover = (e: EventoDePonteiro<HTMLDivElement>) => {
    const a = arraste.current;
    if (!a || !elemento) return;
    const dx = e.clientX - a.x0, dy = e.clientY - a.y0;
    if (a.tipo === "mover") a.props = { translate: `${px(a.t0[0] + dx / escala)} ${px(a.t0[1] + dy / escala)}` };
    else a.props = { scale: String(Math.round(Math.min(10, Math.max(0.05, a.s0 * (1 + dx / a.larguraNaTela))) * 1000) / 1000) };
    onAoVivo(elemento.ref, a.props);
  };
  const soltar = () => {
    const a = arraste.current;
    arraste.current = null;
    if (!a?.props || !elemento) return;
    onGravar(elemento.ref, a.props, a.tipo === "mover" ? `Movi ${nomeDoElemento(elemento)}` : `Mudei o tamanho de ${nomeDoElemento(elemento)}`);
  };

  return (
    <div className="video-edicao" aria-hidden={!elemento}>
      {passando && (!elemento || passando.x !== elemento.rect.x || passando.y !== elemento.rect.y) && (
        <div className="video-edicao-passando" style={caixa(passando)} />
      )}
      {elemento && !editandoTexto && (
        <div className="video-edicao-caixa" style={caixa(elemento.rect)} role="group" aria-label={`Selecionado: ${nomeDoElemento(elemento)}`}
          title="Arraste para mover · canto para aumentar · dois cliques para editar o texto"
          onPointerDown={comecar("mover")} onPointerMove={mover} onPointerUp={soltar} onPointerCancel={soltar}
          onDoubleClick={() => { if (elemento.texto !== null) onEditarTexto(); }}>
          <span className="video-edicao-nome">{nomeDoElemento(elemento)}</span>
          <div className="video-edicao-canto" aria-label="Arraste para mudar o tamanho" onPointerDown={comecar("escala")} onPointerMove={mover} onPointerUp={soltar} onPointerCancel={soltar} />
        </div>
      )}
      {elemento && editandoTexto && (
        <textarea className="video-edicao-texto" style={caixa(elemento.rect)} autoFocus value={rascunho} aria-label="Texto do elemento"
          onChange={(e) => setRascunho(e.target.value)}
          onBlur={() => { if (rascunho !== (elemento.texto ?? "")) onTexto(elemento.ref, rascunho); onFecharTexto(); }}
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Escape") { setRascunho(elemento.texto ?? ""); onFecharTexto(); }
            else if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); (e.target as HTMLTextAreaElement).blur(); }
          }} />
      )}
    </div>
  );
}

export function nomeDoElemento(el: Pick<ElementoNoPreview, "id" | "tag" | "texto">) {
  if (el.texto?.trim()) return `“${el.texto.trim().slice(0, 28)}${el.texto.trim().length > 28 ? "…" : ""}”`;
  if (el.id) return `#${el.id}`;
  return NOME_DA_TAG[el.tag] ?? el.tag;
}

const NOME_DA_TAG: Record<string, string> = { img: "imagem", video: "vídeo", audio: "áudio", div: "bloco", svg: "desenho", section: "seção", span: "texto", p: "parágrafo" };
