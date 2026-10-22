// Fork (revisão do copiloto): os botões flutuantes do canto (Tarefas + logo do copiloto) desviam do
// que não podem cobrir. Com a posição padrão, eles caíam em cima do campo de mensagem e do Enviar
// das abas de agente e da coluna de conversa do editor de vídeo. Regra: campo de mensagem embaixo →
// o lançador sobe para cima dele; coluna alta (mais da metade da janela) → vai para a esquerda dela.
// A posição escolhida no arrasto continua sendo a base; o desvio é calculado por cima dela.

/** O que o lançador não pode cobrir. */
export const OBSTACULOS_DO_LANCADOR = ".video-chat-normal, .composer, .orq-composer";

export interface Caixa { left: number; top: number; right: number; bottom: number }
/** Distâncias até a borda direita e a de baixo (como no `style` do lançador). */
export interface Recuo { right: number; bottom: number }

const FOLGA = 8;

function cruza(a: Caixa, b: Caixa): boolean {
  return a.left < b.right + FOLGA && a.right > b.left - FOLGA && a.top < b.bottom + FOLGA && a.bottom > b.top - FOLGA;
}

/**
 * Recuo final do lançador (`largura` × `altura`) a partir do `base`, desviando dos `obstaculos`
 * (caixas na tela). Fica sempre dentro da janela; se não houver lugar livre, devolve o melhor que deu.
 */
export function desviarLancador(base: Recuo, largura: number, altura: number, obstaculos: Caixa[], janela: { w: number; h: number }): Recuo {
  const limite = (r: Recuo): Recuo => ({
    right: Math.min(Math.max(FOLGA, r.right), Math.max(FOLGA, janela.w - largura - FOLGA)),
    bottom: Math.min(Math.max(FOLGA, r.bottom), Math.max(FOLGA, janela.h - altura - FOLGA)),
  });
  // Coluna alta primeiro: desviar dela já tira o lançador de cima do composer que mora dentro.
  const ordem = obstaculos.filter((x) => x.right - x.left > 0 && x.bottom - x.top > 0)
    .sort((a, b) => (b.bottom - b.top) - (a.bottom - a.top));
  let r = limite(base);
  // Poucas voltas: subir acima de um composer pode cair em outro (layout dividido).
  for (let volta = 0; volta < 4; volta++) {
    const caixa = { left: janela.w - r.right - largura, right: janela.w - r.right, top: janela.h - r.bottom - altura, bottom: janela.h - r.bottom };
    const o = ordem.find((x) => cruza(caixa, x));
    if (!o) break;
    const alta = o.bottom - o.top > janela.h / 2;
    const proximo = limite(alta ? { ...r, right: janela.w - o.left + FOLGA } : { ...r, bottom: janela.h - o.top + FOLGA });
    if (proximo.right === r.right && proximo.bottom === r.bottom) break;
    r = proximo;
  }
  return r;
}
