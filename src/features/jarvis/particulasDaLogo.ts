// Orbe de partículas. Transforma o símbolo do Niski em partículas: a silhueta vem da versão
// preta chapada (`niski-simbolo-preto.png`, alfa), a cor de cada partícula vem do mesmo ponto da
// versão com brilho (`niski-simbolo.png`). Mais partículas no contorno deixam a forma nítida mesmo
// tremendo; uma poeira em volta reage à voz. Amostragem pura e determinística (semente fixa),
// a leitura das imagens fica em `carregarPixelsDaLogo`.
import simbolo from "../../assets/marca/niski-simbolo.png";
import simboloPreto from "../../assets/marca/niski-simbolo-preto.png";

/** Floats por partícula no buffer do WebGL: casa (x, y), cor (r, g, b), semente, tipo, brilho. */
export const FLOATS_POR_PARTICULA = 8;
export const TIPO = { miolo: 0, contorno: 1, poeira: 2, halo: 3 } as const;

export interface Pixels { largura: number; altura: number; dados: Uint8ClampedArray }

/** PRNG pequeno (mulberry32): mesma logo, mesmas partículas, em toda abertura. */
function sorteador(semente: number) {
  let a = semente >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Sorteia `quantidade` partículas. Casas normalizadas em [-1, 1] (y para cima), centradas na caixa
 * da silhueta e com a proporção preservada. Sem silhueta (imagem vazia) devolve um buffer vazio.
 */
export function amostrarLogo(mascara: Pixels, cor: Pixels, quantidade: number, semente = 7): Float32Array {
  const { largura: L, altura: A, dados: m } = mascara;
  const dentro = (x: number, y: number) => x >= 0 && y >= 0 && x < L && y < A && m[(y * L + x) * 4 + 3] > 127;
  const miolo: number[] = [];
  const contorno: number[] = [];
  let minX = L, minY = A, maxX = -1, maxY = -1;
  for (let y = 0; y < A; y++) {
    for (let x = 0; x < L; x++) {
      if (!dentro(x, y)) continue;
      const borda = !dentro(x - 1, y) || !dentro(x + 1, y) || !dentro(x, y - 1) || !dentro(x, y + 1);
      (borda ? contorno : miolo).push(y * L + x);
      if (x < minX) minX = x; if (x > maxX) maxX = x;
      if (y < minY) minY = y; if (y > maxY) maxY = y;
    }
  }
  if (maxX < 0) return new Float32Array(0);
  if (!miolo.length) miolo.push(...contorno);

  const cx = (minX + maxX + 1) / 2;
  const cy = (minY + maxY + 1) / 2;
  const meio = Math.max(maxX - minX + 1, maxY - minY + 1) / 2;
  const sortear = sorteador(semente);
  const saida = new Float32Array(quantidade * FLOATS_POR_PARTICULA);
  const nPoeira = Math.round(quantidade * 0.09);
  // Halo: poucas partículas grandes e fracas sobre o miolo, um "bloom" sem passe extra de desenho.
  const nHalo = Math.round(quantidade * 0.035);
  const nContorno = Math.round((quantidade - nPoeira - nHalo) * 0.3);

  // Cor do ponto (x, y) na imagem com brilho, na escala da máscara. Sem cor ali (alfa ~0), um
  // degradê azul→violeta na diagonal, que é a paleta do símbolo.
  const corEm = (x: number, y: number): [number, number, number, number] => {
    const px = Math.min(cor.largura - 1, Math.max(0, Math.floor((x / L) * cor.largura)));
    const py = Math.min(cor.altura - 1, Math.max(0, Math.floor((y / A) * cor.altura)));
    const i = (py * cor.largura + px) * 4;
    const alfa = cor.dados[i + 3] / 255;
    const t = Math.min(1, Math.max(0, (x / L + y / A) / 2));
    let r = 0.35 + 0.35 * t, g = 0.55 - 0.25 * t, b = 1;
    if (alfa > 0.08) {
      r = cor.dados[i] / 255; g = cor.dados[i + 1] / 255; b = cor.dados[i + 2] / 255;
    }
    // Clareia o tom (a cor da imagem com brilho é escura onde o alfa é baixo).
    const maior = Math.max(r, g, b, 0.001);
    const ganho = Math.min(1 / maior, 2.2);
    return [r * ganho, g * ganho, b * ganho, alfa];
  };

  let n = 0;
  const por = (tipo: number, lista: number[], total: number) => {
    for (let k = 0; k < total; k++, n++) {
      const idx = lista[Math.floor(sortear() * lista.length)];
      const x = (idx % L) + sortear();
      const y = Math.floor(idx / L) + sortear();
      const [r, g, b, alfa] = corEm(x, y);
      const o = n * FLOATS_POR_PARTICULA;
      saida[o] = (x - cx) / meio;
      saida[o + 1] = -(y - cy) / meio;
      saida[o + 2] = r; saida[o + 3] = g; saida[o + 4] = b;
      saida[o + 5] = sortear();
      saida[o + 6] = tipo;
      saida[o + 7] = tipo === TIPO.contorno ? 0.95 : tipo === TIPO.halo ? 0.09 : 0.5 + 0.45 * alfa;
    }
  };
  por(TIPO.contorno, contorno, nContorno);
  por(TIPO.miolo, miolo, quantidade - nPoeira - nHalo - nContorno);
  por(TIPO.halo, miolo, nHalo);
  for (let k = 0; k < nPoeira; k++, n++) {
    // Poeira num anel em volta do símbolo, com a cor de um ponto qualquer do contorno.
    const ang = sortear() * Math.PI * 2;
    const raio = 0.82 + sortear() * 0.36;
    const ref = contorno[Math.floor(sortear() * contorno.length)];
    const [r, g, b] = corEm(ref % L, Math.floor(ref / L));
    const o = n * FLOATS_POR_PARTICULA;
    saida[o] = Math.cos(ang) * raio;
    saida[o + 1] = Math.sin(ang) * raio;
    saida[o + 2] = r; saida[o + 3] = g; saida[o + 4] = b;
    saida[o + 5] = sortear();
    saida[o + 6] = TIPO.poeira;
    saida[o + 7] = 0.6;
  }
  return saida;
}

/** Lado em que as duas imagens são lidas: detalhe de sobra para ~3 mil partículas, leitura rápida. */
const LADO_DA_AMOSTRA = 200;
let pixelsDaLogo: Promise<{ mascara: Pixels; cor: Pixels }> | null = null;

function lerImagem(url: string): Promise<Pixels> {
  return new Promise((resolver, rejeitar) => {
    const img = new Image();
    img.onload = () => {
      const tela = document.createElement("canvas");
      tela.width = LADO_DA_AMOSTRA; tela.height = LADO_DA_AMOSTRA;
      const ctx = tela.getContext("2d", { willReadFrequently: true });
      if (!ctx) { rejeitar(new Error("canvas 2d indisponível")); return; }
      ctx.drawImage(img, 0, 0, LADO_DA_AMOSTRA, LADO_DA_AMOSTRA);
      resolver({ largura: LADO_DA_AMOSTRA, altura: LADO_DA_AMOSTRA, dados: ctx.getImageData(0, 0, LADO_DA_AMOSTRA, LADO_DA_AMOSTRA).data });
    };
    img.onerror = () => rejeitar(new Error("logo não carregou"));
    img.src = url;
  });
}

/** Lê as duas versões do símbolo uma vez por janela (a bolinha e o painel reaproveitam). */
export function carregarPixelsDaLogo(): Promise<{ mascara: Pixels; cor: Pixels }> {
  pixelsDaLogo ??= Promise.all([lerImagem(simboloPreto), lerImagem(simbolo)])
    .then(([mascara, cor]) => ({ mascara, cor }))
    .catch((erro: unknown) => { pixelsDaLogo = null; throw erro; });
  return pixelsDaLogo;
}
