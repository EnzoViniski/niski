/**
 * Fork (0.9.4): animação ao abrir, trocar e fechar conversas — do jeito seguro.
 *
 * A 0.9.2 usava a View Transitions API: o WebKit fotografa o documento inteiro, suspende a pintura
 * até o callback rodar e, enquanto a transição existe, o documento todo responde por ela. A janela da
 * 0.9.2 chegou a parar de receber cliques. Aqui só o conteúdo que acabou de aparecer anima, com a
 * Web Animations API: `opacity` + `translateY` (rodam no compositor), sem `fill` — terminada ou
 * cancelada, não sobra estilo nenhum. Nada cobre a página e nada segura clique; no pior caso a
 * animação simplesmente não toca.
 *
 * Sem `scale` de propósito: o xterm mede a letra com `getBoundingClientRect` ao montar, e um
 * `scale` nesse momento deixaria o terminal com colunas erradas.
 */
import { useLayoutEffect, type RefObject } from "react";
import { motionOn } from "../../lib/motion";

// Fork (Vidro, 21/10/2026): 8 px e 300 ms com a curva de "assentar" no lugar dos 10 px/220 ms, que o
// Enzo achou duros. A opacidade chega antes do deslocamento (offset .6) para não parecer que pisca.
const COMPLETA: Keyframe[] = [
  { opacity: 0, transform: "translateY(8px)" },
  { opacity: 1, offset: 0.6 },
  { opacity: 1, transform: "translateY(0)" },
];
const REDUZIDA: Keyframe[] = [{ opacity: 0 }, { opacity: 1 }];
/** Sai rápido e assenta devagar, sem passar do ponto (a mesma `--v-curva` do `vidro.css`). */
const CURVA = "cubic-bezier(.22, 1, .36, 1)";

export function animarEntrada(elemento: HTMLElement | null): Animation | null {
  if (!elemento || typeof elemento.animate !== "function" || !motionOn() || document.hidden) return null;
  const reduzida = document.documentElement.dataset.motion === "reduzidas";
  return elemento.animate(reduzida ? REDUZIDA : COMPLETA, { duration: reduzida ? 160 : 300, easing: CURVA });
}

/** Anima `ref` quando `chave` muda (outra aba/conversa no painel) e quando o painel nasce. */
export function useAnimarAoTrocar(ref: RefObject<HTMLElement | null>, chave: string) {
  useLayoutEffect(() => {
    const animacao = animarEntrada(ref.current);
    return () => animacao?.cancel();
  }, [ref, chave]);
}
