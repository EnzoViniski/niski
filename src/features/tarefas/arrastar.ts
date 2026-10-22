/**
 * Fork (Tarefas): arrastar cartões com o ponteiro (não com o arrastar nativo do HTML, que no WebKit
 * do app depende do tratamento de arquivos soltos na janela). Uma cópia do cartão segue o mouse,
 * a zona embaixo (`data-soltar` + `data-valor`) ganha a marca de onde vai cair e a lista rola
 * sozinha perto das bordas. Esc cancela. Um clique sem mover continua abrindo o cartão.
 */
import { useCallback, useEffect, useRef, useState, type PointerEvent as PointerReact } from "react";

export interface Alvo { tipo: string; valor: string; indice: number }

const LIMIAR = 5;
const BORDA = 48;

/** Índice de soltar dentro da zona: antes do primeiro cartão cujo meio está abaixo do ponteiro. */
export function indiceNaZona(zona: Element, y: number, ignorar: string): number {
  const cartoes = [...zona.querySelectorAll<HTMLElement>("[data-tarefa]")].filter((c) => c.dataset.tarefa !== ignorar);
  const i = cartoes.findIndex((c) => { const r = c.getBoundingClientRect(); return y < r.top + r.height / 2; });
  return i < 0 ? cartoes.length : i;
}

function rolarPerto(el: Element | null, x: number, y: number) {
  for (let atual = el; atual; atual = atual.parentElement) {
    const h = atual as HTMLElement;
    const estilo = getComputedStyle(h);
    const r = h.getBoundingClientRect();
    if (/(auto|scroll)/.test(estilo.overflowY) && h.scrollHeight > h.clientHeight) {
      if (y < r.top + BORDA) h.scrollTop -= 14; else if (y > r.bottom - BORDA) h.scrollTop += 14;
    }
    if (/(auto|scroll)/.test(estilo.overflowX) && h.scrollWidth > h.clientWidth) {
      if (x < r.left + BORDA) h.scrollLeft -= 18; else if (x > r.right - BORDA) h.scrollLeft += 18;
    }
  }
}

export function useArrastoDeCartoes(onSoltar: (id: string, alvo: Alvo) => void) {
  const [arrastando, setArrastando] = useState<string | null>(null);
  const [alvo, setAlvo] = useState<Alvo | null>(null);
  const estado = useRef<{ id: string; x0: number; y0: number; dx: number; dy: number; cartao: HTMLElement; copia: HTMLElement | null; alvo: Alvo | null } | null>(null);
  const engolirClique = useRef(false);
  const soltar = useRef(onSoltar);
  soltar.current = onSoltar;

  const limpar = useCallback(() => {
    estado.current?.copia?.remove();
    estado.current = null;
    document.body.classList.remove("tarefa-arrastando");
    setArrastando(null);
    setAlvo(null);
  }, []);

  useEffect(() => {
    const mover = (e: PointerEvent) => {
      const s = estado.current;
      if (!s) return;
      if (!s.copia) {
        if (Math.hypot(e.clientX - s.x0, e.clientY - s.y0) < LIMIAR) return;
        const r = s.cartao.getBoundingClientRect();
        const copia = s.cartao.cloneNode(true) as HTMLElement;
        copia.classList.add("tarefa-cartao-fantasma");
        Object.assign(copia.style, { width: `${r.width}px`, left: "0px", top: "0px" });
        s.dx = s.x0 - r.left; s.dy = s.y0 - r.top;
        document.body.appendChild(copia);
        document.body.classList.add("tarefa-arrastando");
        s.copia = copia;
        setArrastando(s.id);
      }
      s.copia.style.transform = `translate(${e.clientX - s.dx}px, ${e.clientY - s.dy}px) rotate(2deg)`;
      const embaixo = document.elementFromPoint(e.clientX, e.clientY);
      const zona = embaixo?.closest<HTMLElement>("[data-soltar]");
      const novo = zona ? { tipo: zona.dataset.soltar ?? "", valor: zona.dataset.valor ?? "", indice: indiceNaZona(zona, e.clientY, s.id) } : null;
      if (novo?.tipo !== s.alvo?.tipo || novo?.valor !== s.alvo?.valor || novo?.indice !== s.alvo?.indice) { s.alvo = novo; setAlvo(novo); }
      rolarPerto(embaixo, e.clientX, e.clientY);
    };
    const subir = () => {
      const s = estado.current;
      if (!s) return;
      if (s.copia) {
        engolirClique.current = true;
        setTimeout(() => { engolirClique.current = false; }, 0);
        if (s.alvo) soltar.current(s.id, s.alvo);
      }
      limpar();
    };
    const tecla = (e: KeyboardEvent) => { if (e.key === "Escape" && estado.current?.copia) { e.stopPropagation(); limpar(); } };
    window.addEventListener("pointermove", mover);
    window.addEventListener("pointerup", subir);
    window.addEventListener("pointercancel", limpar);
    window.addEventListener("keydown", tecla, true);
    return () => {
      window.removeEventListener("pointermove", mover);
      window.removeEventListener("pointerup", subir);
      window.removeEventListener("pointercancel", limpar);
      window.removeEventListener("keydown", tecla, true);
      estado.current?.copia?.remove();
    };
  }, [limpar]);

  /** Liga no `onPointerDown` do cartão. Botões e campos dentro do cartão não começam arrasto. */
  const iniciar = useCallback((e: PointerReact<HTMLElement>, id: string) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button, input, textarea, a, select")) return;
    estado.current = { id, x0: e.clientX, y0: e.clientY, dx: 0, dy: 0, cartao: e.currentTarget, copia: null, alvo: null };
  }, []);

  /** O clique que vem logo depois de soltar não abre o cartão. */
  const cliqueValido = useCallback(() => !engolirClique.current, []);

  return { arrastando, alvo, iniciar, cliqueValido };
}
