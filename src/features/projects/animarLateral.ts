// Fork: movimento suave na lateral. Antes, reordenar (arrastar ou ⌥↑/⌥↓), abrir uma pasta ou uma
// conversa nova entrar fazia as linhas pularem de lugar. Aqui cada linha marcada com `data-anima`
// desliza da posição antiga para a nova (técnica FLIP, com a Web Animations API) e linha nova entra
// com um fade curto. Respeita "Animações: desligadas" e o reduzir movimento do sistema (`motionOn`).
import { useLayoutEffect, useRef, type RefObject } from "react";
import { motionOn } from "../../lib/motion";

export const DURACAO_MS = 220;
export const CURVA = "cubic-bezier(.2, .8, .2, 1)";

interface Posicao { topo: number }

/** Topo da linha em relação ao "dono" dela: a linha `data-anima` de fora (conversa dentro da pasta)
 *  ou a própria lista. Relativo de propósito: rolar a lateral ou mover a pasta inteira não pode fazer
 *  cada conversa animar sozinha (seria a mesma distância contada duas vezes). */
function topoRelativo(linha: HTMLElement, raiz: HTMLElement): number {
  const dono = linha.parentElement?.closest<HTMLElement>("[data-anima]");
  const referencia = dono && raiz.contains(dono) ? dono : raiz;
  return linha.getBoundingClientRect().top - referencia.getBoundingClientRect().top;
}

/** Calcula o que animar, sem tocar no DOM (testável). `null` = linha nova. */
export function deslocamentos(antes: Map<string, Posicao>, depois: Map<string, Posicao>): Map<string, number | null> {
  const resultado = new Map<string, number | null>();
  for (const [chave, agora] of depois) {
    const velha = antes.get(chave);
    if (!velha) { resultado.set(chave, null); continue; }
    const dy = velha.topo - agora.topo;
    if (Math.abs(dy) >= 1) resultado.set(chave, dy);
  }
  return resultado;
}

export function useAnimarLista(ref: RefObject<HTMLElement | null>) {
  const antes = useRef<Map<string, Posicao> | null>(null);
  // Gaveta de pasta abrindo/fechando (transição de CSS): as linhas de baixo já estão deslizando; medir
  // no meio dela e animar de novo daria um tranco. Enquanto houver uma correndo, só atualiza as medidas.
  const gavetas = useRef(0);
  useLayoutEffect(() => {
    const raiz = ref.current;
    if (!raiz) return;
    const ehGaveta = (evento: Event) => (evento.target as HTMLElement | null)?.classList?.contains("pasta-gaveta");
    const comecou = (evento: Event) => { if (ehGaveta(evento)) gavetas.current += 1; };
    const acabou = (evento: Event) => { if (ehGaveta(evento)) gavetas.current = Math.max(0, gavetas.current - 1); };
    raiz.addEventListener("transitionrun", comecou);
    raiz.addEventListener("transitionend", acabou);
    raiz.addEventListener("transitioncancel", acabou);
    return () => {
      raiz.removeEventListener("transitionrun", comecou);
      raiz.removeEventListener("transitionend", acabou);
      raiz.removeEventListener("transitioncancel", acabou);
    };
  }, [ref]);
  useLayoutEffect(() => {
    const raiz = ref.current;
    if (!raiz) return;
    const linhas = Array.from(raiz.querySelectorAll<HTMLElement>("[data-anima]"));
    // A posição de agora precisa ser a do layout, não a de uma animação ainda correndo.
    for (const linha of linhas) linha.getAnimations?.().forEach((animacao) => animacao.cancel());
    const depois = new Map<string, Posicao>();
    for (const linha of linhas) depois.set(linha.dataset.anima!, { topo: topoRelativo(linha, raiz) });
    const anterior = antes.current;
    antes.current = depois;
    // Primeira medida (a lateral acabou de montar) não anima: seria tudo "entrando" de uma vez.
    if (!anterior || gavetas.current > 0 || !motionOn() || typeof raiz.animate !== "function") return;
    const mudancas = deslocamentos(anterior, depois);
    for (const linha of linhas) {
      const chave = linha.dataset.anima!;
      if (!mudancas.has(chave)) continue;
      const dy = mudancas.get(chave);
      if (dy === null) {
        linha.animate([{ opacity: 0, transform: "translateY(-4px)" }, { opacity: 1, transform: "none" }], { duration: DURACAO_MS, easing: CURVA });
      } else {
        linha.animate([{ transform: `translateY(${dy}px)` }, { transform: "none" }], { duration: DURACAO_MS, easing: CURVA });
      }
    }
  });
}

/** Ao soltar, a cópia que seguia o mouse desliza até o lugar final antes de sumir. */
export function assentarFantasma(fantasma: HTMLElement, destino: HTMLElement, aoTerminar: () => void) {
  if (!motionOn() || typeof fantasma.animate !== "function") { aoTerminar(); return; }
  const de = fantasma.getBoundingClientRect();
  const para = destino.getBoundingClientRect();
  const animacao = fantasma.animate(
    [{ transform: getComputedStyle(fantasma).transform === "none" ? "none" : getComputedStyle(fantasma).transform, opacity: 0.95 },
      { transform: `translate(${para.left - de.left}px, ${para.top - de.top}px)`, opacity: 1 }],
    { duration: 160, easing: CURVA, fill: "forwards" },
  );
  animacao.onfinish = aoTerminar;
  animacao.oncancel = aoTerminar;
}
