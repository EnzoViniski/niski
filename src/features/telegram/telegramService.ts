// Fork: só o transporte IPC do computador recebe configuração do bot; nunca salva token no front.
import { engineApi } from "../terminal/terminalService";
import type { TipoDeAtividade } from "../atividade/tipos";

export interface TelegramStatus {
  conectado: boolean; ativo: boolean; bot: string; tipos: TipoDeAtividade[]; incluir_titulos: boolean;
  erro: string | null; parear_url: string | null; expira_em_ms: number | null;
}
export async function telegram(caminho = "", corpo?: unknown): Promise<TelegramStatus> {
  const resposta = await engineApi(corpo === undefined ? "GET" : "POST", `/telegram${caminho}`, corpo === undefined ? null : JSON.stringify(corpo));
  if (!resposta || resposta.status >= 400) {
    let erro = "Não consegui consultar o Telegram. Atualize o app e tente novamente.";
    try { erro = (JSON.parse(resposta.body) as { error?: string }).error ?? erro; } catch { /* engine antigo */ }
    throw new Error(erro);
  }
  return JSON.parse(resposta.body) as TelegramStatus;
}
