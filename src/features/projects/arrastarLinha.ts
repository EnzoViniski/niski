import type { PointerEvent as PointerEventReact } from "react";
import { targetAt, type TabDropTarget } from "../workspace/tabPointerDrag";

/**
 * Fork: arrastar uma linha da lateral (projeto ou conversa).
 *
 * - Uma cópia da linha acompanha o mouse, e a lista se reorganiza ao vivo (`aoMudar`) para mostrar
 *   como vai ficar; o lugar de origem vira um espaço vazio.
 * - Com `layout`, a linha pode sair da lateral e cair num painel do layout (as mesmas zonas das abas:
 *   centro abre ali, bordas dividem o painel).
 *
 * Eventos de ponteiro, não HTML5 drag-and-drop: com o drop de arquivos do Tauri ligado o webview
 * pode não entregar `dragstart`/`drop` (o mesmo motivo de `tabPointerDrag.ts`). Só vira arraste
 * depois de alguns pixels — um clique continua sendo clique — e, se arrastou, o clique que o
 * navegador dispara ao soltar é engolido. Esc cancela.
 */
const LIMIAR_PX = 5;
/** Folga dos lados da lista em que o arraste ainda conta como "reordenar". */
const FOLGA_X_PX = 24;

export interface Arraste {
  /** Posição prevista entre as **outras** linhas (0 = primeira, `n` = no fim); `null` = sem prévia. */
  aoMudar: (posicao: number | null) => void;
  aoSoltar: (posicao: number) => void;
  /** O arraste acabou (soltou ou cancelou), antes de `aoSoltar`. */
  aoTerminar?: () => void;
  /** Soltar num painel do layout (só conversas). */
  layout?: {
    aoMudar: (alvo: TabDropTarget | null) => void;
    aoSoltar: (alvo: TabDropTarget) => void;
  };
}

function linhasDe(lista: HTMLElement): HTMLElement[] {
  return Array.from(lista.children).filter((filho): filho is HTMLElement => filho instanceof HTMLElement && "reordenar" in filho.dataset);
}

/** Entre as linhas de `lista` (menos `ignorar`), antes de qual o ponto `y` cai. Usa o meio do primeiro
 *  filho de cada linha (o cabeçalho da pasta, não a pasta aberta inteira). */
export function posicaoNaLista(lista: HTMLElement, y: number, ignorar?: HTMLElement | null): number {
  const linhas = linhasDe(lista).filter((linha) => linha !== ignorar);
  const i = linhas.findIndex((linha) => {
    const ref = (linha.firstElementChild as HTMLElement | null) ?? linha;
    const caixa = ref.getBoundingClientRect();
    return y < caixa.top + caixa.height / 2;
  });
  return i < 0 ? linhas.length : i;
}

/** A cópia que segue o mouse: a própria linha clonada, fora do fluxo e sem receber eventos. */
function criarFantasma(linha: HTMLElement, caixa: DOMRect): HTMLElement {
  const fantasma = linha.cloneNode(true) as HTMLElement;
  fantasma.removeAttribute("data-reordenar");
  fantasma.setAttribute("aria-hidden", "true");
  fantasma.classList.remove("is-arrastando");
  fantasma.classList.add("lateral-fantasma");
  Object.assign(fantasma.style, { position: "fixed", left: `${caixa.left}px`, top: `${caixa.top}px`, width: `${caixa.width}px`, margin: "0" });
  document.body.appendChild(fantasma);
  return fantasma;
}

export function iniciarArraste(evento: PointerEventReact, lista: HTMLElement | null, arraste: Arraste) {
  if (evento.button !== 0 || !lista) return;
  // O alvo pode ser um botão de ação da linha (fechar, reiniciar): esses não arrastam.
  if ((evento.target as HTMLElement).closest("[data-nao-arrasta]")) return;
  const linha = (evento.currentTarget as HTMLElement).closest<HTMLElement>("[data-reordenar]");
  if (!linha) return;
  const { clientX: inicioX, clientY: inicioY, pointerId } = evento;
  let arrastando = false;
  let fantasma: HTMLElement | null = null;
  let deslocamento = { x: 0, y: 0 };
  let posicao: number | null = null;
  let alvo: TabDropTarget | null = null;
  // O primeiro aviso sai sempre: é ele que marca a linha como "sendo arrastada", mesmo já fora da lista.
  let avisou = false;

  const naLista = (x: number) => {
    const caixa = lista.getBoundingClientRect();
    return x >= caixa.left - FOLGA_X_PX && x <= caixa.right + FOLGA_X_PX;
  };

  const mover = (e: PointerEvent) => {
    if (e.pointerId !== undefined && pointerId !== undefined && e.pointerId !== pointerId) return;
    if (!arrastando) {
      if (Math.hypot(e.clientX - inicioX, e.clientY - inicioY) < LIMIAR_PX) return;
      arrastando = true;
      const caixa = linha.getBoundingClientRect();
      deslocamento = { x: inicioX - caixa.left, y: inicioY - caixa.top };
      fantasma = criarFantasma(linha, caixa);
      document.body.classList.add("lateral-arrastando");
    }
    e.preventDefault();
    fantasma!.style.left = `${e.clientX - deslocamento.x}px`;
    fantasma!.style.top = `${e.clientY - deslocamento.y}px`;

    const dentro = naLista(e.clientX);
    const novaPosicao = dentro ? posicaoNaLista(lista, e.clientY, linha) : null;
    const novoAlvo = !dentro && arraste.layout ? targetAt(e.clientX, e.clientY) : null;
    fantasma!.classList.toggle("is-sobre-layout", Boolean(novoAlvo));
    // Só avisa quando muda: cada aviso re-renderiza a lista (e, no layout, os painéis).
    if (!avisou || novaPosicao !== posicao) { avisou = true; posicao = novaPosicao; arraste.aoMudar(posicao); }
    if (novoAlvo?.paneId !== alvo?.paneId || novoAlvo?.zone !== alvo?.zone) { alvo = novoAlvo; arraste.layout?.aoMudar(alvo); }
  };

  const terminar = (soltou: boolean) => {
    window.removeEventListener("pointermove", mover);
    window.removeEventListener("pointerup", soltar);
    window.removeEventListener("pointercancel", cancelar);
    window.removeEventListener("keydown", tecla, true);
    window.removeEventListener("blur", cancelar);
    document.body.classList.remove("lateral-arrastando");
    fantasma?.remove();
    if (!arrastando) return;
    arraste.aoTerminar?.();
    if (alvo) arraste.layout?.aoMudar(null);
    if (soltou && posicao !== null) arraste.aoSoltar(posicao);
    else if (soltou && alvo) arraste.layout?.aoSoltar(alvo);
    // O `click` que vem logo depois do `pointerup` abriria a conversa/pasta: engole só esse.
    const engolir = (clique: MouseEvent) => { clique.preventDefault(); clique.stopPropagation(); };
    window.addEventListener("click", engolir, { capture: true, once: true });
    window.setTimeout(() => window.removeEventListener("click", engolir, { capture: true }), 0);
  };
  const soltar = () => terminar(true);
  const cancelar = () => terminar(false);
  const tecla = (e: KeyboardEvent) => { if (e.key === "Escape") { e.preventDefault(); terminar(false); } };

  window.addEventListener("pointermove", mover, { passive: false });
  window.addEventListener("pointerup", soltar);
  window.addEventListener("pointercancel", cancelar);
  window.addEventListener("keydown", tecla, true);
  window.addEventListener("blur", cancelar);
}
