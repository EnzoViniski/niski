/**
 * Fork (desempenho, 2026-10-22): um `git status` por pasta para todo mundo que o relê sozinho.
 *
 * O rodapé, o painel Git e a faixa de **cada** aba de agente aberta liam o status da mesma pasta a
 * cada 3 s, cada um por conta própria — e cada leitura são três processos `git` (`rev-parse` duas
 * vezes e `status`). Com três agentes na tela eram 15 processos a cada 3 s para a mesma resposta.
 * Agora as leituras de fundo que caem dentro da mesma janela compartilham o mesmo pedido.
 * Depois de uma ação (stage, commit, checkout) quem agiu pede `fresco` e o resultado vale para todos.
 */
import { gitStatus, type GitStatus } from "./gitService";

/** Menor que o ritmo dos polls (3 s): cada tique ainda vê o disco, só não repete o vizinho. */
export const JANELA_DO_STATUS_MS = 2_000;

type Leitor = (pasta: string) => Promise<GitStatus | null>;
const lidos = new Map<string, { em: number; pedido: Promise<GitStatus | null> }>();

export function gitStatusCompartilhado(pasta: string, { fresco = false, ler = gitStatus as Leitor, agora = Date.now() } = {}): Promise<GitStatus | null> {
  const anterior = lidos.get(pasta);
  if (!fresco && anterior && agora - anterior.em < JANELA_DO_STATUS_MS) return anterior.pedido;
  const pedido = ler(pasta);
  lidos.set(pasta, { em: agora, pedido });
  // Falha não fica guardada: o próximo tique tenta de novo.
  pedido.catch(() => { if (lidos.get(pasta)?.pedido === pedido) lidos.delete(pasta); });
  return pedido;
}

/** Só para testes. */
export function esquecerStatusCompartilhado() {
  lidos.clear();
}
