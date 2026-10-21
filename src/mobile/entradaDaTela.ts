/**
 * Fork (Vidro, 21/10/2026): ao trocar de tela no celular (início → projeto → conversa…) o conteúdo
 * novo entra com um fade curto e 6 px de deslocamento, em vez de trocar seco.
 *
 * Mesmo cuidado do `features/workspace/animarConversa.ts` do PC, sem importar o GSAP no celular:
 * Web Animations só com `opacity`/`transform`, sem `fill` (acabou ou foi cancelada, não sobra estilo),
 * e nada anima com "reduzir movimento" ligado no sistema ou com a página em segundo plano.
 */
import { useLayoutEffect, useRef, type RefObject } from "react";

const QUADROS: Keyframe[] = [
  { opacity: 0, transform: "translateY(6px)" },
  { opacity: 1, offset: 0.6 },
  { opacity: 1, transform: "translateY(0)" },
];

export function animarTela(elemento: HTMLElement | null): Animation | null {
  if (!elemento || typeof elemento.animate !== "function" || document.hidden) return null;
  if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) return null;
  return elemento.animate(QUADROS, { duration: 280, easing: "cubic-bezier(.22, 1, .36, 1)" });
}

/** Anima `ref` quando `chave` muda. A primeira tela (abrir o app) não anima: já vem do splash. */
export function useEntradaDaTela(ref: RefObject<HTMLElement | null>, chave: string) {
  const primeira = useRef(true);
  useLayoutEffect(() => {
    if (primeira.current) { primeira.current = false; return; }
    const animacao = animarTela(ref.current);
    return () => animacao?.cancel();
  }, [ref, chave]);
}
