/**
 * Economia de energia (2026-10-19).
 *
 * O macOS acusava o app de "usar muita energia" parado: animações CSS infinitas (brilho da nav,
 * reflexo do botão enviar, pulsos) e os fundos WebGL repintam a 60 quadros por segundo mesmo com
 * a janela atrás de outra ou ninguém mexendo. Aqui mora o estado "pausa": janela oculta, sem foco
 * ou parada há {@link INATIVIDADE_MS} sem mouse/teclado. Com ele ligado:
 *
 * - `html[data-energia="pausa"]` congela as animações decorativas (`src/styles/energia.css`);
 * - os fundos WebGL param de pedir quadro ({@link proximoQuadro}) e voltam ao retomar.
 *
 * Nada some da tela: congelar mantém o último quadro. Quem liga é `useEconomiaDeEnergia` no App.
 */

/** Sem mouse/teclado/rolagem por isso, a janela em foco também conta como parada. Curto de
 *  propósito: o Enzo fica com a janela em foco lendo a Conversa, e 60 s deixava tudo animando. */
export const INATIVIDADE_MS = 8_000;

/** Fundos WebGL: no máximo ~30 quadros por segundo (o display link do macOS é 60/120 Hz). */
const INTERVALO_MINIMO_MS = 33;

let pausado = false;
const retomadas = new Map<number, FrameRequestCallback>();
let proximoId = -1;

export function energiaPausada() {
  return pausado;
}

export function definirPausaDeEnergia(valor: boolean) {
  if (valor === pausado) return;
  pausado = valor;
  if (typeof document !== "undefined") {
    if (valor) document.documentElement.dataset.energia = "pausa";
    else delete document.documentElement.dataset.energia;
  }
  if (!valor) {
    // Mesmo id de antes: o efeito guardou esse número e é com ele que vai cancelar ao desmontar.
    const pendentes = [...retomadas.entries()];
    retomadas.clear();
    for (const [id, cb] of pendentes) pedirQuadro(id, cb);
  }
}

/**
 * `requestAnimationFrame` que respeita a pausa e limita a ~30 fps: pausado, guarda o pedido e só o
 * atende ao retomar; senão espera o que faltar dos 33 ms desde o último quadro atendido (com
 * `setTimeout`, para nem pedir quadro ao display link antes da hora). Devolve um id para
 * {@link cancelarQuadro} — sempre diferente de 0, que os efeitos usam como "parado".
 */
const pedidos = new Map<number, { timer?: number; raf?: number }>();
let ultimoQuadro = 0;
let proximoPedido = 1;

export function proximoQuadro(cb: FrameRequestCallback): number {
  if (pausado) {
    const id = proximoId--;
    retomadas.set(id, cb);
    return id;
  }
  const id = proximoPedido++;
  pedirQuadro(id, cb);
  return id;
}

function pedirQuadro(id: number, cb: FrameRequestCallback) {
  const pedido: { timer?: number; raf?: number } = {};
  pedidos.set(id, pedido);
  const pedir = () => {
    pedido.timer = undefined;
    pedido.raf = requestAnimationFrame((t) => {
      pedidos.delete(id);
      ultimoQuadro = t;
      cb(t);
    });
  };
  const falta = INTERVALO_MINIMO_MS - (performance.now() - ultimoQuadro);
  if (falta > 4) pedido.timer = window.setTimeout(pedir, falta);
  else pedir();
}

export function cancelarQuadro(id: number) {
  if (retomadas.delete(id)) return;
  const pedido = pedidos.get(id);
  if (!pedido) return;
  pedidos.delete(id);
  if (pedido.timer !== undefined) window.clearTimeout(pedido.timer);
  if (pedido.raf !== undefined) cancelAnimationFrame(pedido.raf);
}
