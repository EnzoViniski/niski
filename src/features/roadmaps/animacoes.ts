/**
 * Fork (Roadmaps): animações curtas com Web Animations, só `opacity`/`transform`, sem `fill` (o
 * elemento volta ao CSS ao terminar) e nunca segurando clique: o enfeite tem `pointer-events: none`.
 * Nada anima com "reduzir movimento", animações desligadas, janela oculta ou pausa de energia.
 * Nada de View Transitions (travou a janela na 0.9.2).
 */
import { energiaPausada } from "../../lib/energia";
import { motionOn } from "../../lib/motion";

function pode(): boolean {
  return motionOn() && !energiaPausada() && typeof document !== "undefined" && !document.hidden;
}

/** Etapa concluída: o nó pulsa, um anel se abre em volta e, com `rotulo`, sobe um "+250 XP". */
export function animarConclusao(no: HTMLElement | null, rotulo?: string): Animation[] {
  if (!no || !pode() || typeof no.animate !== "function") return [];
  const animacoes = [no.animate(
    [{ transform: "scale(1)" }, { transform: "scale(1.18)" }, { transform: "scale(0.96)" }, { transform: "scale(1)" }],
    { duration: 520, easing: "cubic-bezier(.2,.8,.2,1)" },
  )];
  const anel = document.createElement("span");
  anel.className = "roadmap-anel";
  anel.setAttribute("aria-hidden", "true");
  no.appendChild(anel);
  const abrir = anel.animate([{ opacity: 0.7, transform: "scale(0.8)" }, { opacity: 0, transform: "scale(2.1)" }], { duration: 650, easing: "ease-out" });
  const tirar = () => anel.remove();
  abrir.onfinish = tirar;
  abrir.oncancel = tirar;
  animacoes.push(abrir);
  if (rotulo) {
    const texto = document.createElement("span");
    texto.className = "roadmap-ganho";
    texto.setAttribute("aria-hidden", "true");
    texto.textContent = rotulo;
    no.appendChild(texto);
    const subir = texto.animate([{ opacity: 0, transform: "translate(-50%, 4px)" }, { opacity: 1, transform: "translate(-50%, -18px)", offset: 0.3 }, { opacity: 0, transform: "translate(-50%, -40px)" }], { duration: 1100, easing: "ease-out" });
    const tirarTexto = () => texto.remove();
    subir.onfinish = tirarTexto;
    subir.oncancel = tirarTexto;
    animacoes.push(subir);
  }
  return animacoes;
}

/** Entrada leve de um painel (etapa, revisão). */
export function animarEntrada(el: HTMLElement | null): Animation | null {
  if (!el || !pode() || typeof el.animate !== "function") return null;
  return el.animate([{ opacity: 0, transform: "translateY(6px)" }, { opacity: 1, transform: "none" }], { duration: 180, easing: "ease-out" });
}

/** Rola até o nó: suave só quando há movimento. */
export function rolarAte(el: HTMLElement | null) {
  el?.scrollIntoView?.({ block: "center", behavior: pode() ? "smooth" : "auto" });
}
