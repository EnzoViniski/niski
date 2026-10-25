// Fork: o front nunca guarda o token; ele vai uma vez ao engine (IPC do PC ou celular pareado).
import { engineApi } from "../terminal/terminalService";
import type { TipoDeAtividade } from "../atividade/tipos";

export interface TelegramStatus {
  conectado: boolean; ativo: boolean; bot: string; tipos: TipoDeAtividade[]; incluir_titulos: boolean;
  erro: string | null; parear_url: string | null; expira_em_ms: number | null;
}
/** Fork (Telegram no celular): no app do PC, IPC; no celular pareado, a rede (`mobile/telas/Telegram.tsx`). */
export type TransporteDoTelegram = (metodo: "GET" | "POST", caminho: string, corpo: string | null) => Promise<{ status: number; body: string } | null>;
let transporte: TransporteDoTelegram = (metodo, caminho, corpo) => engineApi(metodo, caminho, corpo);
export function usarTransporteDoTelegram(novo: TransporteDoTelegram) { transporte = novo; }

export async function telegram(caminho = "", corpo?: unknown): Promise<TelegramStatus> {
  const resposta = await transporte(corpo === undefined ? "GET" : "POST", `/telegram${caminho}`, corpo === undefined ? null : JSON.stringify(corpo));
  if (!resposta || resposta.status >= 400) {
    let erro = "Não consegui consultar o Telegram. Atualize o app e tente novamente.";
    try { erro = (JSON.parse(resposta?.body ?? "") as { error?: string }).error ?? erro; } catch { /* engine antigo */ }
    throw new Error(erro);
  }
  return JSON.parse(resposta.body) as TelegramStatus;
}
