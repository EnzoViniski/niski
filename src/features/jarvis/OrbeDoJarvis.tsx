// Jarvis. O orbe: núcleo que pulsa com a voz do Jarvis, anel de marcas que ondula com a voz
// do Enzo e arcos que giram (mais rápido pensando). Desenhado num canvas, um quadro por vez; para
// quando a janela não está visível (energia) e gira devagar com "reduzir movimento".
import { useEffect, useRef, type MutableRefObject } from "react";
import type { EstadoDaVoz } from "./estadoDoJarvis";

export interface Niveis { mic: number; fala: number }

const CORES: Record<EstadoDaVoz | "erro", { forte: string; suave: string }> = {
  iniciando: { forte: "120, 200, 255", suave: "60, 120, 200" },
  ouvindo: { forte: "92, 225, 255", suave: "40, 140, 255" },
  pensando: { forte: "255, 190, 92", suave: "255, 120, 40" },
  falando: { forte: "160, 240, 255", suave: "70, 170, 255" },
  pausado: { forte: "150, 160, 175", suave: "90, 100, 115" },
  erro: { forte: "255, 110, 110", suave: "200, 50, 60" },
};

const GIRO: Record<EstadoDaVoz | "erro", number> = { iniciando: 0.5, ouvindo: 0.35, pensando: 1.6, falando: 0.6, pausado: 0.08, erro: 0.1 };

export function OrbeDoJarvis({ estado, niveis, tamanho, compacto = false }: {
  estado: EstadoDaVoz | "erro";
  niveis: MutableRefObject<Niveis>;
  tamanho: number;
  compacto?: boolean;
}) {
  const tela = useRef<HTMLCanvasElement>(null);
  const estadoAtual = useRef(estado);
  estadoAtual.current = estado;

  useEffect(() => {
    const canvas = tela.current;
    const ctx = canvas?.getContext?.("2d");
    if (!canvas || !ctx) return;
    const escala = window.devicePixelRatio || 1;
    canvas.width = tamanho * escala;
    canvas.height = tamanho * escala;
    ctx.scale(escala, escala);
    const devagar = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    let quadro = 0;
    let angulo = 0;
    let mic = 0;
    let fala = 0;
    let anterior = performance.now();

    const desenhar = (agora: number) => {
      quadro = requestAnimationFrame(desenhar);
      if (document.hidden) return;
      const dt = Math.min(0.05, (agora - anterior) / 1000);
      anterior = agora;
      const e = estadoAtual.current;
      // Sobe rápido, desce devagar: o orbe acompanha a fala sem tremer.
      const suavizar = (atual: number, alvo: number) => atual + (alvo - atual) * (alvo > atual ? 0.45 : 0.12);
      mic = suavizar(mic, niveis.current.mic);
      fala = suavizar(fala, niveis.current.fala);
      angulo += dt * GIRO[e] * (devagar ? 0.25 : 1) * Math.PI;
      const { forte, suave } = CORES[e];
      const c = tamanho / 2;
      const r = tamanho * 0.46;
      ctx.clearRect(0, 0, tamanho, tamanho);

      // Halo.
      const halo = ctx.createRadialGradient(c, c, r * 0.2, c, c, r * 1.05);
      halo.addColorStop(0, `rgba(${suave}, ${0.22 + fala * 0.35})`);
      halo.addColorStop(1, `rgba(${suave}, 0)`);
      ctx.fillStyle = halo;
      ctx.beginPath(); ctx.arc(c, c, r * 1.05, 0, Math.PI * 2); ctx.fill();

      // Núcleo: cresce com a voz do Jarvis.
      const raioDoNucleo = r * (0.26 + fala * 0.16 + (e === "pensando" ? Math.sin(agora / 260) * 0.02 : 0));
      const nucleo = ctx.createRadialGradient(c, c, 0, c, c, raioDoNucleo);
      nucleo.addColorStop(0, `rgba(255, 255, 255, ${0.9})`);
      nucleo.addColorStop(0.35, `rgba(${forte}, 0.85)`);
      nucleo.addColorStop(1, `rgba(${suave}, 0)`);
      ctx.fillStyle = nucleo;
      ctx.beginPath(); ctx.arc(c, c, raioDoNucleo, 0, Math.PI * 2); ctx.fill();

      // Anel fino.
      ctx.strokeStyle = `rgba(${forte}, 0.55)`;
      ctx.lineWidth = 1.2;
      ctx.beginPath(); ctx.arc(c, c, r * 0.5, 0, Math.PI * 2); ctx.stroke();

      // Marcas: ondulam com a voz do Enzo.
      if (!compacto) {
        const marcas = 72;
        for (let i = 0; i < marcas; i++) {
          const a = (i / marcas) * Math.PI * 2 - angulo * 0.3;
          const onda = (Math.sin(a * 3 + agora / 180) + 1) / 2;
          const tamanhoDaMarca = r * (0.04 + mic * 0.16 * (0.4 + onda * 0.6));
          const de = r * 0.62;
          ctx.strokeStyle = `rgba(${forte}, ${0.25 + mic * 0.6 * onda})`;
          ctx.lineWidth = 1.4;
          ctx.beginPath();
          ctx.moveTo(c + Math.cos(a) * de, c + Math.sin(a) * de);
          ctx.lineTo(c + Math.cos(a) * (de + tamanhoDaMarca), c + Math.sin(a) * (de + tamanhoDaMarca));
          ctx.stroke();
        }
      }

      // Arcos que giram em sentidos opostos.
      const arcos: Array<[number, number, number, number]> = [
        [0.86, 1.1, 2.4, 2.2], [0.86, 1.1 + Math.PI, 1.2, 2.2], [0.96, -1.6, 0.9, 1.1], [0.96, 0.4, 0.5, 1.1], [0.96, 2.2, 1.4, 1.1],
      ];
      arcos.forEach(([raio, inicio, comprimento, largura], i) => {
        const giro = i < 2 ? angulo : -angulo * 0.7;
        ctx.strokeStyle = `rgba(${i < 2 ? forte : suave}, ${0.5 + mic * 0.4})`;
        ctx.lineWidth = largura + (i < 2 ? fala * 2 : mic * 1.5);
        ctx.lineCap = "round";
        ctx.beginPath(); ctx.arc(c, c, r * raio, inicio + giro, inicio + giro + comprimento); ctx.stroke();
      });
    };
    quadro = requestAnimationFrame(desenhar);
    return () => cancelAnimationFrame(quadro);
  }, [tamanho, compacto, niveis]);

  return <canvas ref={tela} className="jarvis-orbe" style={{ width: tamanho, height: tamanho }} aria-hidden />;
}
