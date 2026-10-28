/**
 * Quem está usando cada página de navegador (webview nativo `nav-*`) agora — e o pedido de abrir um
 * link vindo de fora do DOM (o terminal).
 *
 * O Enzo (23/10/2026) abriu um link do Hyperframes, fechou a aba e ficou um quadrado branco por cima do app
 * que não saía. O webview nativo fica **por cima** do HTML: se ninguém o esconde ou fecha, ele fica. O
 * `useLinksEmAba` confere de tempos em tempos (`navegador_conferir`): o que não é de aba nenhuma fecha; o
 * que não está desenhado agora some. Aqui ficam as duas listas:
 * - `desenhadas`: `NavegadorPane` montado (a página está na tela);
 * - `presas`: quem segura a página viva mesmo sem desenhá-la (o estudo do Roadmap com outra aba à frente).
 */

const desenhadas = new Map<string, number>();
const presas = new Map<string, number>();

function somar(mapa: Map<string, number>, rotulo: string, delta: 1 | -1) {
  const total = (mapa.get(rotulo) ?? 0) + delta;
  if (total > 0) mapa.set(rotulo, total); else mapa.delete(rotulo);
}

/** O `NavegadorPane` com este rótulo está montado. Devolve o desfazer. */
export function marcarDesenhada(rotulo: string): () => void {
  somar(desenhadas, rotulo, 1);
  return () => somar(desenhadas, rotulo, -1);
}

/** Mantém a página viva enquanto quem chamou estiver montado. Devolve o desfazer. */
export function prenderPagina(rotulo: string): () => void {
  somar(presas, rotulo, 1);
  return () => somar(presas, rotulo, -1);
}

export function rotulosDesenhados(): string[] { return [...desenhadas.keys()]; }
export function rotulosPresos(): string[] { return [...presas.keys()]; }

/** Evento da janela que o `useLinksEmAba` ouve: abrir um link numa aba de navegador do layout. */
export const EVENTO_ABRIR_LINK = "niski:abrir-link";
export interface PedidoDeLink { url: string; paneId?: string }

/** Abre `url` (http/https) numa aba de navegador ao lado do painel de `origem`. Outros esquemas: nada. */
export function abrirLinkNoApp(url: string, origem?: Element | null): boolean {
  if (!/^https?:\/\//i.test(url)) return false;
  const paneId = origem?.closest<HTMLElement>("[data-drop-pane]")?.dataset.dropPane;
  window.dispatchEvent(new CustomEvent<PedidoDeLink>(EVENTO_ABRIR_LINK, { detail: { url, paneId } }));
  return true;
}
