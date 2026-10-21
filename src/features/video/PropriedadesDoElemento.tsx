// Fork (Editor de vídeo): propriedades do elemento escolhido no preview — texto, fonte, cor,
// posição, tamanho e giro. Cada mudança aparece na hora (pela ponte) e grava só o `style` dele.
import { useEffect, useState } from "react";
import { Button } from "../../components/ui";
import { lerScale, lerTranslate, nomeDoElemento, px } from "./EdicaoNoPreview";
import type { ElementoNoPreview } from "./Preview";

/** `rgb(1, 2, 3)` / `rgba(...)` → `#010203` (o seletor de cor só aceita hex). */
export function corParaHex(cor: string) {
  const m = /rgba?\(\s*(\d+)[,\s]+(\d+)[,\s]+(\d+)/i.exec(cor);
  if (!m) return /^#[0-9a-f]{6}$/i.test(cor) ? cor : "#ffffff";
  return `#${[m[1], m[2], m[3]].map((v) => Number(v).toString(16).padStart(2, "0")).join("")}`;
}

function Numero({ rotulo, valor, sufixo, onGravar, passo = 1 }: { rotulo: string; valor: number; sufixo: string; onGravar: (v: number) => void; passo?: number }) {
  const [texto, setTexto] = useState(String(Math.round(valor * 10) / 10));
  useEffect(() => setTexto(String(Math.round(valor * 10) / 10)), [valor]);
  const gravar = () => { const v = Number(texto.replace(",", ".")); if (Number.isFinite(v) && Math.abs(v - valor) > 1e-3) onGravar(v); else setTexto(String(Math.round(valor * 10) / 10)); };
  return (
    <label className="video-prop-campo">
      <span>{rotulo}</span>
      <span className="video-prop-entrada">
        <input className="field px-2 py-1 text-xs" inputMode="decimal" aria-label={rotulo} value={texto} step={passo}
          onChange={(e) => setTexto(e.target.value)} onBlur={gravar}
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Enter") { e.preventDefault(); gravar(); }
            else if (e.key === "ArrowUp" || e.key === "ArrowDown") { e.preventDefault(); onGravar((Number(texto.replace(",", ".")) || 0) + (e.key === "ArrowUp" ? 1 : -1) * passo * (e.shiftKey ? 10 : 1)); }
          }} />
        <small>{sufixo}</small>
      </span>
    </label>
  );
}

export function PropriedadesDoElemento({ elemento, bloqueado, onEstilo, onTexto, onFechar, onPedirAoAgente }: {
  elemento: ElementoNoPreview;
  bloqueado: boolean;
  /** Muda o `style` (na hora e no arquivo). */
  onEstilo: (props: Record<string, string | null>, motivo: string) => void;
  onTexto: (texto: string) => void;
  onFechar: () => void;
  /** Põe a referência do elemento no campo da conversa ao lado. */
  onPedirAoAgente?: () => void;
}) {
  const [x, y] = lerTranslate(elemento.translate);
  const escala = lerScale(elemento.scale);
  const giro = Number.parseFloat(elemento.rotate) || 0;
  const nome = nomeDoElemento(elemento);
  const [texto, setTexto] = useState(elemento.texto ?? "");
  useEffect(() => setTexto(elemento.texto ?? ""), [elemento.ref, elemento.texto]);
  return (
    <div className="video-prop" aria-label="Propriedades do elemento">
      <div className="video-prop-cabeca">
        <h3 className="video-prop-titulo" title={elemento.id ? `id: ${elemento.id}` : undefined}>{nome}</h3>
        <button type="button" className="video-mini-botao" aria-label="Tirar a seleção" onClick={onFechar}>×</button>
      </div>
      {!elemento.visivel && <p className="video-prop-aviso">Escondido neste ponto do vídeo. Mova a agulha para onde ele aparece.</p>}
      {elemento.texto !== null && (
        <label className="video-prop-campo is-largo">
          <span>Texto</span>
          <textarea className="field px-2 py-1 text-xs" rows={2} value={texto} disabled={bloqueado} aria-label="Texto"
            onChange={(e) => setTexto(e.target.value)} onBlur={() => { if (texto !== (elemento.texto ?? "")) onTexto(texto); }}
            onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); (e.target as HTMLTextAreaElement).blur(); } }} />
        </label>
      )}
      <div className="video-prop-grade">
        {elemento.fonte !== null && <Numero rotulo="Tamanho da letra" valor={elemento.fonte} sufixo="px" onGravar={(v) => onEstilo({ "font-size": px(Math.max(4, v)) }, `Letra de ${nome} em ${Math.round(v)} px`)} />}
        <label className="video-prop-campo">
          <span>Cor</span>
          <input type="color" className="video-prop-cor" aria-label="Cor" value={corParaHex(elemento.cor)} disabled={bloqueado}
            onChange={(e) => onEstilo({ color: e.target.value }, `Cor de ${nome}`)} />
        </label>
        <Numero rotulo="Posição X" valor={x} sufixo="px" onGravar={(v) => onEstilo({ translate: `${px(v)} ${px(y)}` }, `Movi ${nome}`)} />
        <Numero rotulo="Posição Y" valor={y} sufixo="px" onGravar={(v) => onEstilo({ translate: `${px(x)} ${px(v)}` }, `Movi ${nome}`)} />
        <Numero rotulo="Tamanho" valor={escala * 100} sufixo="%" passo={5} onGravar={(v) => onEstilo({ scale: String(Math.max(5, v) / 100) }, `Tamanho de ${nome}: ${Math.round(v)}%`)} />
        <Numero rotulo="Giro" valor={giro} sufixo="°" onGravar={(v) => onEstilo({ rotate: `${Math.round(v * 10) / 10}deg` }, `Girei ${nome}`)} />
      </div>
      <div className="video-prop-botoes">
        {onPedirAoAgente && <Button size="sm" variant="glass" onClick={onPedirAoAgente} title="Escreve no campo da conversa qual é este elemento; complete com o pedido">Pedir ao agente sobre isto</Button>}
        <Button size="sm" variant="ghost" disabled={bloqueado || (!elemento.translate && !elemento.scale && !elemento.rotate)}
          onClick={() => onEstilo({ translate: null, scale: null, rotate: null }, `Voltei ${nome} para o lugar original`)}>Voltar ao lugar original</Button>
      </div>
      <p className="video-prop-dica">No vídeo: arraste para mover, puxe o canto para aumentar, dois cliques para editar o texto. Para animação e efeitos, peça ao agente na conversa.</p>
    </div>
  );
}
