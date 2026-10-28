// Orbe de partículas. O símbolo do Niski feito de partículas que respiram, vibram com a voz
// do Enzo, ondulam do centro para fora com a voz do Jarvis e giram num redemoinho enquanto ele pensa.
// Mesma interface do `OrbeDoJarvis`, que continua como reserva: sem WebGL2, logo que não carrega ou
// contexto perdido, o orbe 2D antigo aparece no lugar. Custo: um buffer estático e uma chamada de
// desenho por quadro; com a janela escondida o laço de animação para por completo.
import { useEffect, useRef, useState, type MutableRefObject } from "react";
import type { EstadoDaVoz } from "./estadoDoJarvis";
import { OrbeDoJarvis, type Niveis } from "./OrbeDoJarvis";
import { amostrarLogo, carregarPixelsDaLogo } from "./particulasDaLogo";
import { aproximar, SuavizadorDeVoz } from "./nivelDaVoz";
import { criarDesenhista, type DesenhistaDoOrbe } from "./orbeDeParticulasGL";

/** Tempo (s) que as partículas levam para formar a logo ao abrir. */
const DURACAO_DA_FORMACAO = 1.6;
/** Intervalo mínimo entre quadros no modo econômico (~30 fps; a folga absorve a variação do vsync). */
const INTERVALO_CALMO_MS = 1000 / 30 - 4;
/** Teto de ~60 fps no modo rápido: em tela de 120 Hz o WebKit chamaria o dobro sem ganho visível. */
const INTERVALO_RAPIDO_MS = 1000 / 60 - 3;

export function OrbeDeParticulas({ estado, niveis, tamanho, compacto = false }: {
  estado: EstadoDaVoz | "erro";
  niveis: MutableRefObject<Niveis>;
  tamanho: number;
  compacto?: boolean;
}) {
  const tela = useRef<HTMLCanvasElement>(null);
  const estadoAtual = useRef(estado);
  estadoAtual.current = estado;
  const [reserva, setReserva] = useState(false);

  useEffect(() => {
    const canvas = tela.current;
    if (!canvas || reserva) return;
    let desenhista: DesenhistaDoOrbe | null = null;
    let quadro = 0;
    let cancelado = false;
    let retomar: (() => void) | null = null;
    const cairNaReserva = () => { if (!cancelado) setReserva(true); };

    // Testa o WebGL2 antes de carregar a logo: no jsdom e em máquina sem GPU cai direto no 2D.
    if (typeof WebGL2RenderingContext === "undefined") { cairNaReserva(); return; }
    const aoPerderContexto = (e: Event) => { e.preventDefault(); cairNaReserva(); };
    canvas.addEventListener("webglcontextlost", aoPerderContexto);

    void carregarPixelsDaLogo().then(({ mascara, cor }) => {
      if (cancelado) return;
      // Densidade por área: ~3 mil no painel, ~1,1 mil na bolinha (pontos maiores).
      const quantidade = compacto ? 1100 : Math.round(Math.min(4200, Math.max(1600, (tamanho * tamanho) / 13)));
      const particulas = amostrarLogo(mascara, cor, quantidade);
      if (!particulas.length) { cairNaReserva(); return; }
      try {
        desenhista = criarDesenhista(canvas, particulas, { lado: tamanho, pontoBase: compacto ? 4.2 : 3.4, escala: compacto ? 0.9 : 0.8 });
      } catch {
        desenhista = null;
      }
      if (!desenhista) { cairNaReserva(); return; }

      const voz = new SuavizadorDeVoz();
      const tons = { pensar: 0, pausa: 0, erro: 0, iniciar: 1 };
      const inicio = performance.now();
      let anterior = inicio;
      const passo = (agora: number) => {
        // Janela escondida: o laço para de vez (nem agenda o próximo quadro) até voltar a aparecer.
        if (document.hidden || !desenhista) { quadro = 0; return; }
        quadro = requestAnimationFrame(passo);
        // 60 quadros por segundo só com o Jarvis falando (as ondas rápidas) e na formação; o resto
        // (ouvindo, pensando, conectando, parado) fica em 30. Medido no WebKit: a taxa de quadros manda
        // no custo (60 fps ≈ 3,5× o de 30), o número de partículas quase não pesa.
        const rapido = voz.fala > 0.02 || agora - inicio < DURACAO_DA_FORMACAO * 1000;
        if (agora - anterior < (rapido ? INTERVALO_RAPIDO_MS : INTERVALO_CALMO_MS)) return;
        const e = estadoAtual.current;
        const dt = Math.min(0.1, Math.max(0, (agora - anterior) / 1000));
        anterior = agora;
        voz.passo(niveis.current, dt);
        tons.pensar = aproximar(tons.pensar, e === "pensando" ? 1 : 0, dt);
        tons.pausa = aproximar(tons.pausa, e === "pausado" ? 1 : 0, dt);
        tons.erro = aproximar(tons.erro, e === "erro" ? 1 : 0, dt);
        tons.iniciar = aproximar(tons.iniciar, e === "iniciando" ? 1 : 0, dt);
        const t = (agora - inicio) / 1000;
        desenhista.desenhar({
          tempo: t,
          forma: Math.min(1, t / DURACAO_DA_FORMACAO),
          fala: voz.fala,
          mic: voz.mic,
          pensar: tons.pensar,
          pausa: tons.pausa,
          erro: tons.erro,
          iniciar: tons.iniciar,
          historico: voz.historico,
        });
      };
      retomar = () => {
        if (document.hidden || quadro || cancelado) return;
        anterior = performance.now();
        quadro = requestAnimationFrame(passo);
      };
      document.addEventListener("visibilitychange", retomar);
      retomar();
    }).catch(cairNaReserva);

    return () => {
      cancelado = true;
      cancelAnimationFrame(quadro);
      if (retomar) document.removeEventListener("visibilitychange", retomar);
      canvas.removeEventListener("webglcontextlost", aoPerderContexto);
      desenhista?.destruir();
    };
  }, [tamanho, compacto, niveis, reserva]);

  if (reserva) return <OrbeDoJarvis estado={estado} niveis={niveis} tamanho={tamanho} compacto={compacto} />;
  return (
    <canvas ref={tela} className="jarvis-orbe jarvis-orbe-particulas" data-estado={estado}
      style={{ width: tamanho, height: tamanho }} aria-hidden />
  );
}
