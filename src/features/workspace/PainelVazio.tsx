import { useEffect, useState } from "react";
import { TerminalIcon } from "../../components/ui/PixelIcon";
import { shortcutLabel } from "../../lib/shortcuts";
import type { PaneKind } from "../../types/workspace";

/** O mascote em pixel: `X` corpo, `E` olho. Desenhado aqui (13×9) para não depender de asset. */
const MASCOTE = [
  "..XXXXXXXXX..",
  "..XXXXXXXXX..",
  "..XXEXXXXEX..",
  "XXXXXXXXXXXXX",
  "XXXXXXXXXXXXX",
  "..XXXXXXXXX..",
  "..XXXXXXXXX..",
];
/** Dois quadros das perninhas: alternar os dois é o "andar". */
const PERNAS = ["..X.X...X.X..", ".X..X...X..X."];

export function Mascote() {
  const celulas = (linha: string, y: number, classe?: string) =>
    [...linha].map((c, x) => (c === "." ? null : (
      <rect key={`${x}-${y}`} x={x} y={y} width={1.02} height={1.02} className={classe ?? (c === "E" ? "vazio-olho" : "vazio-corpo")} />
    )));
  return (
    <svg viewBox="0 0 13 9" className="vazio-mascote" shapeRendering="crispEdges">
      {MASCOTE.flatMap((linha, y) => celulas(linha, y))}
      <g className="vazio-pernas-a">{celulas(PERNAS[0], 7, "vazio-corpo")}{celulas(PERNAS[0], 8, "vazio-corpo")}</g>
      <g className="vazio-pernas-b">{celulas(PERNAS[1], 7, "vazio-corpo")}{celulas(PERNAS[1], 8, "vazio-corpo")}</g>
    </svg>
  );
}

const FALAS = [
  "Bora codar?",
  "Me chama que eu refatoro.",
  "Testes primeiro, né?",
  "Que tal um npm run dev?",
  "Posso revisar esse PR.",
  "Commit pequeno é commit feliz.",
];

/**
 * Painel sem aba: em vez de dois botões soltos, uma cena — o mascote passeando por um chão de pixel
 * com uma fala que troca — e as duas escolhas em cartões, com os atalhos úteis embaixo. Sem animação
 * quando o app ou o Windows pedem menos movimento (CSS).
 */
export function PainelVazio({ onCriar }: { onCriar: (kind: PaneKind, title: string) => void }) {
  const [fala, setFala] = useState(0);
  useEffect(() => {
    const troca = window.setInterval(() => setFala((atual) => (atual + 1) % FALAS.length), 6_000);
    return () => window.clearInterval(troca);
  }, []);

  return (
    <div className="vazio-painel">
      <div className="vazio-cena" aria-hidden>
        {[12, 27, 44, 61, 78, 90].map((x, i) => (
          <span key={x} className="vazio-estrela" style={{ left: `${x}%`, top: `${14 + ((i * 23) % 46)}%`, animationDelay: `${i * 0.7}s` }} />
        ))}
        <div className="vazio-andarilho">
          <span key={fala} className="vazio-fala">{FALAS[fala]}</span>
          {/* Só o mascote vira para voltar; a fala continua legível. */}
          <span className="vazio-vira"><Mascote /></span>
        </div>
        <div className="vazio-chao" />
      </div>

      <div className="vazio-texto">
        <h2 className="vazio-titulo">Painel vazio</h2>
        <p className="vazio-sub">Chame uma IA para trabalhar neste projeto ou abra um terminal para rodar comandos.</p>
      </div>

      <div className="vazio-opcoes">
        <button type="button" className="vazio-opcao vazio-opcao-agente" aria-label="Novo agente" title="Abre o seletor de CLI (Claude, Codex, Cursor…)"
          onClick={() => onCriar("agent", "Novo agente")}>
          <span className="vazio-opcao-icone"><svg viewBox="0 0 13 9" className="h-5 w-6" shapeRendering="crispEdges" aria-hidden>
            {MASCOTE.flatMap((linha, y) => [...linha].map((c, x) => c === "X" ? <rect key={`${x}-${y}`} x={x} y={y} width={1.02} height={1.02} fill="currentColor" /> : null))}
          </svg></span>
          <span className="vazio-opcao-corpo">
            <span className="vazio-opcao-titulo">Novo agente</span>
            <span className="vazio-opcao-desc">Claude, Codex ou Cursor conversando nesta pasta</span>
          </span>
        </button>
        <button type="button" className="vazio-opcao" aria-label="Novo terminal" title="Shell do sistema, sem agente" onClick={() => onCriar("terminal", "Novo terminal")}>
          <span className="vazio-opcao-icone"><TerminalIcon aria-hidden className="h-5 w-5" /></span>
          <span className="vazio-opcao-corpo">
            <span className="vazio-opcao-titulo">Novo terminal</span>
            <span className="vazio-opcao-desc">npm run dev, git, testes — sem IA</span>
          </span>
        </button>
      </div>

      <p className="vazio-dicas">
        <span><kbd>{shortcutLabel("quickOpen")}</kbd> busca arquivo</span>
        <span><kbd>{shortcutLabel("splitHorizontal")}</kbd> divide o painel</span>
        <span><kbd>{shortcutLabel("nextAttention")}</kbd> agente pedindo atenção</span>
      </p>
    </div>
  );
}
