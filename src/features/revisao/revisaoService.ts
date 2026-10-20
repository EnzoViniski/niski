import { invoke } from "@tauri-apps/api/core";
import { actionKey, request, type Timeline } from "../../mobile/api";
import { listTerminalSessions } from "../terminal/terminalService";

export interface Linha { texto: string; tipo: string; antiga: number | null; nova: number | null }
export interface Trecho { cabecalho: string; linhas: Linha[]; patch: string }
export interface Arquivo {
  caminho: string; staged: boolean; status: string; adicionadas: number; removidas: number;
  patch: string; trechos: Trecho[]; granular: boolean;
}
export type Acao = "stage" | "unstage" | "descartar";
export interface Comentario {
  id: string; arquivo: string; linha: number; lado: "anterior" | "atual";
  trecho: string; texto: string;
}

export const listarRevisao = (projectPath: string, staged: boolean) => invoke<Arquivo[]>("revisao_listar", { projectPath, staged });
export const aplicarRevisao = (projectPath: string, arquivo: Arquivo, acao: Acao, trecho: number | null) => invoke<void>("revisao_aplicar", {
  projectPath, caminho: arquivo.caminho, staged: arquivo.staged, esperado: arquivo.patch, acao, trecho,
});

export function montarComentarios(comentarios: Comentario[]): string {
  return "Revise e ajuste os pontos abaixo. Os trechos são snapshots da revisão; confira o estado atual dos arquivos antes de editar.\n\n" + comentarios.map(c =>
    `${c.arquivo}:${c.linha} (versão ${c.lado})\nTrecho:\n${c.trecho}\nComentário: ${c.texto.trim()}`,
  ).join("\n\n");
}

/** Mesma rota, transporte, revisão da tela e fila usados pela Conversa/compositor. */
export async function enviarComentarios(sessionId: string, texto: string, chave: string = actionKey()) {
  const sessao = (await listTerminalSessions()).find(s => s.id === sessionId);
  if (!sessao?.conversation_id) throw new Error("Esta aba ainda não tem uma conversa ativa. Abra a Conversa primeiro.");
  const rota = `/conversas/${encodeURIComponent(sessao.conversation_id)}`;
  const atual = (await request<Timeline>(`${rota}/timeline`)).conversation?.capabilities;
  if (!atual || (!atual.prompt && !atual.enfileirar)) {
    throw new Error(atual?.approve ? "Responda o pedido de permissão na Conversa antes de enviar." : atual?.reason ?? "O agente não pode receber comentários agora.");
  }
  await request(`${rota}/prompt`, {
    method: "POST",
    headers: { "Content-Type": "application/json", "Idempotency-Key": chave, "If-Match": atual.revision },
    body: JSON.stringify({ texto, ...(!atual.prompt ? { fila: true } : {}) }),
  });
}
