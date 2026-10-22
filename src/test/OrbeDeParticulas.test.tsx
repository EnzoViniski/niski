// Fork: orbe de partículas do Jarvis — suavização da voz, amostragem da logo e reserva sem WebGL2.
import { render } from "@testing-library/react";
import { createRef } from "react";
import { SuavizadorDeVoz, TAMANHO_DO_HISTORICO, aproximar } from "../features/jarvis/nivelDaVoz";
import { FLOATS_POR_PARTICULA, TIPO, amostrarLogo, type Pixels } from "../features/jarvis/particulasDaLogo";
import { OrbeDeParticulas } from "../features/jarvis/OrbeDeParticulas";
import type { Niveis } from "../features/jarvis/OrbeDoJarvis";

const QUADRO = 1 / 60;

function simular(voz: SuavizadorDeVoz, nivel: { mic: number; fala: number }, segundos: number) {
  for (let t = 0; t < segundos; t += QUADRO) voz.passo(nivel, QUADRO);
}

describe("SuavizadorDeVoz", () => {
  it("ruído de fundo constante no microfone não mexe o orbe", () => {
    const voz = new SuavizadorDeVoz();
    simular(voz, { mic: 0.32, fala: 0 }, 4);
    expect(voz.mic).toBeLessThan(0.05);
  });

  it("fala por cima do ruído abre o microfone, e o silêncio fecha", () => {
    const voz = new SuavizadorDeVoz();
    simular(voz, { mic: 0.3, fala: 0 }, 3);
    simular(voz, { mic: 0.85, fala: 0 }, 0.3);
    expect(voz.mic).toBeGreaterThan(0.8);
    simular(voz, { mic: 0.3, fala: 0 }, 1.5);
    expect(voz.mic).toBeLessThan(0.05);
  });

  it("a voz do Jarvis sobe rápido e desce devagar", () => {
    const voz = new SuavizadorDeVoz();
    simular(voz, { mic: 0, fala: 0.85 }, 0.15);
    expect(voz.fala).toBeGreaterThan(0.9);
    simular(voz, { mic: 0, fala: 0 }, 0.1);
    expect(voz.fala).toBeGreaterThan(0.4);
    simular(voz, { mic: 0, fala: 0 }, 1);
    expect(voz.fala).toBeLessThan(0.01);
  });

  it("o histórico leva a energia do centro para a borda", () => {
    const voz = new SuavizadorDeVoz();
    simular(voz, { mic: 0, fala: 0.9 }, 0.1);
    simular(voz, { mic: 0, fala: 0 }, 0.4);
    const pico = voz.historico.indexOf(Math.max(...voz.historico));
    expect(pico).toBeGreaterThan(8);
    expect(pico).toBeLessThan(TAMANHO_DO_HISTORICO - 4);
    expect(voz.historico[0]).toBeLessThan(0.1);
  });

  it("valor inválido e passo enorme não quebram", () => {
    const voz = new SuavizadorDeVoz();
    voz.passo({ mic: Number.NaN, fala: Number.POSITIVE_INFINITY }, 10);
    expect(Number.isFinite(voz.mic) && Number.isFinite(voz.fala)).toBe(true);
    expect(voz.historico.every(Number.isFinite)).toBe(true);
  });

  it("aproximar chega ao alvo sem passar", () => {
    let v = 0;
    for (let i = 0; i < 120; i++) v = aproximar(v, 1, QUADRO);
    expect(v).toBeGreaterThan(0.99);
    expect(v).toBeLessThanOrEqual(1);
  });
});

/** Quadrado cheio de 20×20 com um furo de 6×6 no meio, numa tela 40×40. */
function quadradoComFuro(): { mascara: Pixels; cor: Pixels } {
  const L = 40;
  const mascara = new Uint8ClampedArray(L * L * 4);
  const cor = new Uint8ClampedArray(L * L * 4);
  for (let y = 0; y < L; y++) for (let x = 0; x < L; x++) {
    const i = (y * L + x) * 4;
    const noQuadrado = x >= 10 && x < 30 && y >= 10 && y < 30;
    const noFuro = x >= 17 && x < 23 && y >= 17 && y < 23;
    if (noQuadrado && !noFuro) mascara[i + 3] = 255;
    cor[i] = 40; cor[i + 1] = 80; cor[i + 2] = 200; cor[i + 3] = 255;
  }
  return { mascara: { largura: L, altura: L, dados: mascara }, cor: { largura: L, altura: L, dados: cor } };
}

describe("amostrarLogo", () => {
  it("põe as partículas da logo dentro da silhueta (fora do furo) e a poeira em volta", () => {
    const { mascara, cor } = quadradoComFuro();
    const p = amostrarLogo(mascara, cor, 1000);
    expect(p.length).toBe(1000 * FLOATS_POR_PARTICULA);
    const tipos = { miolo: 0, contorno: 0, poeira: 0, halo: 0 };
    for (let i = 0; i < 1000; i++) {
      const o = i * FLOATS_POR_PARTICULA;
      const [x, y, r, g, b, semente, tipo] = Array.from(p.subarray(o, o + 7));
      expect(semente).toBeGreaterThanOrEqual(0);
      expect(semente).toBeLessThan(1);
      expect(Math.max(r, g, b)).toBeGreaterThan(0.9);
      if (tipo === TIPO.poeira) { tipos.poeira++; expect(Math.hypot(x, y)).toBeGreaterThan(0.8); continue; }
      tipos[tipo === TIPO.contorno ? "contorno" : tipo === TIPO.halo ? "halo" : "miolo"]++;
      expect(Math.abs(x)).toBeLessThanOrEqual(1);
      expect(Math.abs(y)).toBeLessThanOrEqual(1);
      // Furo: 6/20 do lado, centrado → |x|,|y| < 0,3 ao mesmo tempo é proibido.
      expect(Math.abs(x) < 0.29 && Math.abs(y) < 0.29).toBe(false);
    }
    expect(tipos.contorno).toBeGreaterThan(200);
    expect(tipos.miolo).toBeGreaterThan(500);
    expect(tipos.poeira).toBe(90);
    expect(tipos.halo).toBeGreaterThan(20);
  });

  it("é determinística e devolve vazio sem silhueta", () => {
    const { mascara, cor } = quadradoComFuro();
    expect(amostrarLogo(mascara, cor, 300)).toEqual(amostrarLogo(mascara, cor, 300));
    const vazia = { ...mascara, dados: new Uint8ClampedArray(mascara.dados.length) };
    expect(amostrarLogo(vazia, cor, 300).length).toBe(0);
  });
});

describe("OrbeDeParticulas", () => {
  it("sem WebGL2 (jsdom) cai no orbe 2D sem quebrar", () => {
    const niveis = createRef<Niveis>() as { current: Niveis };
    niveis.current = { mic: 0, fala: 0 };
    const { container } = render(<OrbeDeParticulas estado="ouvindo" niveis={niveis} tamanho={200} />);
    const canvas = container.querySelector("canvas");
    expect(canvas).not.toBeNull();
    expect(canvas?.classList.contains("jarvis-orbe-particulas")).toBe(false);
  });
});
