// Fork: HTTP 202 só confirma a fila; controles precisam saber se a CLI aceitou a ação.
import { request, type Timeline } from "../api";

export async function aguardarAcao(conversaId: string, acaoId: string | undefined): Promise<void> {
  if (!acaoId) throw new Error("O engine não confirmou a ação");
  for (let tentativa = 0; tentativa < 100; tentativa += 1) {
    const lida = await request<Timeline>(`/conversas/${encodeURIComponent(conversaId)}/timeline`);
    const acao = lida.actions.find((a) => a.id === acaoId);
    if (acao?.state === "sent") return;
    if (acao?.state === "rejected") throw new Error(acao.error ?? "A CLI recusou a ação");
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error("A CLI ainda não confirmou a ação. Confira o terminal antes de tentar novamente.");
}
