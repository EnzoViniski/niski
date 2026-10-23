import { engineApi } from "./terminalService";

/**
 * Fork (harness, A4): a vez de trocar a conta de uma conversa no limite. O engine também troca a conta de
 * sessão sem painel aberto (`mobile/troca_no_limite.rs`); o painel pede a reserva antes de trocar, e quem
 * não consegue a reserva não troca. Assim os dois nunca reabrem a mesma conversa duas vezes.
 *
 * - `true`: a vez é deste painel (ou o engine é antigo e não tem a rota: segue como antes);
 * - `false`: o engine (ou outro painel) já está trocando esta conversa.
 */
export async function reservarTrocaDeConta(conversationId: string, dono: string): Promise<boolean> {
  try {
    const resposta = await engineApi("POST", caminho(conversationId, "reservar"), JSON.stringify({ dono }));
    return resposta.status !== 409;
  } catch {
    // Engine fora do ar ou sem a rota: a troca do painel continua como sempre foi.
    return true;
  }
}

export async function soltarTrocaDeConta(conversationId: string, dono: string): Promise<void> {
  try {
    await engineApi("POST", caminho(conversationId, "soltar"), JSON.stringify({ dono }));
  } catch {
    // A reserva vence sozinha no engine (2 min); soltar é só para liberar antes.
  }
}

/** Um id por painel montado, para a reserva ter dono. */
export function donoDoPainel(): string {
  return `painel-${Math.random().toString(36).slice(2, 10)}`;
}

function caminho(conversationId: string, acao: "reservar" | "soltar") {
  return `/conversas/${encodeURIComponent(conversationId)}/troca-de-conta/${acao}`;
}
