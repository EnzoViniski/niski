// Fork (Rotina): pedidos para abrir a tela Rotina de qualquer lugar (paleta, aviso, lateral) sem
// passar callbacks pela árvore. O App escuta e troca a tela; a tela escuta e abre a conversa.
export type PedidoDaRotina = { aba?: "hoje" | "semana" | "treino" | "alimentacao" | "ajustes"; conversa?: "planejar" | "conversar" };

const EVENTO = "medza-abrir-rotina";

export function pedirAbrirRotina(pedido: PedidoDaRotina = {}) {
  window.dispatchEvent(new CustomEvent<PedidoDaRotina>(EVENTO, { detail: pedido }));
}

export function ouvirPedidosDaRotina(ouvinte: (pedido: PedidoDaRotina) => void): () => void {
  const f = (e: Event) => ouvinte((e as CustomEvent<PedidoDaRotina>).detail ?? {});
  window.addEventListener(EVENTO, f);
  return () => window.removeEventListener(EVENTO, f);
}

/** Pedido que chegou antes de a tela montar (ela pega ao abrir). */
let pendente: PedidoDaRotina | null = null;
export function guardarPedidoPendente(pedido: PedidoDaRotina | null) { pendente = pedido; }
export function tirarPedidoPendente(): PedidoDaRotina | null { const p = pendente; pendente = null; return p; }
