// Transporte da revisão oficial em conversa separada.
import { actionKey, request, type Timeline } from "../../mobile/api";
import { listTerminalSessions } from "../terminal/terminalService";
import { enviarComentarios } from "./revisaoService";

export interface PedidoDeRevisao { provider: "claude" | "codex"; escopo: "local" | "branch"; base?: string }
export interface RevisaoCriada { id: string; session_id: string; name: string; origem: string }
export async function revisarComAgente(sessionId: string, pedido: PedidoDeRevisao, chave = actionKey()) {
  const sessao = (await listTerminalSessions()).find(s => s.id === sessionId);
  if (!sessao?.conversation_id) throw new Error("Abra a Conversa da origem antes de pedir a revisão.");
  return request<RevisaoCriada>(`/conversas/${encodeURIComponent(sessao.conversation_id)}/revisao`, {
    method: "POST", headers: { "Content-Type": "application/json", "Idempotency-Key": chave }, body: JSON.stringify(pedido),
  });
}
export async function mandarAchados(sessionId: string, revisao: RevisaoCriada, chave: string) {
  const timeline = await request<Timeline>(`/conversas/${encodeURIComponent(revisao.id)}/timeline`);
  if (timeline.conversation?.state === "working") throw new Error("A revisão ainda está trabalhando. Aguarde o revisor terminar.");
  const achados = timeline.timeline.messages.filter(m => m.role === "assistant" && m.kind === "text").map(m => m.text).join("\n\n");
  if (!achados.trim()) throw new Error("A revisão ainda não trouxe achados. Aguarde a resposta do revisor.");
  const texto = `Confira e ajuste os achados da conversa de revisão ${revisao.id}. Valide cada ponto no estado atual dos arquivos:\n\n${achados}`;
  if (texto.length > 15000) throw new Error("Revisão longa: selecione os achados na conversa e envie por partes.");
  await enviarComentarios(sessionId,texto,chave);
}
